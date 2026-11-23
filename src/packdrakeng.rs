//! Writer for the MDV::Packdrakeng archive format used by `hdlist.cz`.
//!
//! Layout (format version compatible with MDV::Packdrakeng 1.14):
//!   - one or more gzip-compressed blocks of concatenated file payloads;
//!   - TOC: directory names, symlink pairs, sorted file names (each with '\n');
//!   - one 16-byte record per file: coff, csize, off, size (big-endian);
//!   - 64-byte trailer: "cz[0", counts, TOC string length, decompressor, "0]cz".
//!
//! `genhdlist2` creates the archive with `compress => gzip`, `comp_level => 9`
//! and adds each package as `add_virtual('f', fullname, header)`. The external
//! compressor makes the Perl implementation keep the whole payload in a single
//! block; we do the same. The gzip level is configurable here and does not
//! affect compatibility (any gzip stream is accepted by `gzip -d`).

#[cfg(test)]
use anyhow::anyhow;
use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::rpm::RpmInfo;

const TOC_HEADER: &[u8; 4] = b"cz[0";
const TOC_FOOTER: &[u8; 4] = b"0]cz";
const UNCOMPRESS_METHOD: &[u8] = b"gzip -d";
const TRAILER_SIZE: usize = 64;
const GZIP_LEVEL: u32 = 1;

/// Write an hdlist archive containing one virtual file per RPM header.
pub fn write_archive(packages: &[RpmInfo], output_path: &Path) -> Result<()> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for pkg in packages {
        files.insert(pkg.fullname(), pkg.raw_header.clone());
    }

    let mut plain = Vec::new();
    let mut records: Vec<(String, u32, u32, u32, u32)> = Vec::with_capacity(files.len());
    for (name, content) in &files {
        let offset = plain.len() as u32;
        let size = content.len() as u32;
        plain.extend_from_slice(content);
        records.push((name.clone(), 0, 0, offset, size));
    }

    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(GZIP_LEVEL));
    encoder.write_all(&plain)?;
    let compressed = encoder.finish()?;
    let csize = compressed.len() as u32;

    let mut out = File::create(output_path)?;
    out.write_all(&compressed)?;

    let mut toc_strings = Vec::new();
    for (name, _, _, _, _) in &records {
        toc_strings.extend_from_slice(name.as_bytes());
        toc_strings.push(b'\n');
    }
    out.write_all(&toc_strings)?;

    for (_, coff, _, off, size) in &records {
        out.write_all(&coff.to_be_bytes())?;
        out.write_all(&csize.to_be_bytes())?;
        out.write_all(&off.to_be_bytes())?;
        out.write_all(&size.to_be_bytes())?;
    }

    let mut trailer = Vec::with_capacity(TRAILER_SIZE);
    trailer.extend_from_slice(TOC_HEADER);
    trailer.extend_from_slice(&0u32.to_be_bytes()); // directories
    trailer.extend_from_slice(&0u32.to_be_bytes()); // symlinks
    trailer.extend_from_slice(&(records.len() as u32).to_be_bytes());
    trailer.extend_from_slice(&(toc_strings.len() as u32).to_be_bytes());
    let mut method = [0u8; 40];
    method[..UNCOMPRESS_METHOD.len()].copy_from_slice(UNCOMPRESS_METHOD);
    trailer.extend_from_slice(&method);
    trailer.extend_from_slice(TOC_FOOTER);
    debug_assert_eq!(trailer.len(), TRAILER_SIZE);
    out.write_all(&trailer)?;

    Ok(())
}

/// Minimal reader used by tests and diagnostics.
#[cfg(test)]
pub fn read_archive(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    if data.len() < TRAILER_SIZE {
        return Err(anyhow!("archive is too short"));
    }

    let trailer = &data[data.len() - TRAILER_SIZE..];
    if &trailer[..4] != TOC_HEADER || &trailer[60..64] != TOC_FOOTER {
        return Err(anyhow!("bad Packdrakeng trailer"));
    }
    let dirs = u32::from_be_bytes(trailer[4..8].try_into().unwrap()) as usize;
    let links = u32::from_be_bytes(trailer[8..12].try_into().unwrap()) as usize;
    let count = u32::from_be_bytes(trailer[12..16].try_into().unwrap()) as usize;
    let strings_len = u32::from_be_bytes(trailer[16..20].try_into().unwrap()) as usize;
    if dirs != 0 || links != 0 {
        return Err(anyhow!(
            "directories and symlinks are not supported by this reader"
        ));
    }

    let records_start = data.len() - TRAILER_SIZE - count * 16;
    let strings_start = records_start
        .checked_sub(strings_len)
        .ok_or_else(|| anyhow!("bad TOC length"))?;
    let names: Vec<String> = data[strings_start..records_start]
        .split(|&b| b == b'\n')
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    if names.len() != count {
        return Err(anyhow!("TOC name count mismatch"));
    }

    let mut blocks: BTreeMap<(u32, u32), Vec<u8>> = BTreeMap::new();
    for (i, _name) in names.iter().enumerate() {
        let r = &data[records_start + i * 16..records_start + i * 16 + 16];
        let coff = u32::from_be_bytes(r[0..4].try_into().unwrap()) as usize;
        let csize = u32::from_be_bytes(r[4..8].try_into().unwrap()) as usize;
        let end = coff
            .checked_add(csize)
            .ok_or_else(|| anyhow!("bad block offset"))?;
        if end > strings_start {
            return Err(anyhow!("compressed block exceeds data section"));
        }
        blocks
            .entry((coff as u32, csize as u32))
            .or_insert_with(|| data[coff..end].to_vec());
    }

    let mut result = BTreeMap::new();
    for (i, name) in names.iter().enumerate() {
        let r = &data[records_start + i * 16..records_start + i * 16 + 16];
        let coff = u32::from_be_bytes(r[0..4].try_into().unwrap());
        let csize = u32::from_be_bytes(r[4..8].try_into().unwrap());
        let off = u32::from_be_bytes(r[8..12].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(r[12..16].try_into().unwrap()) as usize;
        let block = blocks
            .get(&(coff, csize))
            .ok_or_else(|| anyhow!("missing block"))?;
        let mut decoder = flate2::read::GzDecoder::new(block.as_slice());
        let mut plain = Vec::new();
        decoder.read_to_end(&mut plain)?;
        let content = plain
            .get(off..off + size)
            .ok_or_else(|| anyhow!("bad file offset"))?;
        result.insert(name.clone(), content.to_vec());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, header: &[u8]) -> RpmInfo {
        RpmInfo {
            name: name.to_string(),
            version: "1".to_string(),
            release: "1".to_string(),
            arch: "x86_64".to_string(),
            raw_header: header.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn round_trip_archive() {
        let dir = std::env::temp_dir().join(format!("repocli-packdrakeng-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hdlist.cz");
        let packages = vec![
            package("b", b"second header bytes"),
            package("a", b"first header bytes"),
        ];
        write_archive(&packages, &path).unwrap();
        let entries = read_archive(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries.get("a-1-1.x86_64").unwrap(), b"first header bytes");
        assert_eq!(entries.get("b-1-1.x86_64").unwrap(), b"second header bytes");
        std::fs::remove_file(&path).unwrap();
    }
}
