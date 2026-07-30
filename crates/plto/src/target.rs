use anyhow::{Context, Result, bail};
use std::fs::{create_dir_all, remove_dir_all, rename, symlink_metadata};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingTargetPolicy {
    Reject,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetCommitOutcome {
    Created,
    Replaced,
}

pub(crate) struct TargetTransaction {
    target: PathBuf,
    backup: Option<PathBuf>,
    committed: bool,
}

impl TargetTransaction {
    pub(crate) fn begin(target: PathBuf, policy: ExistingTargetPolicy) -> Result<Self> {
        let backup = match inspect_target(&target)? {
            TargetKind::Missing => None,
            TargetKind::Directory if policy == ExistingTargetPolicy::Reject => {
                bail!(
                    "Target path {} already exists. Use --force to replace it.",
                    target.display()
                );
            }
            TargetKind::Directory => Some(move_existing_target(&target)?),
            TargetKind::Symlink => {
                bail!("Target path {} must not be a symlink", target.display());
            }
            TargetKind::Other => {
                bail!(
                    "Target path {} exists but is not a directory",
                    target.display()
                );
            }
        };

        if let Err(error) = create_dir_all(&target) {
            return restore_after_begin_failure(&target, backup.as_deref(), error);
        }

        Ok(Self {
            target,
            backup,
            committed: false,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.target
    }

    pub(crate) fn commit(&mut self) -> Result<TargetCommitOutcome> {
        let outcome = if let Some(backup) = &self.backup {
            remove_dir_all(backup).with_context(|| {
                format!(
                    "Could not remove replaced target backup {}",
                    backup.display()
                )
            })?;
            self.backup = None;
            TargetCommitOutcome::Replaced
        } else {
            TargetCommitOutcome::Created
        };
        self.committed = true;
        Ok(outcome)
    }

    pub(crate) fn rollback(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }

        remove_dir_if_present(&self.target)?;
        if let Some(backup) = self.backup.take() {
            rename(&backup, &self.target).with_context(|| {
                format!(
                    "Could not restore original target from {} to {}",
                    backup.display(),
                    self.target.display()
                )
            })?;
        }
        self.committed = true;
        Ok(())
    }

    pub(crate) fn rollback_error(&mut self, operation_error: anyhow::Error) -> anyhow::Error {
        match self.rollback() {
            Ok(()) => operation_error,
            Err(rollback_error) => rollback_error.context(format!(
                "Project initialization failed: {operation_error:#}. The original target may remain in a backup directory."
            )),
        }
    }
}

impl Drop for TargetTransaction {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        if let Err(error) = self.rollback() {
            eprintln!(
                "Failed to roll back target {} after an unfinished initialization: {error:#}",
                self.target.display()
            );
        }
    }
}

enum TargetKind {
    Missing,
    Directory,
    Symlink,
    Other,
}

fn inspect_target(path: &Path) -> Result<TargetKind> {
    match symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Ok(TargetKind::Symlink),
        Ok(metadata) if metadata.is_dir() => Ok(TargetKind::Directory),
        Ok(_) => Ok(TargetKind::Other),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(TargetKind::Missing),
        Err(error) => {
            Err(error).with_context(|| format!("Could not inspect target {}", path.display()))
        }
    }
}

fn move_existing_target(target: &Path) -> Result<PathBuf> {
    let backup = next_backup_path(target)?;
    rename(target, &backup).with_context(|| {
        format!(
            "Could not move existing target {} to temporary backup {}",
            target.display(),
            backup.display()
        )
    })?;
    Ok(backup)
}

fn next_backup_path(target: &Path) -> Result<PathBuf> {
    let parent = target
        .parent()
        .context("Target path must have a parent directory")?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .context("Target path must have a valid UTF-8 file name")?;

    for _ in 0..100 {
        let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".plto-{name}-backup-{}-{counter}",
            std::process::id()
        ));
        if matches!(inspect_target(&candidate)?, TargetKind::Missing) {
            return Ok(candidate);
        }
    }
    bail!(
        "Could not allocate a unique backup path for {}",
        target.display()
    )
}

fn restore_after_begin_failure(
    target: &Path,
    backup: Option<&Path>,
    create_error: std::io::Error,
) -> Result<TargetTransaction> {
    let Some(backup) = backup else {
        return Err(create_error)
            .with_context(|| format!("Could not create target {}", target.display()));
    };

    let restore_result = rename(backup, target);
    match restore_result {
        Ok(()) => Err(create_error)
            .with_context(|| format!("Could not create target {}", target.display())),
        Err(restore_error) => Err(restore_error).context(format!(
            "Could not create target {}: {create_error}. Original target remains at {}",
            target.display(),
            backup.display()
        )),
    }
}

fn remove_dir_if_present(path: &Path) -> Result<()> {
    match remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("Could not remove failed target {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{read_to_string, write};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("plto-target-{label}-{unique}"));
        create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn replacement_removes_stale_files_after_commit() {
        let root = temp_root("replace");
        let target = root.join("project");
        create_dir_all(&target).unwrap();
        write(target.join("stale.txt"), "stale").unwrap();

        let mut transaction =
            TargetTransaction::begin(target.clone(), ExistingTargetPolicy::Replace).unwrap();
        write(transaction.path().join("new.txt"), "new").unwrap();
        assert_eq!(transaction.commit().unwrap(), TargetCommitOutcome::Replaced);

        assert!(!target.join("stale.txt").exists());
        assert_eq!(read_to_string(target.join("new.txt")).unwrap(), "new");
        remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_restores_original_target_after_rollback() {
        let root = temp_root("rollback");
        let target = root.join("project");
        create_dir_all(&target).unwrap();
        write(target.join("sentinel.txt"), "keep").unwrap();

        let mut transaction =
            TargetTransaction::begin(target.clone(), ExistingTargetPolicy::Replace).unwrap();
        write(transaction.path().join("new.txt"), "new").unwrap();
        transaction.rollback().unwrap();

        assert_eq!(read_to_string(target.join("sentinel.txt")).unwrap(), "keep");
        assert!(!target.join("new.txt").exists());
        remove_dir_all(root).unwrap();
    }
}
