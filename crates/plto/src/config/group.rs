use anyhow::{Context, Result, bail};
use std::fs::read_to_string;
use std::path::Path;
use toml::Value as TomlValue;

use crate::config::{Config, GroupConfig};

pub(crate) fn apply_group_configs(
    config: &mut Config,
    template_root: &Path,
    groups: &[String],
) -> Result<()> {
    for group in groups {
        validate_group_name(group)?;
        let group_config_path = template_root.join(format!("plto.{group}.toml"));
        if !group_config_path.exists() {
            bail!(
                "Template group {group:?} was requested but {} does not exist",
                group_config_path.display()
            );
        }

        let group_config = parse_group_config_file(&group_config_path)?;
        merge_group_config(config, group_config);
    }
    Ok(())
}

fn validate_group_name(group: &str) -> Result<()> {
    if group.is_empty()
        || !group
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-'))
    {
        bail!(
            "Invalid template group {group:?}: group names may contain only ASCII letters, numbers, '_' and '-'"
        );
    }
    Ok(())
}

fn parse_group_config_file(path: &Path) -> Result<GroupConfig> {
    let content = read_to_string(path)
        .with_context(|| format!("Could not read template group config at {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Invalid template group config at {}", path.display()))
}

fn merge_group_config(config: &mut Config, group: GroupConfig) {
    config.template.context.extend(group.template.context);
    config.path.replace.extend(group.path.replace);
    config.path.exclude.extend(group.path.exclude);
    for (plugin, plugin_config) in group.plugins {
        match config.plugins.get_mut(&plugin) {
            Some(existing) => merge_toml_values(existing, plugin_config),
            None => {
                config.plugins.insert(plugin, plugin_config);
            }
        }
    }
    config.setup.steps.extend(group.setup.steps);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PathExcludeConfig;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("plto-group-{label}-{unique}"));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn applies_group_configs_in_order() {
        let root = make_temp_dir("merge");
        fs::write(
            root.join("plto.docker.toml"),
            r#"
            [template.context]
            docker = true
            port = 8080

            [path.exclude]
            compose = { path = "compose.yml", unless = "docker" }
            "#,
        )
        .unwrap();
        fs::write(
            root.join("plto.override.toml"),
            r"
            [template.context]
            port = 9000
            ",
        )
        .unwrap();

        let mut config = Config::default();
        apply_group_configs(
            &mut config,
            &root,
            &["docker".to_string(), "override".to_string()],
        )
        .unwrap();

        assert_eq!(config.template.context["docker"], Value::Bool(true));
        assert_eq!(config.template.context["port"], Value::from(9000));
        assert!(matches!(
            config.path.exclude.get("compose"),
            Some(PathExcludeConfig { unless: Some(unless), .. }) if unless == "docker"
        ));
    }

    #[test]
    fn rejects_invalid_group_names() {
        let root = make_temp_dir("invalid");
        let mut config = Config::default();

        let error =
            apply_group_configs(&mut config, &root, &["../docker".to_string()]).unwrap_err();

        assert!(error.to_string().contains("Invalid template group"));
    }

    #[test]
    fn rejects_missing_group_file() {
        let root = make_temp_dir("missing");
        let mut config = Config::default();

        let error = apply_group_configs(&mut config, &root, &["docker".to_string()]).unwrap_err();

        assert!(error.to_string().contains("does not exist"));
    }
}
