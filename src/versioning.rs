use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::Path;

pub fn remove_previous_versioned_symlinks(media_info_dir: &Path) -> Result<bool> {
    let mut existed = false;
    for entry in fs::read_dir(media_info_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.len() > 15
            && name.chars().nth(15) == Some('-')
            && name.contains("hdlist")
            && entry.path().is_symlink()
        {
            fs::remove_file(entry.path())?;
            existed = true;
        }
    }
    Ok(existed)
}

pub fn create_versioned_symlinks(media_info_dir: &Path, files: &[String]) -> Result<()> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    for file in files {
        let link_name = format!("{}-{}", timestamp, file);
        let src = media_info_dir.join(file);
        let dst = media_info_dir.join(&link_name);
        if src.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(file, dst)?;
            #[cfg(not(unix))]
            std::fs::copy(&src, dst)?;
        }
    }
    Ok(())
}
