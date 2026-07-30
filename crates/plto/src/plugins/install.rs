use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::plugins::id::PluginId;
use crate::plugins::paths::managed_plugin_root;
use crate::process::run_status as run_process_status;

const PLUGIN_INSTALL_TIMEOUT: Duration = Duration::from_mins(10);

#[derive(Debug, Clone)]
pub enum PluginInstallBackend {
    Cargo,
    CargoPath { path: PathBuf },
    Git { url: String },
    UvTool,
    UvToolPath { path: PathBuf },
    Pipx,
    PipxPath { path: PathBuf },
}

/// Installs a plugin with the selected backend.
///
/// # Errors
/// Returns an error if the plugin name is invalid, the managed plugin directory cannot be
/// created, or the backend install command fails.
pub fn install_plugin(name: &str, backend: PluginInstallBackend) -> Result<()> {
    match backend {
        PluginInstallBackend::Cargo => install_cargo(name),
        PluginInstallBackend::CargoPath { path } => install_cargo_path(&path),
        PluginInstallBackend::Git { url } => install_git(&url),
        PluginInstallBackend::UvTool => {
            run_status(Command::new("uv").arg("tool").arg("install").arg(name))
        }
        PluginInstallBackend::UvToolPath { path } => {
            run_status(Command::new("uv").arg("tool").arg("install").arg(path))
        }
        PluginInstallBackend::Pipx => run_status(Command::new("pipx").arg("install").arg(name)),
        PluginInstallBackend::PipxPath { path } => {
            run_status(Command::new("pipx").arg("install").arg(path))
        }
    }
}

fn install_cargo(name: &str) -> Result<()> {
    let plugin = PluginId::parse(name.to_string())?;
    let crate_name = plugin.crate_name();
    let root = managed_plugin_root()?;
    std::fs::create_dir_all(&root)?;
    run_status(
        Command::new("cargo")
            .arg("install")
            .arg(crate_name)
            .arg("--root")
            .arg(root),
    )
}

fn install_cargo_path(path: &std::path::Path) -> Result<()> {
    let root = managed_plugin_root()?;
    std::fs::create_dir_all(&root)?;
    run_status(
        Command::new("cargo")
            .arg("install")
            .arg("--path")
            .arg(path)
            .arg("--root")
            .arg(root),
    )
}

fn install_git(url: &str) -> Result<()> {
    let root = managed_plugin_root()?;
    std::fs::create_dir_all(&root)?;
    run_status(
        Command::new("cargo")
            .arg("install")
            .arg("--git")
            .arg(url)
            .arg("--root")
            .arg(root),
    )
}

fn run_status(command: &mut Command) -> Result<()> {
    run_process_status(command, PLUGIN_INSTALL_TIMEOUT, "plugin install command")?;
    Ok(())
}
