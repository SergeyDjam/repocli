//! rpm6 plugin (Fedora 46+). Header tags are unchanged; the payload may use
//! zstd, which repocli does not need to inspect.

use super::universal;
use super::{RpmFormat, RpmInfo, RpmReader};
use anyhow::Result;
use std::path::Path;

pub struct Rpm6Reader;

impl RpmReader for Rpm6Reader {
    fn format(&self) -> RpmFormat {
        RpmFormat::Rpm6
    }

    fn read(&self, path: &Path) -> Result<RpmInfo> {
        universal::read_file(path)
    }
}
