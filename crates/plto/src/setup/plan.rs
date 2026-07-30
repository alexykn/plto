use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use toml::{Value as TomlValue, map::Map};

use crate::config::{Config, SetupStepConfig};
use crate::plugins::id::PluginId;
use crate::setup::step::SetupStep;

pub(crate) const DEFAULT_PLUGIN_SETUP_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Default)]
pub(crate) struct SetupPlan {
    pub(crate) steps: Vec<SetupStep>,
}

impl SetupPlan {
    pub(crate) fn from_config(config: &Config, target_path: &Path) -> Result<Self> {
        let steps = config
            .setup
            .steps
            .iter()
            .map(|step| build_step(config, target_path, step))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { steps })
    }
}

fn build_step(config: &Config, target_path: &Path, step: &SetupStepConfig) -> Result<SetupStep> {
    let plugin = PluginId::parse(step.plugin.clone())?;
    validate_source_path(&step.source_path)?;
    let workdir = normalize_workdir(target_path, &step.source_path)?;
    let plugin_config =
        merge_plugin_config(config.plugins.get(plugin.as_str()), &step.config_overrides);
    let config = serde_json::to_value(plugin_config)?;
    let timeout = setup_timeout(step.timeout_secs)?;
    Ok(SetupStep {
        plugin,
        source_path: step.source_path.clone(),
        workdir,
        config,
        timeout,
    })
}

fn setup_timeout(timeout_secs: Option<u64>) -> Result<Duration> {
    match timeout_secs {
        Some(0) => bail!("Setup step timeout_secs must be greater than zero"),
        Some(seconds) => Ok(Duration::from_secs(seconds)),
        None => Ok(Duration::from_secs(DEFAULT_PLUGIN_SETUP_TIMEOUT_SECS)),
    }
}

fn merge_plugin_config(
    base: Option<&TomlValue>,
    overrides: &std::collections::BTreeMap<String, TomlValue>,
) -> TomlValue {
    let mut merged = base
        .cloned()
        .unwrap_or_else(|| TomlValue::Table(Map::default()));
    for (key, value) in overrides {
        set_table_value(&mut merged, key.clone(), value.clone());
    }
    merged
}

fn set_table_value(target: &mut TomlValue, key: String, value: TomlValue) {
    if let TomlValue::Table(table) = target {
        match table.get_mut(&key) {
            Some(existing) => merge_toml_values(existing, value),
            None => {
                table.insert(key, value);
            }
        }
        return;
    }

    let mut table = Map::new();
    table.insert(key, value);
    *target = TomlValue::Table(table);
}

fn merge_toml_values(target: &mut TomlValue, source: TomlValue) {
    match (target, source) {
        (TomlValue::Table(target), TomlValue::Table(source)) => {
            for (key, value) in source {
                match target.get_mut(&key) {
                    Some(existing) => merge_toml_values(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, source) => *target = source,
    }
}

fn validate_source_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        bail!("Setup step source_path {} must be relative", path.display());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "Setup step source_path {} must not contain '..'",
            path.display()
        );
    }
    Ok(())
}

fn normalize_workdir(target_path: &Path, source_path: &Path) -> Result<PathBuf> {
    let workdir = target_path.join(source_path);
    if !workdir.starts_with(target_path) {
        bail!(
            "Setup step workdir escaped target path: {}",
            workdir.display()
        );
    }
    Ok(workdir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_setup_step_timeout() {
        let config: Config = toml::from_str(
            r#"
            [[setup.steps]]
            plugin = "uv"
            timeout_secs = 12
            "#,
        )
        .unwrap();

        let plan = SetupPlan::from_config(&config, Path::new("project")).unwrap();

        assert_eq!(plan.steps[0].timeout, Duration::from_secs(12));
        assert!(plan.steps[0].config.get("timeout_secs").is_none());
    }

    #[test]
    fn rejects_zero_setup_step_timeout() {
        let config: Config = toml::from_str(
            r#"
            [[setup.steps]]
            plugin = "uv"
            timeout_secs = 0
            "#,
        )
        .unwrap();

        let error = SetupPlan::from_config(&config, Path::new("project")).unwrap_err();

        assert!(error.to_string().contains("timeout_secs"));
    }
}
