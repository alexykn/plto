use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::config::PathReplacementConfig;
use crate::context::TemplateContext;
use crate::fs::path::reject_parent_components;
use crate::rendering::new_template_environment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourcePathKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub(super) struct SourcePathEntry {
    pub(super) path: PathBuf,
    pub(super) kind: SourcePathKind,
}

#[derive(Debug, Clone)]
struct PathRewriteRule {
    name: String,
    source: PathBuf,
    target: PathBuf,
    source_kind: SourcePathKind,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PathRewritePlan {
    rules: Vec<PathRewriteRule>,
}

impl PathRewritePlan {
    pub(super) fn from_config(
        replacements: &BTreeMap<String, PathReplacementConfig>,
        context: &TemplateContext,
        source_entries: &[SourcePathEntry],
    ) -> Result<Self> {
        if replacements.is_empty() {
            return Ok(Self::default());
        }

        let source_lookup = source_entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.kind))
            .collect::<BTreeMap<_, _>>();
        let env = new_template_environment();
        let mut rules = Vec::with_capacity(replacements.len());

        for (name, replacement) in replacements {
            validate_source_path(name, &replacement.path)?;
            let Some(source_kind) = source_lookup.get(&replacement.path).copied() else {
                bail!(
                    "Invalid [path.replace.{name}]: source path {:?} does not exist in the template",
                    replacement.path.display().to_string()
                );
            };

            let rendered_target =
                env.render_str(&replacement.replace, context)
                    .with_context(|| {
                        format!(
                            "Invalid [path.replace.{name}]: failed to render replacement {:?}",
                            replacement.replace
                        )
                    })?;
            let target = PathBuf::from(rendered_target);
            validate_target_path(name, &target)?;

            rules.push(PathRewriteRule {
                name: name.clone(),
                source: replacement.path.clone(),
                target,
                source_kind,
            });
        }

        reject_duplicate_sources(&rules)?;
        reject_overlapping_sources(&rules)?;

        Ok(Self { rules })
    }

    pub(super) fn rewrite(&self, rel_path: &Path) -> PathBuf {
        for rule in &self.rules {
            if rule.source_kind == SourcePathKind::File {
                if rel_path == rule.source {
                    return rule.target.clone();
                }
                continue;
            }

            if rel_path == rule.source {
                return rule.target.clone();
            }

            if let Ok(suffix) = rel_path.strip_prefix(&rule.source) {
                return rule.target.join(suffix);
            }
        }

        rel_path.to_path_buf()
    }
}

fn validate_source_path(name: &str, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("Invalid [path.replace.{name}]: path must not be empty");
    }
    reject_parent_components(path, &format!("Invalid [path.replace.{name}]: path"))
}

fn validate_target_path(name: &str, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("Invalid [path.replace.{name}]: rendered replacement must not be empty");
    }
    reject_parent_components(
        path,
        &format!("Invalid [path.replace.{name}]: rendered replacement"),
    )
}

fn reject_duplicate_sources(rules: &[PathRewriteRule]) -> Result<()> {
    let mut seen = BTreeMap::<&Path, &str>::new();
    for rule in rules {
        if let Some(existing_name) = seen.insert(rule.source.as_path(), &rule.name) {
            bail!(
                "Invalid path replacements: [path.replace.{existing_name}] and [path.replace.{}] both target {:?}",
                rule.name,
                rule.source.display().to_string()
            );
        }
    }
    Ok(())
}

