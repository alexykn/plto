mod config;
mod setup;

use anyhow::Context;
use plto_plugin_api::{PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use plto_plugin_support::{SetupPlugin, run as run_plugin};

struct GitPlugin;

impl SetupPlugin for GitPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "git".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            supported_api_versions: vec![1],
            capabilities: vec![PluginCapability::Setup],
            description: Some("Initializes git repositories for Plto projects".to_string()),
        }
    }

    fn setup(&self, request: PluginSetupRequest) -> anyhow::Result<PluginSetupResponse> {
        let config: config::GitPluginConfig =
            serde_json::from_value(request.config).context("Invalid git plugin config")?;
        setup::run(&request.workdir, &config, request.options.timeout())?;
        Ok(PluginSetupResponse::success("git setup complete"))
    }
}

#[must_use]
pub fn run() -> std::process::ExitCode {
    run_plugin(&GitPlugin)
}
