use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};

use crate::config::{GlobalConfig, TemplateEntry};
use crate::fs::path::expand_tilde;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemplateKind {
    Path,
    Git,
}

impl Display for TemplateKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path => write!(f, "path"),
            Self::Git => write!(f, "git"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateRecord {
    pub(crate) name: String,
    pub(crate) entry: TemplateEntry,
    pub(crate) config_override: Option<PathBuf>,
}

impl TemplateRecord {
    pub(crate) fn kind(&self) -> TemplateKind {
        match &self.entry {
            TemplateEntry::Path { .. } => TemplateKind::Path,
            TemplateEntry::Git { .. } => TemplateKind::Git,
        }
    }

    pub(crate) fn override_path(&self) -> Option<&Path> {
        self.config_override.as_deref()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateRegistry {
    records: BTreeMap<String, TemplateRecord>,
}

impl TemplateRegistry {
    pub(crate) fn from_config(config: &GlobalConfig) -> Self {
        let records = config
            .templates
            .iter()
            .map(|(name, entry)| {
                let config_override = config
                    .template_configs
                    .get(name)
                    .map(|path| expand_tilde(path).unwrap_or_else(|_| path.clone()));
                (
                    name.clone(),
                    TemplateRecord {
                        name: name.clone(),
                        entry: entry.clone(),
                        config_override,
                    },
                )
            })
            .collect();
        Self { records }
    }

    pub(crate) fn get(&self, name: &str) -> Option<&TemplateRecord> {
        self.records.get(name)
    }

    pub(crate) fn list(&self, verbose: bool) -> String {
        use std::fmt::Write;

        let mut output = String::new();
        let max_length = self.records.keys().map(String::len).max().unwrap_or(0);

        for record in self.records.values() {
            let config_status = if record.config_override.is_some() {
                "override"
            } else {
                "source/default"
            };
            let _ = writeln!(
                output,
                " - {name:<max_length$} | {kind:<4} | {config_status}",
                name = record.name,
                kind = record.kind().to_string(),
                config_status = config_status
            );
            if verbose {
                output.push_str(&format_verbose_record(record));
            }
        }
        output
    }
}

fn format_verbose_record(record: &TemplateRecord) -> String {
    use std::fmt::Write;

    let mut output = String::new();
    match &record.entry {
        TemplateEntry::Path { path } => {
            let _ = writeln!(output, "   path: {}", path.display());
        }
        TemplateEntry::Git { git, rev, subpath } => {
            let _ = writeln!(output, "   git: {git}");
            if let Some(rev) = rev {
                let _ = writeln!(output, "   rev: {rev}");
            }
            if let Some(subpath) = subpath {
                let _ = writeln!(output, "   subpath: {}", subpath.display());
            }
        }
    }
    if let Some(config_override) = &record.config_override {
        let _ = writeln!(output, "   config: {}", config_override.display());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TemplateEntry;
    use std::collections::HashMap;

    #[test]
    fn lists_templates_in_stable_order() {
        let config = GlobalConfig {
            templates: HashMap::from([
                (
                    "zed".to_string(),
                    TemplateEntry::Path {
                        path: PathBuf::from("/tmp/zed"),
                    },
                ),
                (
                    "api".to_string(),
                    TemplateEntry::Git {
                        git: "github:owner/repo".to_string(),
                        rev: None,
                        subpath: None,
                    },
                ),
            ]),
            ..Default::default()
        };
        let registry = TemplateRegistry::from_config(&config);
        let output = registry.list(false);
        assert!(output.find("api").unwrap() < output.find("zed").unwrap());
    }

    #[test]
    fn verbose_list_includes_source_details() {
        let mut config = GlobalConfig::default();
        config.templates.insert(
            "api".to_string(),
            TemplateEntry::Git {
                git: "github:owner/repo".to_string(),
                rev: Some("main".to_string()),
                subpath: Some(PathBuf::from("templates/api")),
            },
        );
        config
            .template_configs
            .insert("api".to_string(), PathBuf::from("~/api.toml"));
        let output = TemplateRegistry::from_config(&config).list(true);
        assert!(output.contains("git: github:owner/repo"));
        assert!(output.contains("rev: main"));
        assert!(output.contains("subpath: templates/api"));
        assert!(output.contains("config: "));
    }
}