fn reject_overlapping_sources(rules: &[PathRewriteRule]) -> Result<()> {
    let mut checked = HashSet::<(&str, &str)>::new();
    for left in rules {
        for right in rules {
            if left.name == right.name {
                continue;
            }
            if !checked.insert((&left.name, &right.name))
                || !checked.insert((&right.name, &left.name))
            {
                continue;
            }
            if left.source.starts_with(&right.source) || right.source.starts_with(&left.source) {
                bail!(
                    "Invalid path replacements: [path.replace.{}] path {:?} overlaps [path.replace.{}] path {:?}",
                    left.name,
                    left.source.display().to_string(),
                    right.name,
                    right.source.display().to_string()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::TemplateContext;
    use serde_json::Value;

    fn context() -> TemplateContext {
        let mut context = TemplateContext::new();
        context.merge(BTreeMap::from([
            (
                "package_name".to_string(),
                Value::String("py3-requests".to_string()),
            ),
            (
                "package_deps".to_string(),
                Value::String("deps-runtime".to_string()),
            ),
        ]));
        context
    }

    fn replacement(path: &str, replace: &str) -> PathReplacementConfig {
        PathReplacementConfig {
            path: PathBuf::from(path),
            replace: replace.to_string(),
        }
    }

    fn entries() -> Vec<SourcePathEntry> {
        vec![
            SourcePathEntry {
                path: PathBuf::from("src/py3-something"),
                kind: SourcePathKind::Directory,
            },
            SourcePathEntry {
                path: PathBuf::from("src/py3-something/__init__.py.j2"),
                kind: SourcePathKind::File,
            },
            SourcePathEntry {
                path: PathBuf::from("deps/funny"),
                kind: SourcePathKind::Directory,
            },
            SourcePathEntry {
                path: PathBuf::from("README-template.md.j2"),
                kind: SourcePathKind::File,
            },
        ]
    }

    #[test]
    fn rewrites_exact_directory_and_descendants() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement(
                "src/py3-something",
                "src/{{ package_name | regex_replace('^py3-', '') }}",
            ),
        )]);
        let plan = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap();

        assert_eq!(
            plan.rewrite(Path::new("src/py3-something")),
            PathBuf::from("src/requests")
        );
        assert_eq!(
            plan.rewrite(Path::new("src/py3-something/__init__.py.j2")),
            PathBuf::from("src/requests/__init__.py.j2")
        );
    }

    #[test]
    fn rewrites_exact_file() {
        let rules = BTreeMap::from([(
            "readme".to_string(),
            replacement("README-template.md.j2", "README.md.j2"),
        )]);
        let plan = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap();

        assert_eq!(
            plan.rewrite(Path::new("README-template.md.j2")),
            PathBuf::from("README.md.j2")
        );
    }

    #[test]
    fn leaves_unrelated_paths_unchanged() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("src/py3-something", "src/requests"),
        )]);
        let plan = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap();

        assert_eq!(
            plan.rewrite(Path::new("docs/guide.md")),
            PathBuf::from("docs/guide.md")
        );
    }

    #[test]
    fn supports_regex_filters_in_replacements() {
        let rules = BTreeMap::from([(
            "deps".to_string(),
            replacement(
                "deps/funny",
                "deps/{{ package_deps | regex_replace('^deps', 'stuff') }}",
            ),
        )]);
        let plan = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap();

        assert_eq!(
            plan.rewrite(Path::new("deps/funny")),
            PathBuf::from("deps/stuff-runtime")
        );
    }

    #[test]
    fn rejects_missing_source() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("src/missing", "src/requests"),
        )]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("source path"));
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn rejects_absolute_source_path() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("/src/pkg", "src/requests"),
        )]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("must be relative"));
    }

    #[test]
    fn rejects_parent_components_in_source_path() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("src/../pkg", "src/requests"),
        )]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("'..'"));
    }

    #[test]
    fn rejects_absolute_rendered_target() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("src/py3-something", "/src/requests"),
        )]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("must be relative"));
    }

    #[test]
    fn rejects_parent_components_in_rendered_target() {
        let rules = BTreeMap::from([(
            "source".to_string(),
            replacement("src/py3-something", "../requests"),
        )]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("'..'"));
    }

    #[test]
    fn rejects_duplicate_source_paths() {
        let rules = BTreeMap::from([
            (
                "first".to_string(),
                replacement("src/py3-something", "src/requests"),
            ),
            (
                "second".to_string(),
                replacement("src/py3-something", "src/other"),
            ),
        ]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("both target"));
    }

    #[test]
    fn rejects_overlapping_source_paths() {
        let rules = BTreeMap::from([
            (
                "parent".to_string(),
                replacement("src/py3-something", "src/requests"),
            ),
            (
                "child".to_string(),
                replacement("src/py3-something/__init__.py.j2", "src/init.py.j2"),
            ),
        ]);

        let error = PathRewritePlan::from_config(&rules, &context(), &entries()).unwrap_err();
        assert!(error.to_string().contains("overlaps"));
    }
}
