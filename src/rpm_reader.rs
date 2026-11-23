use crate::rpm_ffi::RpmInfo;
use anyhow::{anyhow, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use tokio::task;

const LEAD_SIZE: usize = 96;
const HEADER_MAGIC: [u8; 4] = [0x8e, 0xad, 0xe8, 0x01];
const XZ_MAGIC: [u8; 4] = [0xfd, 0x37, 0x7a, 0x58];

// ============================================================================
// Legacy heuristic parser. Superseded by the shared tag-based reader.
// ============================================================================
#[allow(dead_code)]
pub fn read_rpm_fast(path: &Path) -> Result<RpmInfo> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let file_len = mmap.len();

    if file_len < LEAD_SIZE {
        return Err(anyhow!("File too small"));
    }
    let lead = &mmap[0..LEAD_SIZE];
    if &lead[0..4] != b"\xed\xab\xee\xdb" {
        return Err(anyhow!("Invalid RPM magic"));
    }

    let mut pos = LEAD_SIZE;

    let sig_size = u32::from_le_bytes([lead[76], lead[77], lead[78], lead[79]]);
    const MAX_SANE_SIG_SIZE: u32 = 1 << 20; // 1 MB
    if sig_size > 0 && sig_size <= MAX_SANE_SIG_SIZE && pos + sig_size as usize <= file_len {
        pos += sig_size as usize;
    } else if sig_size > 0 {
        if let Some(header_start) = find_header_magic(&mmap[pos..]) {
            pos += header_start;
        } else {
            return Err(anyhow!("Cannot find header magic"));
        }
    }

    let header = parse_header(&mmap[pos..])?;
    let header_size = 16 + header.index_count * 16 + header.data_size;
    let header_end = pos + header_size;

    let payload_start = find_payload_start(&mmap[header_end..])?;
    let meta_buf = &mmap[header_end..header_end + payload_start];

    let strings: Vec<String> = meta_buf
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| String::from_utf8(s.to_vec()).ok())
        .collect();

    let source_rpm = strings.iter().find(|s| s.ends_with(".src.rpm")).cloned();
    let name = strings
        .iter()
        .find(|s| {
            s.chars().all(|c| c.is_ascii_alphabetic() || c == '-')
                && s.len() >= 2
                && !s.contains("BEGIN")
                && !s.contains("END")
        })
        .cloned()
        .or_else(|| {
            source_rpm
                .as_ref()
                .and_then(|s| s.split('-').next().map(|n| n.to_string()))
        })
        .unwrap_or_default();
    let version = strings
        .iter()
        .find(|s| s.chars().all(|c| c.is_ascii_digit() || c == '.') && s.matches('.').count() == 2)
        .cloned()
        .unwrap_or_default();
    let release = strings
        .iter()
        .find(|s| s.chars().all(|c| c.is_ascii_digit()) && s.len() <= 5)
        .cloned()
        .unwrap_or_default();
    let arch = strings
        .iter()
        .find(|s| s == &"x86_64" || s == &"i386" || s == &"noarch")
        .cloned();
    let url = strings.iter().find(|s| s.starts_with("http")).cloned();
    let license = strings
        .iter()
        .find(|s| s.contains("ASL") || s.contains("GPL") || s.contains("BSD") || s.contains("MIT"))
        .cloned();
    let summary = strings
        .iter()
        .find(|s| {
            s.len() >= 10
                && s.len() <= 200
                && !s.contains('/')
                && !s.starts_with("http")
                && !s.contains("BEGIN PGP")
                && s.chars().filter(|c| c.is_ascii_alphabetic()).count() > s.len() / 2
        })
        .cloned();
    let description = strings
        .iter()
        .find(|s| {
            s.len() > 100
                && s.len() < 2000
                && !s.contains("BEGIN PGP")
                && !s.contains("END PGP")
                && s.lines().count() > 1
                && s.chars().filter(|c| c.is_ascii_alphabetic()).count() > s.len() / 3
        })
        .cloned();

    Ok(RpmInfo {
        path: path.to_path_buf(),
        raw_header: Vec::new(),
        name,
        version,
        release,
        arch: arch.unwrap_or_default(),
        epoch: None,
        summary: summary.unwrap_or_default(),
        description: description.unwrap_or_default(),
        url: url.unwrap_or_default(),
        license: license.unwrap_or_default(),
        sourcerpm: source_rpm.unwrap_or_default(),
        size: 0,
        group: String::new(),
        provides: vec![],
        requires: vec![],
        conflicts: vec![],
        obsoletes: vec![],
        files: vec![],
        changelog_time: vec![],
        changelog_name: vec![],
        changelog_text: vec![],
    })
}

// ============================================================================
// Slow async parser for raw_header (used for hdlist only)
// ============================================================================
// Kept as a reference implementation until stage 3 replaces hdlist writing.
#[allow(dead_code)]
pub async fn read_raw_header_slow_async(path: &Path) -> Result<Vec<u8>> {
    let path = path.to_path_buf();
    task::spawn_blocking(move || {
        let pkg = rpm::Package::open(&path)?;
        Ok(pkg.header_bytes()?)
    })
    .await?
}

// Legacy entry point; superseded by RpmReaderRegistry in main.rs.
#[allow(dead_code)]
pub fn read_rpm(path: &Path) -> Result<RpmInfo> {
    read_rpm_fast(path)
}

struct ParsedHeader {
    index_count: usize,
    data_size: usize,
}

fn parse_header(data: &[u8]) -> Result<ParsedHeader> {
    if data.len() < 16 {
        return Err(anyhow!("Header too short"));
    }
    let magic = &data[0..4];
    if magic != HEADER_MAGIC {
        return Err(anyhow!("Invalid header magic"));
    }
    let index_count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let data_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
    Ok(ParsedHeader {
        index_count,
        data_size,
    })
}

fn find_header_magic(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|window| window == HEADER_MAGIC)
}

fn find_payload_start(data: &[u8]) -> Result<usize> {
    if let Some(pos) = data.windows(4).position(|window| window == XZ_MAGIC) {
        Ok(pos)
    } else {
        Ok(0)
    }
}
