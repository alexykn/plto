use anyhow::{Context, Result};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) type WorkspaceMap = std::collections::BTreeMap<PathBuf, WorkspaceEntry>;

pub(crate) struct WorkspaceEntry {
    pub(crate) content: EntryContent,
    permissions: EntryPermissions,
}

impl WorkspaceEntry {
    pub(crate) fn new(content: EntryContent, metadata: &Metadata) -> Self {
        Self {
            content,
            permissions: EntryPermissions::from_metadata(metadata),
        }
    }

    #[cfg(test)]
    pub(crate) fn rendered(bytes: impl Into<Rc<[u8]>>) -> Self {
        Self {
            content: EntryContent::Rendered(bytes.into()),
            permissions: EntryPermissions::default(),
        }
    }

    pub(crate) fn rendered_from_template(&self, bytes: Vec<u8>) -> Self {
        Self {
            content: EntryContent::Rendered(Rc::from(bytes)),
            permissions: self.permissions,
        }
    }

    pub(crate) fn apply_permissions(&self, path: &Path) -> Result<()> {
        self.permissions.apply_to(path)
    }
}

#[derive(Clone, Copy, Default)]
struct EntryPermissions {
    #[cfg(unix)]
    mode: u32,
}

impl EntryPermissions {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            #[cfg(unix)]
            mode: metadata.permissions().mode(),
        }
    }

    fn apply_to(self, path: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(self.mode))
                .with_context(|| {
                    format!("Failed to preserve permissions for {}", path.display())
                })?;
        }
        Ok(())
    }
}

pub(crate) enum EntryContent {
    BinaryLazy {
        path: PathBuf,
        cache: OnceLock<Rc<[u8]>>,
    },
    Rendered(Rc<[u8]>),
    Template(Rc<str>),
    Directory,
}
