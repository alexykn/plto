use anyhow::{Context, Result};
use std::fs::{create_dir_all, read, write};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use super::content::{EntryContent, WorkspaceMap};

pub(crate) struct RenderedWorkspace {
    files: WorkspaceMap,
}

impl RenderedWorkspace {
    pub(crate) fn new(files: WorkspaceMap) -> Result<Self> {
        if files
            .values()
            .any(|entry| matches!(entry.content, EntryContent::Template(_)))
        {
            anyhow::bail!("Workspace contains an unrendered template");
        }
        Ok(Self { files })
    }

    pub(crate) fn contains_directory(&self, path: &Path) -> bool {
        let path = normalize_directory_path(path);
        if path.as_os_str().is_empty() {
            return true;
        }

        if self
            .files
            .get(&path)
            .is_some_and(|entry| matches!(entry.content, EntryContent::Directory))
        {
            return true;
        }

        self.files
            .keys()
            .any(|candidate| candidate != &path && candidate.starts_with(&path))
    }

    pub(crate) fn write_to(&self, target: &Path) -> Result<WorkspaceWriteSummary> {
        let mut summary = WorkspaceWriteSummary::default();
        let mut directories = Vec::new();
        for (path, entry) in &self.files {
            let full_path = target.join(path);
            match &entry.content {
                EntryContent::BinaryLazy {
                    path: source_path,
                    cache,
                } => {
                    if cache.get().is_none() {
                        let bytes = read(source_path).map(Rc::<[u8]>::from).with_context(|| {
                            format!("Failed to read binary file {}", source_path.display())
                        })?;
                        let _ = cache.set(bytes);
                    }
                    let bytes = cache
                        .get()
                        .context("Binary cache was not initialized after read")?;
                    write_file(&full_path, bytes.as_ref(), entry)?;
                    summary.files_written += 1;
                }
                EntryContent::Rendered(bytes) => {
                    write_file(&full_path, bytes.as_ref(), entry)?;
                    summary.files_written += 1;
                }
                EntryContent::Directory => {
                    create_dir_all(&full_path)?;
                    directories.push((full_path, entry));
                    summary.directories_created += 1;
                }
                EntryContent::Template(_) => unreachable!("RenderedWorkspace rejects templates"),
            }
        }
        for (path, entry) in directories {
            entry.apply_permissions(&path)?;
        }
        Ok(summary)
    }
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceWriteSummary {
    pub(crate) files_written: usize,
    pub(crate) directories_created: usize,
}

fn write_file(path: &Path, bytes: &[u8], entry: &super::content::WorkspaceEntry) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    write(path, bytes)?;
    entry.apply_permissions(path)
}

fn normalize_directory_path(path: &Path) -> PathBuf {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(PathBuf::from(value)),
            _ => None,
        })
        .fold(PathBuf::new(), |mut normalized, component| {
            normalized.push(component);
            normalized
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::content::WorkspaceEntry;

    #[test]
    fn detects_rendered_directories_from_descendants() {
        let workspace = RenderedWorkspace::new(WorkspaceMap::from([(
            PathBuf::from("backend/pyproject.toml"),
            WorkspaceEntry::rendered(Rc::<[u8]>::from(Vec::<u8>::new())),
        )]))
        .unwrap();

        assert!(workspace.contains_directory(Path::new(".")));
        assert!(workspace.contains_directory(Path::new("backend")));
        assert!(workspace.contains_directory(Path::new("./backend")));
        assert!(!workspace.contains_directory(Path::new("frontend")));
        assert!(!workspace.contains_directory(Path::new("backend/pyproject.toml")));
    }
}
