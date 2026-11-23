//! rpm4 plugin (rpm.org 4.x). Uses the shared tag-based reader.

use super::universal;
use super::{RpmFormat, RpmInfo, RpmReader};
use anyhow::Result;
use std::path::Path;

pub struct Rpm4Reader;

impl RpmReader for Rpm4Reader {
    fn format(&self) -> RpmFormat {
        RpmFormat::Rpm4
    }

    fn read(&self, path: &Path) -> Result<RpmInfo> {
        universal::read_file(path)
    }
}
