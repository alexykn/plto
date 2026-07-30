use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::Path;

pub(crate) struct GitCacheLock {
    file: File,
}

impl GitCacheLock {
    pub(crate) fn acquire(lock_path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .with_context(|| format!("Could not open Git cache lock {}", lock_path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("Could not lock Git cache {}", lock_path.display()))?;
        Ok(Self { file })
    }
}

impl Drop for GitCacheLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
