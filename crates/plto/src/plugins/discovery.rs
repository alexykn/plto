use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::config::GlobalConfig;
use crate::fs::path::expand_tilde;
use crate::plugins::id::{PluginId, executable_names};
use crate::plugins::paths::managed_plugin_bin_dir;

#[derive(Debug, Clone)]
pub(crate) enum PluginLocationKind {
    Registry,
    Managed,
    Path,
}

#[derive(Debug, Clone)]
pub(crate) struct PluginCommand {
    pub(crate) command: PathBuf,
    pub(crate) kind: PluginLocationKind,
}

pub(crate) fn resolve_plugin_command(
    global_config: &GlobalConfig,
    plugin: &PluginId,
) -> Result<PluginCommand> {
    if let Some(entry) = global_config.plugin_registry.get(plugin.as_str()) {
        let command = expand_tilde(&entry.command)?;
        return Ok(PluginCommand {
            command,
            kind: PluginLocationKind::Registry,
        });
    }

    let managed_dir = managed_plugin_bin_dir()?;
    for name in executable_names(&plugin.crate_name()) {
        let managed = managed_dir.join(name);
        if is_executable_file(&managed) {
            return Ok(PluginCommand {
                command: managed,
                kind: PluginLocationKind::Managed,
            });
        }
    }

    if let Some(path) = find_on_path(&plugin.crate_name()) {
        return Ok(PluginCommand {
            command: path,
            kind: PluginLocationKind::Path,
        });
    }

    bail!(
        "Plugin {plugin:?} was not found. Expected a {} executable in Plto's plugin dir or on PATH.",
        plugin.crate_name()
    )
}

pub(crate) fn validate_plugin_executable(path: &Path) -> Result<PathBuf> {
    let command = expand_tilde(path)?;
    let command = command
        .canonicalize()
        .with_context(|| format!("Could not resolve plugin executable {}", command.display()))?;
    if !is_executable_file(&command) {
        bail!(
            "Plugin executable {} is not an executable file",
            command.display()
        );
    }
    Ok(command)
}

pub(crate) fn discover_path_plugins() -> Vec<PathBuf> {
    let mut plugins = Vec::new();
    let Some(path) = env::var_os("PATH") else {
        return plugins;
    };

    for dir in env::split_paths(&path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if file_name.starts_with("plto-plugin-") && is_executable_file(&entry.path()) {
                plugins.push(entry.path());
            }
        }
    }
    plugins.sort();
    plugins.dedup();
    plugins
}

fn find_on_path(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        for name in executable_names(binary) {
            let candidate = dir.join(name);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        let Ok(metadata) = path.metadata() else {
            return false;
        };
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

pub(crate) fn load_global_config() -> Result<GlobalConfig> {
    let path = crate::config::get_global_config_path()?;
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }
    crate::config::parse_global_config_file(&path)
        .with_context(|| format!("Could not load global config from {}", path.display()))
}
