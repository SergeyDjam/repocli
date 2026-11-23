//! rpm5 plugin (ROSA / ALT Linux). Uses the shared tag-based reader.

use super::universal;
use super::{RpmFormat, RpmInfo, RpmReader};
use anyhow::Result;
use std::path::Path;

pub struct Rpm5Reader;

impl RpmReader for Rpm5Reader {
    fn format(&self) -> RpmFormat {
        RpmFormat::Rpm5
    }

    fn read(&self, path: &Path) -> Result<RpmInfo> {
        universal::read_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpm::detect::detect_format;

    const FIXTURE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/zstd-1.5.6-1-R11.x86_64.rpm"
    );

    #[test]
    fn fixture_is_detected_as_rpm5() {
        let detection = detect_format(Path::new(FIXTURE)).unwrap();
        assert_eq!(detection.format, RpmFormat::Rpm5);
        assert_eq!(detection.rpm_version.as_deref(), Some("5.4.10"));
        assert_eq!(detection.signature_offset, Some(96));
        assert_eq!(detection.header_offset, 1016);
    }

    #[test]
    fn reads_real_rpm5_fixture() {
        let info = Rpm5Reader.read(Path::new(FIXTURE)).unwrap();
        assert_eq!(info.name, "zstd");
        assert_eq!(info.version, "1.5.6");
        assert_eq!(info.release, "1");
        assert_eq!(info.arch, "x86_64");
        assert_eq!(info.size, 337_026);
        assert_eq!(info.group, "Archiving/Compression");
        assert_eq!(info.provides, vec!["zstd[== 1.5.6-1:11]".to_string()]);
        assert_eq!(info.requires.len(), 26);
        assert!(!info.raw_header.is_empty());
    }
}
