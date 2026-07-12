use anyhow::{Context, Result, anyhow, bail};
use plato_plugin_api::{
    PLUGIN_API_VERSION, PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse,
};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::plugins::id::PluginId;
use crate::process::{spawn, wait_with_timeout};

pub(crate) fn read_metadata(command: &Path, timeout: Duration) -> Result<PluginMetadata> {
    let description = format!("plugin metadata command {}", command.display());
    let mut process = Command::new(command);
    process
        .arg("metadata")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = spawn(&mut process, &description)?;

    let stdout = read_stdout_in_background(&mut child)?;
    let status = wait_with_timeout(&mut child, timeout, &description);
    let stdout = collect_output(stdout);
    let status = status?;
    let stdout = stdout?;

    if !status.success() {
        bail!("Plugin metadata command failed: {}", command.display());
    }
    let metadata = serde_json::from_slice::<PluginMetadata>(&stdout).with_context(|| {
        format!(
            "Plugin {} returned invalid metadata JSON",
            command.display()
        )
    })?;
    Ok(metadata)
}

pub(crate) fn validate_setup_metadata(
    expected: &PluginId,
    metadata: &PluginMetadata,
) -> Result<()> {
    if metadata.name != expected.as_str() {
        bail!(
            "Plugin metadata name {:?} does not match requested plugin {:?}",
            metadata.name,
            expected.as_str()
        );
    }
    if !metadata
        .supported_api_versions
        .contains(&PLUGIN_API_VERSION)
    {
        bail!(
            "Plugin {} does not support Plato plugin API v{}",
            metadata.name,
            PLUGIN_API_VERSION
        );
    }
    if !metadata.capabilities.contains(&PluginCapability::Setup) {
        bail!(
            "Plugin {} does not advertise setup capability",
            metadata.name
        );
    }
    Ok(())
}

pub(crate) fn run_setup(
    command: &Path,
    request: &PluginSetupRequest,
    timeout: Duration,
) -> Result<PluginSetupResponse> {
    let description = format!("plugin setup command {}", command.display());
    let mut process = Command::new(command);
    process
        .arg("setup")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = spawn(&mut process, &description)?;

    let stdout = read_stdout_in_background(&mut child)?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Plugin stdin was unavailable"))?;
        serde_json::to_writer(&mut stdin, request)?;
        stdin.write_all(b"\n")?;
    }

    let status = wait_with_timeout(&mut child, timeout, &description);
    let stdout = collect_output(stdout);
    let status = status?;
    let stdout = stdout?;

    if !status.success() {
        bail!("Plugin setup command failed: {}", command.display());
    }

    let response = serde_json::from_slice::<PluginSetupResponse>(&stdout).with_context(|| {
        format!(
            "Plugin {} returned invalid setup response JSON",
            command.display()
        )
    })?;
    if !response.ok {
        if let Some(error) = &response.error {
            bail!("Plugin {} failed: {}", request.plugin, error.message);
        }
        bail!("Plugin {} failed without an error message", request.plugin);
    }
    Ok(response)
}

fn read_stdout_in_background(
    child: &mut Child,
) -> Result<thread::JoinHandle<std::io::Result<Vec<u8>>>> {
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Plugin stdout was unavailable"))?;
    Ok(thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output)?;
        Ok(output)
    }))
}

fn collect_output(handle: thread::JoinHandle<std::io::Result<Vec<u8>>>) -> Result<Vec<u8>> {
    handle
        .join()
        .map_err(|_| anyhow!("Plugin stdout reader panicked"))?
        .context("Failed to read plugin stdout")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> PluginMetadata {
        PluginMetadata {
            name: "uv".to_string(),
            version: "1.0.0".to_string(),
            supported_api_versions: vec![PLUGIN_API_VERSION],
            capabilities: vec![PluginCapability::Setup],
            description: None,
        }
    }

    #[test]
    fn rejects_mismatched_metadata_names() {
        let error =
            validate_setup_metadata(&PluginId::parse("pip").unwrap(), &metadata()).unwrap_err();
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn rejects_plugins_without_setup_capability() {
        let mut metadata = metadata();
        metadata.capabilities.clear();
        let error =
            validate_setup_metadata(&PluginId::parse("uv").unwrap(), &metadata).unwrap_err();
        assert!(error.to_string().contains("setup capability"));
    }
}
