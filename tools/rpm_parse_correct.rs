// tools/rpm_parse_correct.rs
use memmap2::Mmap;
use std::env;
use std::fs::File;
use std::path::Path;

const LEAD_SIZE: usize = 96;
const HEADER_MAGIC: [u8; 4] = [0x8e, 0xad, 0xe8, 0x01];
const XZ_MAGIC: [u8; 4] = [0xfd, 0x37, 0x7a, 0x58];

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: rpm_parse_correct <rpm-file>");
        std::process::exit(1);
    }
    let path = Path::new(&args[1]);
    if let Err(e) = parse_rpm(path) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Fast RPM parser using mmap.
fn parse_rpm(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let file_len = mmap.len();
    println!("File: {}", path.display());
    println!("Size: {} bytes", file_len);

    // Lead
    if file_len < LEAD_SIZE {
        return Err("File too small".into());
    }
    let lead = &mmap[0..LEAD_SIZE];
    if &lead[0..4] != b"\xed\xab\xee\xdb" {
        return Err("Invalid RPM magic".into());
    }
    println!("Lead magic OK.");

    // The header starts after the lead
    let mut pos = LEAD_SIZE;

    // Skip the signature if present and plausible
    let sig_size = u32::from_le_bytes([lead[76], lead[77], lead[78], lead[79]]);
    const MAX_SANE_SIG_SIZE: u32 = 1 << 20; // 1 MB
    if sig_size > 0 && sig_size <= MAX_SANE_SIG_SIZE && pos + sig_size as usize <= file_len {
        pos += sig_size as usize;
    } else if sig_size > 0 {
        // If the signature size is implausible, scan for header magic
        if let Some(header_start) = find_header_magic(&mmap[pos..]) {
            pos += header_start;
        } else {
            return Err("Cannot find header magic".into());
        }
    }

    // Read the header
    let header = parse_header(&mmap[pos..])?;
    println!(
        "\nMain header: index_count={}, data_size={}",
        header.index_count, header.data_size
    );

    // Compute the end of the header
    let header_size = 16 + header.index_count * 16 + header.data_size;
    let header_end = pos + header_size;

    // Locate the payload start
    let payload_start = find_payload_start(&mmap[header_end..])?;
    let payload_start_abs = header_end + payload_start;
    println!(
        "Payload starts at offset {} (0x{:x})",
        payload_start_abs, payload_start_abs
    );

    // Extract metadata from the area between header and payload
    let meta_len = payload_start;
    let meta_buf = &mmap[header_end..header_end + meta_len];

    // Split into strings
    let strings: Vec<String> = meta_buf
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| String::from_utf8(s.to_vec()).ok())
        .collect();

    // Heuristic parsing
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
    let vendor = strings
        .iter()
        .find(|s| *s == "ROSA" || *s == "Fedora" || *s == "CentOS")
        .cloned();
    let group = strings
        .iter()
        .find(|s| *s == "Security" || *s == "Development/Languages" || *s == "System")
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

    println!("\n--- Parsed fields ---");
    println!("Name: {}", name);
    println!("Version: {}", version);
    println!("Release: {}", release);
    println!("Arch: {}", arch.unwrap_or_default());
    println!("License: {}", license.unwrap_or_default());
    println!("URL: {}", url.unwrap_or_default());
    println!("Summary: {}", summary.unwrap_or_default());
    println!("Description: {}", description.unwrap_or_default());
    println!("Vendor: {}", vendor.unwrap_or_default());
    println!("Group: {}", group.unwrap_or_default());
    println!("Source RPM: {}", source_rpm.unwrap_or_default());

    Ok(())
}

/// Storage for the parsed header.
struct ParsedHeader {
    index_count: usize,
    data_size: usize,
}

/// Parse an RPM header starting at the current position.
fn parse_header(data: &[u8]) -> Result<ParsedHeader, Box<dyn std::error::Error>> {
    if data.len() < 16 {
        return Err("Header too short".into());
    }
    let magic = &data[0..4];
    if magic != HEADER_MAGIC {
        return Err("Invalid header magic".into());
    }
    let index_count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
    let data_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]) as usize;
    Ok(ParsedHeader {
        index_count,
        data_size,
    })
}

/// Fast header magic search in a slice (returns offset relative to slice start).
fn find_header_magic(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|window| window == HEADER_MAGIC)
}

/// Locate the payload start: look for xz magic, otherwise assume uncompressed.
fn find_payload_start(data: &[u8]) -> Result<usize, Box<dyn std::error::Error>> {
    // First try to find xz magic
    if let Some(pos) = data.windows(4).position(|window| window == XZ_MAGIC) {
        return Ok(pos);
    }
    // If xz magic is absent, assume the payload is uncompressed and starts here
    Ok(0)
}
