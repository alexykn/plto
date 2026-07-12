use anyhow::{Context, Result, bail};
use std::fs::{create_dir_all, read_to_string, write};
use std::path::Path;
use toml::{Value, map::Map};

use crate::config::get_global_config_path;
use crate::plugins::command::{read_metadata, validate_setup_metadata};
use crate::plugins::discovery::validate_plugin_executable;
use crate::plugins::id::PluginId;

/// Registers an explicit plugin executable path in global config.
///
/// # Errors
/// Returns an error if the plugin name is invalid, global config cannot be loaded or written,
/// or the existing config shape is incompatible with a plugin registry.
pub fn register_plugin(name: &str, command: &Path) -> Result<()> {
    let plugin = PluginId::parse(name.to_string())?;
    let command = validate_plugin_executable(command)?;
    let metadata = read_metadata(&command, std::time::Duration::from_secs(30))?;
    validate_setup_metadata(&plugin, &metadata)?;
    let path = get_global_config_path()?;
    let mut root = load_global_toml(&path)?;
    let table = root
        .as_table_mut()
        .context("Global config root must be a TOML table")?;
    let registry = table
        .entry("plugin_registry".to_string())
        .or_insert_with(|| Value::Table(Map::default()))
        .as_table_mut()
        .context("[plugin_registry] must be a TOML table")?;
    let mut entry = toml::map::Map::new();
    entry.insert(
        "command".to_string(),
        Value::String(command.to_string_lossy().to_string()),
    );
    entry.insert("source".to_string(), Value::String("manual".to_string()));
    registry.insert(plugin.as_str().to_string(), Value::Table(entry));
    write_global_toml(&path, &root)
}

/// Removes an explicit plugin registry entry from global config.
///
/// # Errors
/// Returns an error if the plugin name is invalid, global config cannot be loaded or written,
/// or the existing config shape is incompatible with a plugin registry.
pub fn remove_plugin(name: &str) -> Result<()> {
    let plugin = PluginId::parse(name.to_string())?;
    let path = get_global_config_path()?;
    let mut root = load_global_toml(&path)?;
    let table = root
        .as_table_mut()
        .context("Global config root must be a TOML table")?;
    let Some(registry) = table
        .get_mut("plugin_registry")
        .and_then(Value::as_table_mut)
    else {
        return Ok(());
    };
    registry.remove(plugin.as_str());
    write_global_toml(&path, &root)
}

fn load_global_toml(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Table(Map::default()));
    }
    let raw = read_to_string(path)
        .with_context(|| format!("Could not read global config at {}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(Value::Table(Map::default()));
    }
    toml::from_str(&raw).with_context(|| format!("Invalid global config at {}", path.display()))
}

fn write_global_toml(path: &Path, root: &Value) -> Result<()> {
    let Some(parent) = path.parent() else {
        bail!("Global config path {} has no parent", path.display());
    };
    create_dir_all(parent)?;
    write(path, toml::to_string_pretty(&root)?)?;
    Ok(())
}
