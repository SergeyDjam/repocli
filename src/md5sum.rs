use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn generate(media_info_dir: &Path, files: &[String]) -> Result<()> {
    let md5_path = media_info_dir.join("MD5SUM");
    let mut md5_file =
        File::create(&md5_path).with_context(|| format!("Cannot create {}", md5_path.display()))?;
    for name in files {
        let path = media_info_dir.join(name);
        if path.exists() {
            let data = std::fs::read(&path)?;
            let digest = md5::compute(&data);
            writeln!(md5_file, "{:x}  {}", digest, name)?;
        }
    }
    Ok(())
}
