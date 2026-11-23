use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn find_rpms(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut rpms = Vec::new();
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == "rpm").unwrap_or(false) {
            rpms.push(path.to_path_buf());
        }
    }
    Ok(rpms)
}

pub fn read_file_deps(path: &Path) -> Result<HashSet<String>> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().map(|l| l.trim().to_string()).collect())
}
