//! Safe, tag-based reader for the RPM header structure.
//!
//! Both the signature header and the main header use the same layout:
//!   16-byte prefix (magic, 4 reserved bytes, index count, data size),
//!   `index_count` entries of 16 bytes (tag, type, offset, count),
//!   followed by the data store.
//! All integers are stored big-endian. No field is guessed from raw bytes.

use anyhow::{anyhow, Result};

pub const HEADER_MAGIC: [u8; 4] = [0x8e, 0xad, 0xe8, 0x01];

const MAX_INDEX_COUNT: usize = 1 << 16;
const MAX_DATA_SIZE: usize = 1 << 26;

pub const TYPE_STRING: u32 = 6;
pub const TYPE_BIN: u32 = 7;
pub const TYPE_STRING_ARRAY: u32 = 8;
pub const TYPE_I18NSTRING: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub tag: u32,
    pub typ: u32,
    pub offset: usize,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct Header<'a> {
    pub raw: &'a [u8],
    pub index_count: usize,
    pub data_size: usize,
    pub total_size: usize,
    data_start: usize,
}

fn get_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

impl<'a> Header<'a> {
    /// Parse a header at `offset` in `data`, validating every declared size.
    pub fn parse(data: &'a [u8], offset: usize) -> Result<Self> {
        let prefix_end = offset
            .checked_add(16)
            .ok_or_else(|| anyhow!("header offset overflow"))?;
        let prefix = data
            .get(offset..prefix_end)
            .ok_or_else(|| anyhow!("header at {} is truncated", offset))?;
        if prefix[..4] != HEADER_MAGIC {
            return Err(anyhow!("invalid header magic at {}", offset));
        }

        let index_count =
            get_u32(prefix, 8).ok_or_else(|| anyhow!("missing index count"))? as usize;
        let data_size = get_u32(prefix, 12).ok_or_else(|| anyhow!("missing data size"))? as usize;
        if index_count > MAX_INDEX_COUNT {
            return Err(anyhow!(
                "implausible index count {} at {}",
                index_count,
                offset
            ));
        }
        if data_size > MAX_DATA_SIZE {
            return Err(anyhow!("implausible data size {} at {}", data_size, offset));
        }

        let index_bytes = index_count
            .checked_mul(16)
            .ok_or_else(|| anyhow!("index size overflow"))?;
        let data_start = 16usize
            .checked_add(index_bytes)
            .ok_or_else(|| anyhow!("header layout overflow"))?;
        let total_size = data_start
            .checked_add(data_size)
            .ok_or_else(|| anyhow!("header size overflow"))?;
        let end = offset
            .checked_add(total_size)
            .ok_or_else(|| anyhow!("header end overflow"))?;
        let raw = data.get(offset..end).ok_or_else(|| {
            anyhow!(
                "header at {} needs {} bytes, have {}",
                offset,
                total_size,
                data.len() - offset
            )
        })?;

        Ok(Self {
            raw,
            index_count,
            data_size,
            total_size,
            data_start,
        })
    }

    pub fn entry(&self, tag: u32) -> Option<Entry> {
        (0..self.index_count).find_map(|i| {
            let base = 16 + i * 16;
            let found = get_u32(self.raw, base)?;
            if found != tag {
                return None;
            }
            Some(Entry {
                tag: found,
                typ: get_u32(self.raw, base + 4)?,
                offset: get_u32(self.raw, base + 8)? as usize,
                count: get_u32(self.raw, base + 12)? as usize,
            })
        })
    }

    /// STRING (6) and I18NSTRING (9) values share the same representation.
    pub fn string(&self, tag: u32) -> Option<String> {
        let entry = self.entry(tag)?;
        if entry.typ != TYPE_STRING && entry.typ != TYPE_I18NSTRING {
            return None;
        }
        if entry.count == 0 {
            return Some(String::new());
        }
        let start = self.data_start.checked_add(entry.offset)?;
        let rest = self.raw.get(start..)?;
        let end = rest.iter().position(|&b| b == 0)?;
        Some(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    /// STRING_ARRAY (8): `count` consecutive NUL-terminated strings.
    pub fn string_array(&self, tag: u32) -> Vec<String> {
        let entry = match self.entry(tag) {
            Some(e) => e,
            None => return Vec::new(),
        };
        if entry.typ != TYPE_STRING_ARRAY {
            return Vec::new();
        }
        let mut pos = match self.data_start.checked_add(entry.offset) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let mut values = Vec::with_capacity(entry.count.min(4096));
        for _ in 0..entry.count.min(4096) {
            let rest = match self.raw.get(pos..) {
                Some(r) => r,
                None => break,
            };
            let end = match rest.iter().position(|&b| b == 0) {
                Some(e) => e,
                None => break,
            };
            values.push(String::from_utf8_lossy(&rest[..end]).into_owned());
            pos += end + 1;
        }
        values
    }

    /// INT32 (4): `count` big-endian 32-bit integers.
    pub fn int32(&self, tag: u32) -> Vec<u32> {
        let entry = match self.entry(tag) {
            Some(e) => e,
            None => return Vec::new(),
        };
        if entry.typ != 4 {
            return Vec::new();
        }
        let start = match self.data_start.checked_add(entry.offset) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let mut values = Vec::with_capacity(entry.count.min(65536));
        for i in 0..entry.count.min(65536) {
            let at = match start.checked_add(i * 4) {
                Some(p) => p,
                None => break,
            };
            match get_u32(self.raw, at) {
                Some(v) => values.push(v),
                None => break,
            }
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_header(entries: &[(u32, u32, u32, u32)], data: &[u8]) -> Vec<u8> {
        let mut h = Vec::new();
        h.extend_from_slice(&HEADER_MAGIC);
        h.extend_from_slice(&[0u8; 4]);
        h.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        h.extend_from_slice(&(data.len() as u32).to_be_bytes());
        for (tag, typ, offset, count) in entries {
            h.extend_from_slice(&tag.to_be_bytes());
            h.extend_from_slice(&typ.to_be_bytes());
            h.extend_from_slice(&offset.to_be_bytes());
            h.extend_from_slice(&count.to_be_bytes());
        }
        h.extend_from_slice(data);
        h
    }

    #[test]
    fn reads_string_i18n_and_array() {
        let data = b"zstd\0zstd package\0en\0";
        let header = build_header(
            &[
                (1000, TYPE_STRING, 0, 1),
                (1004, TYPE_I18NSTRING, 5, 1),
                (1118, TYPE_STRING_ARRAY, 18, 1),
            ],
            data,
        );
        let parsed = Header::parse(&header, 0).unwrap();
        assert_eq!(parsed.string(1000).as_deref(), Some("zstd"));
        assert_eq!(parsed.string(1004).as_deref(), Some("zstd package"));
        assert_eq!(parsed.string_array(1118), vec!["en"]);
    }

    #[test]
    fn rejects_corrupted_sizes() {
        let mut header = build_header(&[(1000, TYPE_STRING, 0, 1)], b"x\0");
        let len = header.len() as u32;
        header[12..16].copy_from_slice(&len.to_be_bytes());
        header.extend_from_slice(&[0u8; 8]);
        assert!(Header::parse(&header, 0).is_err());
    }
}
