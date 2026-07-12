use anyhow::{Result, bail};
use plato_plugin_support::command::run_command_with_timeout;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{PipConfig, PythonScope};
use crate::pyproject::{editable_install_target, ensure_readme, get_or_create_requirements_file};

pub(crate) fn setup(workdir: &Path, config: &PipConfig, timeout: Option<Duration>) -> Result<()> {
    validate(config)?;
    let python_command = format!("python{}", config.python);
    run_command_with_timeout(&python_command, ["-m", "venv", ".venv"], workdir, timeout)?;

    match config.scope {
        PythonScope::Base => Ok(()),
        PythonScope::Install => install_project(workdir, config, timeout),
        PythonScope::Requirements => install_requirements(workdir, timeout),
    }
}

fn install_project(workdir: &Path, config: &PipConfig, timeout: Option<Duration>) -> Result<()> {
    ensure_readme(workdir)?;
    let python = venv_python(workdir).to_string_lossy().into_owned();
    let editable_target = editable_install_target(&config.extras);
    run_command_with_timeout(
        &python,
        ["-m", "pip", "install", "-e", editable_target.as_str()],
        workdir,
        timeout,
    )
}

fn install_requirements(workdir: &Path, timeout: Option<Duration>) -> Result<()> {
    let python = venv_python(workdir).to_string_lossy().into_owned();
    let requirements = get_or_create_requirements_file(workdir)?;
    let requirements = requirements.to_string_lossy().to_string();
    run_command_with_timeout(
        &python,
        ["-m", "pip", "install", "-r", requirements.as_str()],
        workdir,
        timeout,
    )
}

fn venv_python(workdir: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        return workdir.join(".venv").join("Scripts").join("python.exe");
    }

    #[cfg(not(windows))]
    workdir.join(".venv").join("bin").join("python")
}

fn validate(config: &PipConfig) -> Result<()> {
    match config.scope {
        PythonScope::Install if !config.groups.is_empty() => bail!(
            "pip groups cannot be applied to editable install setup. Remove groups or use a different plugin."
        ),
        PythonScope::Requirements if !config.groups.is_empty() || !config.extras.is_empty() => {
            bail!("pip groups/extras cannot be applied to requirements-file setup.")
        }
        PythonScope::Base if !config.groups.is_empty() || !config.extras.is_empty() => bail!(
            "pip groups/extras require scope = \"install\" with extras only, or remove groups/extras."
        ),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_groups_for_editable_install() {
        let config = PipConfig {
            scope: PythonScope::Install,
            groups: vec!["dev".to_string()],
            ..PipConfig::default()
        };
        assert!(validate(&config).is_err());
    }

    #[test]
    fn builds_platform_appropriate_venv_python_path() {
        let path = venv_python(Path::new("project"));
        #[cfg(windows)]
        assert!(path.ends_with(".venv\\Scripts\\python.exe"));
        #[cfg(not(windows))]
        assert!(path.ends_with(".venv/bin/python"));
    }
}
