use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use toml::Value as TomlValue;

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) template: TemplateConfig,
    #[serde(default)]
    pub(crate) path: PathConfig,
    #[serde(default)]
    pub(crate) plugins: BTreeMap<String, TomlValue>,
    #[serde(default)]
    pub(crate) setup: SetupConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct TemplateConfig {
    #[serde(default)]
    pub(crate) context: BTreeMap<String, JsonValue>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct PathConfig {
    #[serde(default)]
    pub(crate) replace: BTreeMap<String, PathReplacementConfig>,
    #[serde(default)]
    pub(crate) exclude: BTreeMap<String, PathExcludeConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct PathReplacementConfig {
    pub(crate) path: PathBuf,
    pub(crate) replace: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct PathExcludeConfig {
    pub(crate) path: PathBuf,
    #[serde(default)]
    pub(crate) unless: Option<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetupConfig {
    #[serde(default)]
    pub(crate) steps: Vec<SetupStepConfig>,
}

fn default_source_path() -> PathBuf {
    PathBuf::from(".")
}

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct SetupStepConfig {
    pub(crate) plugin: String,
    #[serde(default = "default_source_path")]
    pub(crate) source_path: PathBuf,
    #[serde(default)]
    pub(crate) timeout_secs: Option<u64>,
    #[serde(flatten)]
    pub(crate) config_overrides: BTreeMap<String, TomlValue>,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupConfig {
    #[serde(default)]
    pub(crate) template: TemplateConfig,
    #[serde(default)]
    pub(crate) path: PathConfig,
    #[serde(default)]
    pub(crate) plugins: BTreeMap<String, TomlValue>,
    #[serde(default)]
    pub(crate) setup: SetupConfig,
}

/// Loads template config from an explicit TOML path.
///
/// # Errors
/// Returns an error if the file is missing, unreadable, or invalid TOML.
pub(crate) fn parse_config_file(toml_path: &Path) -> Result<Config> {
    if !toml_path.exists() {
        let parent = toml_path.parent().unwrap_or_else(|| Path::new("."));
        bail!("Missing plto.toml in {}", parent.display());
    }
    let content = read_to_string(toml_path).context(format!(
        "Could not read plto toml at {}",
        toml_path.display()
    ))?;
    toml::from_str(&content).context(format!(
        "Invalid format in plto toml at {}",
        toml_path.display()
    ))
}
