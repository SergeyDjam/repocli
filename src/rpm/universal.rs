//! Shared tag-based RPM reader.
//!
//! The header layout (magic, index entries, data store) and the tag numbers
//! used here are identical across rpm3, rpm4, rpm5 and rpm6. Version-specific
//! plugins are thin adapters over this module; only payload handling differs
//! between formats and repocli never needs the payload.

use super::detect::detect;
use super::header::Header;
use super::RpmInfo;
use anyhow::{anyhow, Result};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

const TAG_NAME: u32 = 1000;
const TAG_VERSION: u32 = 1001;
const TAG_RELEASE: u32 = 1002;
const TAG_EPOCH: u32 = 1003;
const TAG_SUMMARY: u32 = 1004;
const TAG_DESCRIPTION: u32 = 1005;
const TAG_SIZE: u32 = 1009;
const TAG_GROUP: u32 = 1016;
const TAG_LICENSE: u32 = 1014;
const TAG_URL: u32 = 1020;
const TAG_ARCH: u32 = 1022;
const TAG_SOURCERPM: u32 = 1044;
const TAG_DIRINDEXES: u32 = 1116;
const TAG_BASENAMES: u32 = 1117;
const TAG_DIRNAMES: u32 = 1118;
const TAG_CHANGELOGTIME: u32 = 1080;
const TAG_CHANGELOGNAME: u32 = 1081;
const TAG_CHANGELOGTEXT: u32 = 1082;
const TAG_PROVIDENAME: u32 = 1047;
const TAG_PROVIDEVERSION: u32 = 1113;
const TAG_PROVIDEFLAGS: u32 = 1112;
const TAG_REQUIRENAME: u32 = 1049;
const TAG_REQUIREVERSION: u32 = 1050;
const TAG_REQUIREFLAGS: u32 = 1048;
const TAG_CONFLICTNAME: u32 = 1054;
const TAG_CONFLICTVERSION: u32 = 1055;
const TAG_CONFLICTFLAGS: u32 = 1053;
const TAG_OBSOLETENAME: u32 = 1090;
const TAG_OBSOLETEVERSION: u32 = 1091;
const TAG_OBSOLETEFLAGS: u32 = 1092;

// URPM::print_list_entry(): Fstr[(flags >> 1) & 0x7]
const FLAG_STRINGS: [&str; 8] = ["?0", "<", ">", "?3", "==", "<=", ">=", "?7"];

/// Build dependency entries in the exact form produced by URPM:
/// `name[op evr]`, joined by '@' in the synthesis writer.
fn dependencies(header: &Header<'_>, names: u32, versions: u32, flags: u32) -> Vec<String> {
    let names = header.string_array(names);
    let versions = header.string_array(versions);
    let flags = header.int32(flags);
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let mut entry = name;
            let sense = flags.get(i).copied().unwrap_or(0);
            // RPMSENSE_RPMLIB is skipped by print_list_entry(); bit 14 is
            // RPMSENSE_FIND_REQUIRES and must be kept.
            if sense & 0x1000000 != 0 {
                return String::new();
            }
            if sense & 0x10000 != 0 {
                entry.push_str("[*]");
            }
            if let Some(version) = versions.get(i) {
                if !version.is_empty() {
                    let op = FLAG_STRINGS[((sense >> 1) & 0x7) as usize];
                    entry.push('[');
                    entry.push_str(op);
                    entry.push(' ');
                    entry.push_str(version);
                    entry.push(']');
                }
            }
            entry
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn file_list(header: &Header<'_>) -> Vec<String> {
    let dirs = header.string_array(TAG_DIRNAMES);
    let names = header.string_array(TAG_BASENAMES);
    let indexes = header.int32(TAG_DIRINDEXES);
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let dir = indexes
                .get(i)
                .and_then(|d| dirs.get(*d as usize))
                .map(String::as_str)
                .unwrap_or("");
            format!("{}{}", dir, name)
        })
        .collect()
}

/// Read metadata and the raw header from a mapped RPM file.
pub fn read_mapped(path: &Path, mmap: &[u8]) -> Result<RpmInfo> {
    let detection = detect(mmap)?;

    if let Some(offset) = detection.signature_offset {
        Header::parse(mmap, offset).map_err(|e| anyhow!("bad signature header: {}", e))?;
    }

    let main = Header::parse(mmap, detection.header_offset)
        .map_err(|e| anyhow!("bad main header: {}", e))?;
    if main.entry(TAG_NAME).is_none() {
        return Err(anyhow!("main header has no NAME tag (tag {})", TAG_NAME));
    }

    Ok(RpmInfo {
        path: path.to_path_buf(),
        raw_header: main.raw.to_vec(),
        name: main.string(TAG_NAME).unwrap_or_default(),
        version: main.string(TAG_VERSION).unwrap_or_default(),
        release: main.string(TAG_RELEASE).unwrap_or_default(),
        arch: main.string(TAG_ARCH).unwrap_or_default(),
        epoch: main.int32(TAG_EPOCH).into_iter().next(),
        summary: main.string(TAG_SUMMARY).unwrap_or_default(),
        description: main.string(TAG_DESCRIPTION).unwrap_or_default(),
        url: main.string(TAG_URL).unwrap_or_default(),
        license: main.string(TAG_LICENSE).unwrap_or_default(),
        sourcerpm: main.string(TAG_SOURCERPM).unwrap_or_default(),
        size: main.int32(TAG_SIZE).into_iter().next().unwrap_or(0) as u64,
        group: main.string(TAG_GROUP).unwrap_or_default(),
        provides: dependencies(&main, TAG_PROVIDENAME, TAG_PROVIDEVERSION, TAG_PROVIDEFLAGS),
        requires: dependencies(&main, TAG_REQUIRENAME, TAG_REQUIREVERSION, TAG_REQUIREFLAGS),
        conflicts: dependencies(
            &main,
            TAG_CONFLICTNAME,
            TAG_CONFLICTVERSION,
            TAG_CONFLICTFLAGS,
        ),
        obsoletes: dependencies(
            &main,
            TAG_OBSOLETENAME,
            TAG_OBSOLETEVERSION,
            TAG_OBSOLETEFLAGS,
        ),
        files: file_list(&main),
        changelog_time: main
            .int32(TAG_CHANGELOGTIME)
            .into_iter()
            .map(i64::from)
            .collect(),
        changelog_name: main.string_array(TAG_CHANGELOGNAME),
        changelog_text: main.string_array(TAG_CHANGELOGTEXT),
    })
}

/// Read metadata and the raw header from a file path.
pub fn read_file(path: &Path) -> Result<RpmInfo> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    read_mapped(path, &mmap)
}
