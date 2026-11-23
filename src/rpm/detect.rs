//! On-disk RPM format detection.
//!
//! Detection is intentionally split into two parts:
//!   1. structural scan (lead + header chain) - exact, no guessing;
//!   2. format classification - heuristic for now, to be replaced at stage 1
//!      with exact tag rules derived from genhdlist2.pm and URPM sources.

use anyhow::{anyhow, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

use super::header::Header;

/// RPMTAG_RPMVERSION - the authoritative format version, unlike the frozen
/// 3.0 lead that every modern RPM carries.
const TAG_RPMVERSION: u32 = 1064;

pub const LEAD_SIZE: usize = 96;
pub const LEAD_MAGIC: [u8; 4] = [0xed, 0xab, 0xee, 0xdb];
pub const HEADER_MAGIC: [u8; 4] = [0x8e, 0xad, 0xe8, 0x01];

const MAX_INDEX_COUNT: usize = 1 << 16;
// Only the signature header and the main header are needed. Scanning the
// rest of a large RPM byte-by-byte is pure overhead.
const MAX_HEADERS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RpmFormat {
    Rpm3,
    Rpm4,
    Rpm5,
    Rpm6,
}

impl RpmFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            RpmFormat::Rpm3 => "rpm3",
            RpmFormat::Rpm4 => "rpm4",
            RpmFormat::Rpm5 => "rpm5",
            RpmFormat::Rpm6 => "rpm6",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Exact,
    Probable,
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub format: RpmFormat,
    pub confidence: Confidence,
    pub lead_major: u8,
    pub lead_minor: u8,
    pub signature_offset: Option<usize>,
    pub header_offset: usize,
    /// Value of RPMTAG_RPMVERSION when present (e.g. "5.4.10").
    pub rpm_version: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct HeaderInfo {
    index_count: usize,
    data_size: usize,
    total_size: usize,
}

/// Parse the fixed 16-byte header prefix and validate its declared sizes.
fn parse_header_info(data: &[u8], offset: usize) -> Result<HeaderInfo> {
    if offset.checked_add(16).is_none_or(|end| end > data.len()) {
        return Err(anyhow!("header at {} is truncated", offset));
    }
    let slice = &data[offset..offset + 16];
    if slice[..4] != HEADER_MAGIC {
        return Err(anyhow!("invalid header magic at {}", offset));
    }

    let index_count = u32::from_be_bytes([slice[8], slice[9], slice[10], slice[11]]) as usize;
    let data_size = u32::from_be_bytes([slice[12], slice[13], slice[14], slice[15]]) as usize;
    if index_count > MAX_INDEX_COUNT {
        return Err(anyhow!(
            "implausible index count {} at {}",
            index_count,
            offset
        ));
    }

    let total_size = 16usize
        .checked_add(
            index_count
                .checked_mul(16)
                .ok_or_else(|| anyhow!("index size overflow"))?,
        )
        .and_then(|v| v.checked_add(data_size))
        .ok_or_else(|| anyhow!("header size overflow"))?;

    if offset + total_size > data.len() {
        return Err(anyhow!(
            "header at {} needs {} bytes, have {}",
            offset,
            total_size,
            data.len() - offset
        ));
    }

    Ok(HeaderInfo {
        index_count,
        data_size,
        total_size,
    })
}

/// Collect structurally valid headers, skipping over each header body so that
/// magic bytes inside index/data are not mistaken for a new header.
fn find_headers(data: &[u8], from: usize) -> Vec<(usize, HeaderInfo)> {
    let mut found = Vec::new();
    let mut pos = from;
    while pos + 16 <= data.len() && found.len() < MAX_HEADERS {
        if data[pos..pos + 4] == HEADER_MAGIC {
            if let Ok(info) = parse_header_info(data, pos) {
                found.push((pos, info));
                pos += info.total_size;
                continue;
            }
        }
        pos += 1;
    }
    found
}

/// Classify a package from its lead and header layout.
///
/// RPM5 keeps lead major 3 for backward compatibility, so the lead alone
/// cannot separate v3 from rpm5. Lead 3 is therefore treated as rpm5 with
/// `Probable` confidence; stage 1 will replace this with exact tag rules.
fn classify(version: Option<&str>, lead_major: u8) -> (RpmFormat, Confidence) {
    if let Some(major) = version
        .and_then(|v| v.chars().next())
        .and_then(|c| c.to_digit(10))
    {
        return match major {
            3 => (RpmFormat::Rpm3, Confidence::Exact),
            4 => (RpmFormat::Rpm4, Confidence::Exact),
            5 => (RpmFormat::Rpm5, Confidence::Exact),
            _ => (RpmFormat::Rpm6, Confidence::Exact),
        };
    }
    // No RPMVERSION tag (very old packages): fall back to the lead, but mark
    // the result as probable - the lead is frozen at 3.0 in most modern RPMs.
    match lead_major {
        3 => (RpmFormat::Rpm3, Confidence::Probable),
        4 => (RpmFormat::Rpm4, Confidence::Probable),
        5 => (RpmFormat::Rpm5, Confidence::Probable),
        _ => (RpmFormat::Rpm6, Confidence::Probable),
    }
}

/// Detect the format of an on-disk package.
pub fn detect_format(path: &Path) -> Result<Detection> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    detect(&mmap)
}

