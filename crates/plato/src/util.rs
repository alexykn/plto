use anyhow::{Context, Result, bail};
use regex::Regex;
use std::env::var;
use std::ffi::OsStr;
use std::fs::{File, create_dir_all};
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use crate::process::run_status;

const DEFAULT_EXTERNAL_COMMAND_TIMEOUT: Duration = Duration::from_mins(5);

static ALLOWED_CMD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(git|cargo|uv|python\d*(?:\.\d+)*)$").expect("Invalid regex pattern")
});

fn get_default_editor() -> Result<(String, Vec<String>)> {
    let visual = var("VISUAL").unwrap_or_default();
    let editor = var("EDITOR").unwrap_or_default();
    let raw_cmd = if !visual.trim().is_empty() {
        &visual
    } else if !editor.trim().is_empty() {
        &editor
    } else {
        "nano"
    };
    let mut parts = shell_words::split(raw_cmd)
        .context("Could not parse EDITOR/VISUAL command.")?
        .into_iter();

    let command = parts.next().unwrap_or_else(|| "nano".to_string());
    let args = parts.collect();
    Ok((command, args))
}

/// Opens a `plato.toml` file in the user's editor.
///
/// # Errors
/// Returns an error if the editor cannot be started or exits unsuccessfully.
pub(crate) fn open_config_file(config_file_path: &Path) -> Result<()> {
    if let Some(parent) = config_file_path.parent() {
        create_dir_all(parent)
            .with_context(|| format!("Could not create config directory {}", parent.display()))?;
    }
    if !config_file_path.exists() {
        File::create(config_file_path).with_context(|| {
            format!(
                "Could not create config file {}",
                config_file_path.display()
            )
        })?;
    }
    let (command, mut args) = get_default_editor()?;

    args.push(config_file_path.to_string_lossy().to_string());
    let mut child = Command::new(command).args(args).spawn()?;
    let status = child.wait()?;
    if !status.success() {
        bail!("Editor exited with non-zero exit code.")
    }

    Ok(())
}

pub(crate) fn execute_command<I, S>(cmd: &str, args: I, target: &Path) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cmd_name = Path::new(cmd)
        .file_name()
        .and_then(|result| result.to_str())
        .unwrap_or(cmd);
    if !ALLOWED_CMD_RE.is_match(cmd_name) {
        bail!("Selected command '{cmd}' is not allowed");
    }
    let mut command = Command::new(cmd);
    command.args(args).current_dir(target);
    run_status(
        &mut command,
        DEFAULT_EXTERNAL_COMMAND_TIMEOUT,
        &format!("command {cmd}"),
    )?;
    Ok(())
}
