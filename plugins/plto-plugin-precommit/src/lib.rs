use anyhow::Context;
use plto_plugin_api::{PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use plto_plugin_support::{SetupPlugin, command::run_command_with_timeout, run as run_plugin};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PrecommitConfig {
    #[serde(default)]
    install_hooks: bool,
}

struct PrecommitPlugin;

impl SetupPlugin for PrecommitPlugin {
    fn metadata(&self) -> PluginMetadata {
        metadata("precommit", "Installs pre-commit hooks")
    }

    fn setup(&self, request: PluginSetupRequest) -> anyhow::Result<PluginSetupResponse> {
        let config: PrecommitConfig =
            serde_json::from_value(request.config).context("Invalid precommit plugin config")?;
        if config.install_hooks {
            run_command_with_timeout(
                "pre-commit",
                ["install"],
                &request.workdir,
                request.options.timeout(),
            )?;
        }
        Ok(PluginSetupResponse::success("pre-commit setup complete"))
    }
}

fn metadata(name: &str, description: &str) -> PluginMetadata {
    PluginMetadata {
        name: name.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        supported_api_versions: vec![1],
        capabilities: vec![PluginCapability::Setup],
        description: Some(description.to_string()),
    }
}

#[must_use]
pub fn run() -> std::process::ExitCode {
    run_plugin(&PrecommitPlugin)
}
