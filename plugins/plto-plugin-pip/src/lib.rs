mod config;
mod pyproject;
mod setup;

use anyhow::Context;
use plto_plugin_api::{PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use plto_plugin_support::{SetupPlugin, run as run_plugin};

struct PipPlugin;

impl SetupPlugin for PipPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "pip".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_api_versions: vec![1],
            capabilities: vec![PluginCapability::Setup],
            description: Some("Sets up pip-based Python projects".to_string()),
        }
    }

    fn setup(&self, request: PluginSetupRequest) -> anyhow::Result<PluginSetupResponse> {
        let config: config::PipConfig =
            serde_json::from_value(request.config).context("Invalid pip plugin config")?;
        setup::setup(&request.workdir, &config, request.options.timeout())?;
        Ok(PluginSetupResponse::success("pip setup complete"))
    }
}

#[must_use]
pub fn run() -> std::process::ExitCode {
    run_plugin(&PipPlugin)
}
