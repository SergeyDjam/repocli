//! rpm3 plugin. Header layout is shared with newer formats; only payload
//! handling differs, and repocli never reads the payload.

use super::universal;
use super::{RpmFormat, RpmInfo, RpmReader};
use anyhow::Result;
use std::path::Path;

pub struct Rpm3Reader;

impl RpmReader for Rpm3Reader {
    fn format(&self) -> RpmFormat {
        RpmFormat::Rpm3
    }

    fn read(&self, path: &Path) -> Result<RpmInfo> {
        universal::read_file(path)
    }
}
