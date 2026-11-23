use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LockGuard {
    file: File,
    path: PathBuf,
}

impl LockGuard {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn global_lock(lock_file: &Path) -> Result<LockGuard> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(lock_file)
        .with_context(|| format!("Cannot open lock file {}", lock_file.display()))?;
    file.try_lock_exclusive()
        .with_context(|| format!("Another repocli already running on {}", lock_file.display()))?;
    Ok(LockGuard {
        file,
        path: lock_file.to_path_buf(),
    })
}
