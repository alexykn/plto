mod config;
mod pyproject;
mod setup;

use anyhow::Context;
use plto_plugin_api::{PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use plto_plugin_support::{SetupPlugin, run as run_plugin};

struct UvPlugin;

impl SetupPlugin for UvPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "uv".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_api_versions: vec![1],
            capabilities: vec![PluginCapability::Setup],
            description: Some("Sets up uv-based Python projects".to_string()),
        }
    }

    fn setup(&self, request: PluginSetupRequest) -> anyhow::Result<PluginSetupResponse> {
        let config: config::UvConfig =
            serde_json::from_value(request.config).context("Invalid uv plugin config")?;
        setup::setup(&request.workdir, &config, request.options.timeout())?;
        Ok(PluginSetupResponse::success("uv setup complete"))
    }
}

#[must_use]
pub fn run() -> std::process::ExitCode {
    run_plugin(&UvPlugin)
}
