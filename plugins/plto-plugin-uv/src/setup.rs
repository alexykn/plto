use anyhow::{Result, bail};
use plto_plugin_support::command::run_command_with_timeout;
use std::path::Path;
use std::time::Duration;

use crate::config::{PythonScope, UvConfig, UvSetup};
use crate::pyproject::{editable_install_target, ensure_readme, get_or_create_requirements_file};

pub(crate) fn setup(workdir: &Path, config: &UvConfig, timeout: Option<Duration>) -> Result<()> {
    validate(config)?;
    run_command_with_timeout(
        "uv",
        ["venv", "--python", config.python.as_str()],
        workdir,
        timeout,
    )?;

    match (config.scope, config.setup) {
        (PythonScope::Base, _) => Ok(()),
        (PythonScope::Install, UvSetup::Sync) => {
            ensure_readme(workdir)?;
            let mut args = vec!["sync".to_string()];
            extend_uv_sync_args(&mut args, config, false);
            run_command_with_timeout("uv", args, workdir, timeout)
        }
        (PythonScope::Install, UvSetup::Editable) => {
            ensure_readme(workdir)?;
            let editable_target = editable_install_target(&config.extras);
            run_command_with_timeout(
                "uv",
                ["pip", "install", "-e", editable_target.as_str()],
                workdir,
                timeout,
            )
        }
        (PythonScope::Requirements, UvSetup::Sync) => {
            let mut args = vec!["sync".to_string(), "--no-install-project".to_string()];
            extend_uv_sync_args(&mut args, config, false);
            run_command_with_timeout("uv", args, workdir, timeout)
        }
        (PythonScope::Requirements, UvSetup::Editable) => {
            let requirements = get_or_create_requirements_file(workdir)?;
            let requirements = requirements.to_string_lossy().to_string();
            run_command_with_timeout(
                "uv",
                ["pip", "install", "-r", requirements.as_str()],
                workdir,
                timeout,
            )
        }
    }
}

fn extend_uv_sync_args(args: &mut Vec<String>, config: &UvConfig, no_install_project: bool) {
    if no_install_project {
        args.push("--no-install-project".to_string());
    }
    if config.locked {
        args.push("--locked".to_string());
    }
    if config.frozen {
        args.push("--frozen".to_string());
    }
    for group in &config.groups {
        args.extend(["--group".to_string(), group.clone()]);
    }
    for extra in &config.extras {
        args.extend(["--extra".to_string(), extra.clone()]);
    }
}

fn validate(config: &UvConfig) -> Result<()> {
    if config.locked && config.frozen {
        bail!("uv locked and frozen are mutually exclusive; choose only one.");
    }

    match (config.scope, config.setup) {
        (PythonScope::Install, UvSetup::Editable) if !config.groups.is_empty() => bail!(
            "uv groups cannot be applied to editable install setup. Use setup = \"sync\" or remove groups."
        ),
        (PythonScope::Requirements, UvSetup::Editable)
            if !config.groups.is_empty() || !config.extras.is_empty() =>
        {
            bail!(
                "uv groups/extras cannot be applied to requirements-file setup. Use setup = \"sync\" or remove groups/extras."
            )
        }
        (PythonScope::Base, _) if !config.groups.is_empty() || !config.extras.is_empty() => bail!(
            "uv groups/extras require scope = \"install\" or scope = \"requirements\" with setup = \"sync\"."
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_groups_for_editable_install() {
        let config = UvConfig {
            scope: PythonScope::Install,
            groups: vec!["dev".to_string()],
            ..UvConfig::default()
        };
        assert!(validate(&config).is_err());
    }
}
