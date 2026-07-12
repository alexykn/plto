use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use crate::config::{
    Config, GlobalConfig, TemplateEntry, get_global_config_path, parse_global_config_file,
};
use crate::fs::path::expand_tilde;
use crate::source::git::{GitTemplateFetcher, TempCheckout, merge_git_template_spec};
use crate::source::registry::TemplateRegistry;
use crate::source::selection::{config_path_for, select_ad_hoc_config, select_named_config};

#[derive(Debug, Clone)]
pub(crate) enum TemplateRequest {
    Named {
        name: String,
        cli_rev: Option<String>,
        cli_subpath: Option<PathBuf>,
    },
    Git {
        spec: String,
        cli_rev: Option<String>,
        cli_subpath: Option<PathBuf>,
    },
    Path {
        path: PathBuf,
    },
}

pub(crate) struct PreparedTemplateSource {
    pub(crate) source_path: PathBuf,
    pub(crate) config: Config,
    pub(crate) cleanup: Option<TempCheckout>,
}

pub(crate) struct TemplateResolver {
    global_config: GlobalConfig,
    registry: TemplateRegistry,
}

impl TemplateResolver {
    pub(crate) fn from_global_config() -> Result<Self> {
        let config_path = get_global_config_path()?;
        let global_config = if config_path.exists() {
            parse_global_config_file(&config_path).with_context(|| {
                format!(
                    "Could not load global config from {}",
                    config_path.display()
                )
            })?
        } else {
            eprintln!(
                "WARNING: Global config {} does not exist. Using default configuration.",
                config_path.display()
            );
            GlobalConfig::default()
        };
        Ok(Self::new(global_config))
    }

    pub(crate) fn new(global_config: GlobalConfig) -> Self {
        let registry = TemplateRegistry::from_config(&global_config);
        Self {
            global_config,
            registry,
        }
    }

    pub(crate) fn prepare(&self, request: TemplateRequest) -> Result<PreparedTemplateSource> {
        match request {
            TemplateRequest::Named {
                name,
                cli_rev,
                cli_subpath,
            } => self.prepare_named(&name, cli_rev.as_deref(), cli_subpath.as_deref()),
            TemplateRequest::Git {
                spec,
                cli_rev,
                cli_subpath,
            } => self.prepare_ad_hoc_git(&spec, cli_rev.as_deref(), cli_subpath.as_deref()),
            TemplateRequest::Path { path } => Self::prepare_ad_hoc_path(&path),
        }
    }

    pub(crate) fn config_path_for(&self, template_name: &str) -> Result<PathBuf> {
        config_path_for(&self.registry, template_name)
    }

    pub(crate) fn format_templates(&self, verbose: bool) -> String {
        self.registry.list(verbose)
    }

    fn prepare_named(
        &self,
        name: &str,
        cli_rev: Option<&str>,
        cli_subpath: Option<&Path>,
    ) -> Result<PreparedTemplateSource> {
        let Some(record) = self.registry.get(name) else {
            bail!("No configured template found for {name:?}");
        };

        match &record.entry {
            TemplateEntry::Path { path } => {
                if cli_rev.is_some() || cli_subpath.is_some() {
                    bail!("--rev and --subpath cannot be used with local template {name:?}");
                }
                let source_path = expand_tilde(path)?;
                let config = select_named_config(name, &source_path, record)?;
                Ok(PreparedTemplateSource {
                    source_path,
                    config,
                    cleanup: None,
                })
            }
            TemplateEntry::Git { git, rev, subpath } => {
                let spec = merge_git_template_spec(
                    git,
                    &self.global_config,
                    rev.as_deref(),
                    subpath.as_deref(),
                    cli_rev,
                    cli_subpath,
                )?;
                let fetcher = GitTemplateFetcher::from_user_cache_dir()?;
                let checkout = fetcher.prepare_checkout(&spec)?;
                let config = select_named_config(name, &checkout.source_path, record)?;
                let source_path = checkout.source_path.clone();
                Ok(PreparedTemplateSource {
                    source_path,
                    config,
                    cleanup: Some(checkout.into_cleanup()),
                })
            }
        }
    }

    fn prepare_ad_hoc_git(
        &self,
        spec: &str,
        cli_rev: Option<&str>,
        cli_subpath: Option<&Path>,
    ) -> Result<PreparedTemplateSource> {
        let spec =
            merge_git_template_spec(spec, &self.global_config, None, None, cli_rev, cli_subpath)?;
        let fetcher = GitTemplateFetcher::from_user_cache_dir()?;
        let checkout = fetcher.prepare_checkout(&spec)?;
        let config = select_ad_hoc_config(&checkout.source_path, "ad-hoc Git template")?;
        let source_path = checkout.source_path.clone();
        Ok(PreparedTemplateSource {
            source_path,
            config,
            cleanup: Some(checkout.into_cleanup()),
        })
    }

    fn prepare_ad_hoc_path(path: &Path) -> Result<PreparedTemplateSource> {
        let source_path = expand_tilde(path)?;
        let config = select_ad_hoc_config(&source_path, "--path template")?;
        Ok(PreparedTemplateSource {
            source_path,
            config,
            cleanup: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GlobalConfig, TemplateEntry};
    use std::collections::HashMap;

    #[test]
    fn named_template_does_not_infer_git() {
        let resolver = TemplateResolver::new(GlobalConfig::default());
        let result = resolver.prepare(TemplateRequest::Named {
            name: "owner/repo".to_string(),
            cli_rev: None,
            cli_subpath: None,
        });
        let Err(error) = result else {
            panic!("expected named owner/repo to fail without --git");
        };
        assert!(error.to_string().contains("No configured template"));
    }

    #[test]
    fn config_path_uses_override_for_remote() {
        let config = GlobalConfig {
            templates: HashMap::from([(
                "api".to_string(),
                TemplateEntry::Git {
                    git: "github:owner/repo".to_string(),
                    rev: None,
                    subpath: None,
                },
            )]),
            template_configs: HashMap::from([("api".to_string(), PathBuf::from("~/api.toml"))]),
            ..Default::default()
        };
        let path = TemplateResolver::new(config)
            .config_path_for("api")
            .unwrap();
        assert!(path.ends_with("api.toml"));
    }
}