/// Like [`detect_format`], but lets the caller force a plugin when detection
/// is ambiguous. An explicit hint is reported as `Confidence::Exact`.
pub fn detect_format_with_hint(path: &Path, hint: Option<RpmFormat>) -> Result<Detection> {
    let mut detection = detect_format(path)?;
    if let Some(format) = hint {
        detection.format = format;
        detection.confidence = Confidence::Exact;
    }
    Ok(detection)
}

/// Detect from an already-mapped buffer (used by tests and by batch readers).
pub fn detect(data: &[u8]) -> Result<Detection> {
    if data.len() < LEAD_SIZE {
        return Err(anyhow!("file is shorter than the 96-byte RPM lead"));
    }
    if data[..4] != LEAD_MAGIC {
        return Err(anyhow!("invalid RPM lead magic"));
    }

    let lead_major = data[4];
    let lead_minor = data[5];

    let headers = find_headers(data, LEAD_SIZE);
    if headers.is_empty() {
        return Err(anyhow!("no valid RPM header found after lead"));
    }

    let (signature_offset, header_offset) = if headers.len() >= 2 {
        (Some(headers[0].0), headers[1].0)
    } else {
        (None, headers[0].0)
    };
    let rpm_version = Header::parse(data, header_offset)
        .ok()
        .and_then(|header| header.string(TAG_RPMVERSION));
    let (format, confidence) = classify(rpm_version.as_deref(), lead_major);

    Ok(Detection {
        format,
        confidence,
        lead_major,
        lead_minor,
        signature_offset,
        header_offset,
        rpm_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(index_count: u32, data_size: u32) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(&HEADER_MAGIC);
        header.extend_from_slice(&[0u8; 4]);
        header.extend_from_slice(&index_count.to_be_bytes());
        header.extend_from_slice(&data_size.to_be_bytes());
        let size = 16 + index_count as usize * 16 + data_size as usize;
        header.resize(size, 0);
        header
    }

    fn make_package(lead_major: u8, signature_type: u16, with_signature: bool) -> Vec<u8> {
        let mut data = vec![0u8; LEAD_SIZE];
        data[..4].copy_from_slice(&LEAD_MAGIC);
        data[4] = lead_major;
        data[5] = 0;
        data[78..80].copy_from_slice(&signature_type.to_be_bytes());
        if with_signature {
            data.extend_from_slice(&make_header(2, 0));
        }
        data.extend_from_slice(&make_header(3, 10));
        data.extend_from_slice(&[0u8; 16]);
        data
    }

    #[test]
    fn falls_back_to_lead_without_rpmversion() {
        // Synthetic header has no RPMTAG_RPMVERSION, so classification falls
        // back to the lead and is only Probable. Real packages are classified
        // exactly from tag 1064 (see reads_all_fixture_formats).
        let data = make_package(3, 0, false);
        let d = detect(&data).unwrap();
        assert_eq!(d.format, RpmFormat::Rpm3);
        assert_eq!(d.confidence, Confidence::Probable);
        assert_eq!(d.header_offset, LEAD_SIZE);
        assert_eq!(d.signature_offset, None);
        assert_eq!(d.rpm_version, None);
    }

    #[test]
    fn detects_signature_and_main_header() {
        let data = make_package(4, 5, true);
        let d = detect(&data).unwrap();
        assert_eq!(d.format, RpmFormat::Rpm4);
        assert_eq!(d.signature_offset, Some(LEAD_SIZE));
        assert_eq!(d.header_offset, LEAD_SIZE + 16 + 2 * 16);
    }

    #[test]
    fn rejects_non_rpm_data() {
        let data = vec![0u8; LEAD_SIZE + 64];
        assert!(detect(&data).is_err());
    }

    #[test]
    fn rejects_truncated_header() {
        let mut data = make_package(4, 0, false);
        data.truncate(LEAD_SIZE + 8);
        assert!(detect(&data).is_err());
    }
}
