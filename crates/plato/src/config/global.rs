use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use crate::plugins::id::PluginId;

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GitProvider {
    #[default]
    Github,
    Gitlab,
    Bitbucket,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct GitHostsConfig {
    #[serde(default)]
    pub(crate) github: Option<String>,
    #[serde(default)]
    pub(crate) gitlab: Option<String>,
    #[serde(default)]
    pub(crate) bitbucket: Option<String>,
}

impl GitHostsConfig {
    pub(crate) fn get(&self, provider: GitProvider) -> Option<&str> {
        match provider {
            GitProvider::Github => self.github.as_deref(),
            GitProvider::Gitlab => self.gitlab.as_deref(),
            GitProvider::Bitbucket => self.bitbucket.as_deref(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum TemplateEntry {
    Path {
        path: PathBuf,
    },
    Git {
        git: String,
        rev: Option<String>,
        subpath: Option<PathBuf>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathTemplateEntry {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitTemplateEntry {
    git: String,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    subpath: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TemplateEntryDefinition {
    Path(PathTemplateEntry),
    Git(GitTemplateEntry),
}

impl<'de> Deserialize<'de> for TemplateEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match TemplateEntryDefinition::deserialize(deserializer)? {
            TemplateEntryDefinition::Path(entry) => Ok(Self::Path { path: entry.path }),
            TemplateEntryDefinition::Git(entry) => Ok(Self::Git {
                git: entry.git,
                rev: entry.rev,
                subpath: entry.subpath,
            }),
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlobalConfig {
    #[serde(default)]
    pub(crate) plato: GlobalPlatoConfig,
    #[serde(default)]
    pub(crate) templates: HashMap<String, TemplateEntry>,
    #[serde(default)]
    pub(crate) template_configs: HashMap<String, PathBuf>,
    #[serde(default)]
    pub(crate) plugin_registry: HashMap<String, PluginRegistryEntry>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginRegistryEntry {
    pub(crate) command: PathBuf,
    #[serde(default)]
    pub(crate) source: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlobalPlatoConfig {
    #[serde(default)]
    pub(crate) default_git_provider: GitProvider,
    #[serde(default)]
    pub(crate) git_hosts: GitHostsConfig,
}

impl GlobalConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        for name in self.templates.keys() {
            if name.trim().is_empty() {
                bail!("Template names must not be empty");
            }
        }
        for name in self.template_configs.keys() {
            if !self.templates.contains_key(name) {
                bail!("[template_configs] entry {name:?} has no matching [templates] entry");
            }
        }
        for (name, entry) in &self.plugin_registry {
            PluginId::parse(name.clone())?;
            if entry.command.as_os_str().is_empty() {
                bail!("Plugin registry command for {name:?} must not be empty");
            }
        }
        validate_git_hosts(&self.plato.git_hosts)
    }
}

fn validate_git_hosts(hosts: &GitHostsConfig) -> Result<()> {
    for (name, value) in [
        ("github", hosts.github.as_deref()),
        ("gitlab", hosts.gitlab.as_deref()),
        ("bitbucket", hosts.bitbucket.as_deref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            bail!("Git host override {name:?} must not be empty");
        }
    }
    Ok(())
}

/// Returns the directory that stores Plato's configuration files.
///
/// # Errors
/// Returns an error if the user's home directory cannot be determined.
pub(crate) fn get_global_plato_dir() -> Result<PathBuf> {
    let base_dirs = BaseDirs::new().context("Could not find home directory")?;
    Ok(base_dirs.home_dir().join(".config/plato"))
}

/// Returns the global Plato config path.
///
/// # Errors
/// Returns an error if the user's home directory cannot be determined.
pub(crate) fn get_global_config_path() -> Result<PathBuf> {
    Ok(get_global_plato_dir()?.join("config.toml"))
}

/// Loads global config from an explicit TOML path.
///
/// # Errors
/// Returns an error if the file is unreadable or invalid TOML.
pub(crate) fn parse_global_config_file(toml_path: &Path) -> Result<GlobalConfig> {
    let content = read_to_string(toml_path).context(format!(
        "Could not read global config at {}",
        toml_path.display()
    ))?;
    let config: GlobalConfig = toml::from_str(&content).context(format!(
        "Invalid format in global config at {}",
        toml_path.display()
    ))?;
    config.validate()?;
    Ok(config)
}
