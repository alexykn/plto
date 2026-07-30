use anyhow::{Result, bail};
use plto_plugin_support::command::run_command_with_timeout;
use std::path::Path;
use std::time::Duration;

use crate::config::{CargoConfig, CargoScope, RustProjectType};

pub(crate) fn setup(workdir: &Path, config: &CargoConfig, timeout: Option<Duration>) -> Result<()> {
    validate(workdir, config)?;
    ensure_toolchain(workdir, config, timeout)?;
    add_components(workdir, config, timeout)?;
    add_targets(workdir, config, timeout)?;
    if config.cargo_init {
        cargo_init(workdir, config, timeout)?;
    }
    match config.scope {
        CargoScope::Base => Ok(()),
        CargoScope::Fetch => cargo_command(workdir, config, "fetch", timeout),
        CargoScope::Build => cargo_command(workdir, config, "build", timeout),
    }
}

fn validate(workdir: &Path, config: &CargoConfig) -> Result<()> {
    if config.cargo_init && workdir.join("Cargo.toml").exists() {
        bail!(
            "cannot run cargo init because Cargo.toml already exists; remove cargo_init or remove the manifest"
        );
    }
    Ok(())
}

fn ensure_toolchain(workdir: &Path, config: &CargoConfig, timeout: Option<Duration>) -> Result<()> {
    run_command_with_timeout(
        "rustup",
        ["toolchain", "install", config.toolchain.as_str()],
        workdir,
        timeout,
    )
}

fn add_components(workdir: &Path, config: &CargoConfig, timeout: Option<Duration>) -> Result<()> {
    if config.components.is_empty() {
        return Ok(());
    }
    let mut args = vec!["component".to_string(), "add".to_string()];
    args.extend(config.components.iter().cloned());
    args.extend(["--toolchain".to_string(), config.toolchain.clone()]);
    run_command_with_timeout("rustup", args, workdir, timeout)
}

fn add_targets(workdir: &Path, config: &CargoConfig, timeout: Option<Duration>) -> Result<()> {
    if config.targets.is_empty() {
        return Ok(());
    }
    let mut args = vec!["target".to_string(), "add".to_string()];
    args.extend(config.targets.iter().cloned());
    args.extend(["--toolchain".to_string(), config.toolchain.clone()]);
    run_command_with_timeout("rustup", args, workdir, timeout)
}

fn cargo_init(workdir: &Path, config: &CargoConfig, timeout: Option<Duration>) -> Result<()> {
    let project_type = match config.project_type {
        RustProjectType::Binary => "--bin",
        RustProjectType::Library => "--lib",
    };
    run_command_with_timeout(
        "cargo",
        [
            format!("+{}", config.toolchain),
            "init".to_string(),
            project_type.to_string(),
            "--vcs".to_string(),
            "none".to_string(),
        ],
        workdir,
        timeout,
    )
}

fn cargo_command(
    workdir: &Path,
    config: &CargoConfig,
    command: &str,
    timeout: Option<Duration>,
) -> Result<()> {
    run_command_with_timeout(
        "cargo",
        [format!("+{}", config.toolchain), command.to_string()],
        workdir,
        timeout,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_cargo_init_when_manifest_exists() {
        let dir = std::env::temp_dir().join(format!(
            "plato-cargo-plugin-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        create_dir_all(&dir).unwrap();
        write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let config = CargoConfig {
            cargo_init: true,
            ..CargoConfig::default()
        };
        assert!(validate(&dir, &config).is_err());
        remove_dir_all(dir).unwrap();
    }
}
