//! Pluggable RPM readers.
//!
//! Adding a new on-disk format requires only a new module that implements
//! [`RpmReader`] plus one `register` call in [`RpmReaderRegistry::default`].
//!
//! Stage 0 skeleton: the registry is not yet wired into the runtime path.
//! Remove this allowance once stage 1 switches main.rs to `RpmReaderRegistry`.
#![allow(dead_code)]

pub mod detect;
pub mod header;
pub mod model;
pub mod rpm3;
pub mod rpm4;
pub mod rpm5;
pub mod rpm6;
pub mod universal;

#[allow(unused_imports)]
pub use detect::{detect_format, detect_format_with_hint, Confidence, Detection, RpmFormat};
pub use model::RpmInfo;

use anyhow::{anyhow, Result};
use std::path::Path;

/// Reads metadata and the raw RPM header for one supported format.
pub trait RpmReader {
    fn format(&self) -> RpmFormat;
    fn read(&self, path: &Path) -> Result<RpmInfo>;
}

/// Registry of format plugins, keyed by detected format.
pub struct RpmReaderRegistry {
    readers: Vec<Box<dyn RpmReader + Send + Sync>>,
}

impl RpmReaderRegistry {
    pub fn new() -> Self {
        Self {
            readers: Vec::new(),
        }
    }

    pub fn register<R>(&mut self, reader: R)
    where
        R: RpmReader + Send + Sync + 'static,
    {
        self.readers.push(Box::new(reader));
    }

    pub fn reader_for(&self, format: RpmFormat) -> Option<&(dyn RpmReader + Send + Sync)> {
        self.readers
            .iter()
            .map(|r| r.as_ref())
            .find(|r| r.format() == format)
    }

    /// Detect the format and dispatch to the matching plugin.
    pub fn read(&self, path: &Path) -> Result<RpmInfo> {
        let detection = detect_format(path)?;
        let reader = self.reader_for(detection.format).ok_or_else(|| {
            anyhow!(
                "no reader registered for {} ({:?})",
                detection.format.as_str(),
                detection.confidence
            )
        })?;
        reader.read(path)
    }

    /// Dispatch with an explicit format hint for ambiguous packages.
    pub fn read_with_hint(&self, path: &Path, hint: RpmFormat) -> Result<RpmInfo> {
        let reader = self
            .reader_for(hint)
            .ok_or_else(|| anyhow!("no reader registered for {}", hint.as_str()))?;
        reader.read(path)
    }
}

impl Default for RpmReaderRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(rpm3::Rpm3Reader);
        registry.register(rpm4::Rpm4Reader);
        registry.register(rpm5::Rpm5Reader);
        registry.register(rpm6::Rpm6Reader);
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_covers_all_formats() {
        let registry = RpmReaderRegistry::default();
        for format in [
            RpmFormat::Rpm3,
            RpmFormat::Rpm4,
            RpmFormat::Rpm5,
            RpmFormat::Rpm6,
        ] {
            assert!(
                registry.reader_for(format).is_some(),
                "missing plugin for {}",
                format.as_str()
            );
        }
    }

    #[test]
    fn reads_all_fixture_formats() {
        let registry = RpmReaderRegistry::default();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut seen = std::collections::BTreeSet::new();

        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("rpm") {
                continue;
            }
            let detection = detect_format(&path).unwrap();
            assert_eq!(
                detection.confidence,
                Confidence::Exact,
                "{}",
                path.display()
            );
            seen.insert(detection.format);

            let info = registry.read(&path).unwrap();
            assert!(!info.name.is_empty(), "no name in {}", path.display());
            assert!(!info.version.is_empty(), "no version in {}", path.display());
            assert!(
                !info.raw_header.is_empty(),
                "no raw header in {}",
                path.display()
            );
        }

        // The fixture set must exercise rpm3 through rpm6 via RPMVERSION.
        assert!(
            seen.contains(&RpmFormat::Rpm4),
            "missing rpm4 fixture: {:?}",
            seen
        );
        assert!(
            seen.contains(&RpmFormat::Rpm5),
            "missing rpm5 fixture: {:?}",
            seen
        );
        assert!(
            seen.contains(&RpmFormat::Rpm6),
            "missing rpm6 fixture: {:?}",
            seen
        );
    }
}
