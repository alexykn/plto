use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{metadata, read_to_string};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;
use walkdir::WalkDir;

use crate::config::{PathExcludeConfig, PathReplacementConfig};
use crate::context::TemplateContext;
use crate::rendering::new_template_environment;
use crate::workspace::content::{EntryContent, WorkspaceEntry, WorkspaceMap};
use crate::workspace::path_exclude::apply_path_excludes;
use crate::workspace::path_rewrite::{PathRewritePlan, SourcePathEntry, SourcePathKind};
use crate::workspace::rendered::RenderedWorkspace;

fn deduplicate_dirmap(map: &mut WorkspaceMap) {
    let all_paths: Vec<PathBuf> = map.keys().cloned().collect();
    map.retain(|path, content| {
        if !matches!(content.content, EntryContent::Directory) {
            return true;
        }
        let has_children = all_paths
            .iter()
            .any(|other| other != path && other.starts_with(path));
        !has_children
    });
}

pub(crate) struct WorkspaceBuilder {
    content: WorkspaceMap,
}

impl WorkspaceBuilder {
    pub(crate) fn from_source(source_path: &Path) -> Result<Self> {
        if !source_path.is_dir() {
            bail!(
                "Template source {} is not a directory",
                source_path.display()
            );
        }

        let mut raw_map = WorkspaceMap::new();
        for entry in WalkDir::new(source_path) {
            let entry = entry.with_context(|| {
                format!(
                    "Failed to read template entry under {}",
                    source_path.display()
                )
            })?;
            let path = entry.path();
            let rel_path = path.strip_prefix(source_path)?.to_path_buf();
            if rel_path.as_os_str().is_empty() {
                continue;
            }
            if is_reserved_plato_config_path(&rel_path) {
                continue;
            }
            let file_type = entry.file_type();
            if file_type.is_symlink() {
                bail!("Template contains unsupported symlink: {}", path.display());
            }
            let metadata = metadata(path).with_context(|| {
                format!(
                    "Failed to read metadata for template entry {}",
                    path.display()
                )
            })?;
            let content = if file_type.is_dir() {
                EntryContent::Directory
            } else {
                match path.extension().and_then(|s| s.to_str()) {
                    Some("j2" | "mj") => {
                        let text = read_to_string(path).with_context(|| {
                            format!("Failed to read template {}", path.display())
                        })?;
                        EntryContent::Template(Rc::<str>::from(text))
                    }
                    _ => EntryContent::BinaryLazy {
                        path: path.to_path_buf(),
                        cache: OnceLock::new(),
                    },
                }
            };
            insert_unique(
                &mut raw_map,
                rel_path,
                WorkspaceEntry::new(content, &metadata),
                "loading template",
            )?;
        }
        Ok(Self { content: raw_map })
    }

    pub(crate) fn exclude_paths(
        mut self,
        context: &TemplateContext,
        path_excludes: &BTreeMap<String, PathExcludeConfig>,
    ) -> Result<Self> {
        apply_path_excludes(&mut self.content, context, path_excludes)?;
        deduplicate_dirmap(&mut self.content);
        Ok(self)
    }

    pub(crate) fn rewrite_paths(
        self,
        context: &TemplateContext,
        path_replacements: &BTreeMap<String, PathReplacementConfig>,
    ) -> Result<Self> {
        let source_entries = self
            .content
            .iter()
            .map(|(path, content)| SourcePathEntry {
                path: path.clone(),
                kind: if matches!(content.content, EntryContent::Directory) {
                    SourcePathKind::Directory
                } else {
                    SourcePathKind::File
                },
            })
            .collect::<Vec<_>>();
        let rewrite_plan =
            PathRewritePlan::from_config(path_replacements, context, &source_entries)?;
        let mut target_map = WorkspaceMap::new();
        for (rel_path, content) in self.content {
            let new_path = rewrite_plan.rewrite(&rel_path);
            insert_unique(&mut target_map, new_path, content, "path rewrite")?;
        }
        deduplicate_dirmap(&mut target_map);
        Ok(Self {
            content: target_map,
        })
    }

    pub(crate) fn render_templates(self, context: &impl Serialize) -> Result<Self> {
        let mut rendered_map = WorkspaceMap::new();
        let env = new_template_environment();
        for (path, content) in self.content {
            match &content.content {
                EntryContent::Template(raw_text) => {
                    let rendered = env
                        .render_str(raw_text, context)
                        .with_context(|| format!("Failed to render {}", path.display()))?;
                    let new_path = path.with_extension("");
                    insert_unique(
                        &mut rendered_map,
                        new_path,
                        content.rendered_from_template(rendered.into_bytes()),
                        "template rendering",
                    )?;
                }
                other => {
                    let _ = other;
                    insert_unique(&mut rendered_map, path, content, "template rendering")?;
                }
            }
        }

        Ok(Self {
            content: rendered_map,
        })
    }

    pub(crate) fn build(self) -> Result<RenderedWorkspace> {
        RenderedWorkspace::new(self.content)
    }
}

fn insert_unique(
    entries: &mut WorkspaceMap,
    path: PathBuf,
    entry: WorkspaceEntry,
    operation: &str,
) -> Result<()> {
    if entries.insert(path.clone(), entry).is_some() {
        bail!(
            "Output path collision during {operation}: {}",
            path.display()
        );
    }
    Ok(())
}

fn is_reserved_plato_config_path(rel_path: &Path) -> bool {
    if rel_path
        .parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty())
    {
        return false;
    }
    let Some(file_name) = rel_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name == "plato.toml"
        || (file_name.starts_with("plato.")
            && Path::new(file_name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("plato-builder-{label}-{unique}"));
        create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn identifies_root_plato_config_files_as_reserved() {
        assert!(is_reserved_plato_config_path(Path::new("plato.toml")));
        assert!(is_reserved_plato_config_path(Path::new(
            "plato.docker.toml"
        )));
    }

    #[test]
    fn does_not_reserve_nested_or_non_config_files() {
        assert!(!is_reserved_plato_config_path(Path::new(
            "groups/plato.docker.toml"
        )));
        assert!(!is_reserved_plato_config_path(Path::new("plato.template")));
    }

    #[test]
    fn rejects_binary_and_template_output_collisions() {
        let root = temp_root("collision");
        write(root.join("foo"), "binary").unwrap();
        write(root.join("foo.j2"), "template").unwrap();

        let result = WorkspaceBuilder::from_source(&root)
            .unwrap()
            .render_templates(&TemplateContext::new());
        let Err(error) = result else {
            panic!("expected an output collision");
        };

        assert!(error.to_string().contains("Output path collision"));
        remove_dir_all(root).unwrap();
    }
}
