use anyhow::Result;
use std::path::Path;

use crate::packdrakeng;
use crate::rpm::RpmInfo;

/// Build `hdlist.cz` as an MDV::Packdrakeng archive of RPM headers.
///
/// Each package is stored as a virtual file named by its full name, exactly
/// as `genhdlist2` does with `add_virtual('f', $fullname, $header)`.
pub fn build_hdlist_from_packages(packages: &[RpmInfo], output_path: &Path) -> Result<()> {
    packdrakeng::write_archive(packages, output_path)
}
