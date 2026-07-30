use anyhow::Context;
use plto_plugin_api::{PluginCapability, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use plto_plugin_support::{SetupPlugin, command::run_command_with_timeout, run as run_plugin};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PnpmConfig {
    #[serde(default)]
    install: bool,
    #[serde(default)]
    frozen_lockfile: bool,
}

struct PnpmPlugin;

impl SetupPlugin for PnpmPlugin {
    fn metadata(&self) -> PluginMetadata {
        metadata("pnpm", "Sets up pnpm projects")
    }

    fn setup(&self, request: PluginSetupRequest) -> anyhow::Result<PluginSetupResponse> {
        let config: PnpmConfig =
            serde_json::from_value(request.config).context("Invalid pnpm plugin config")?;
        if config.install {
            let mut args = vec!["install".to_string()];
            if config.frozen_lockfile {
                args.push("--frozen-lockfile".to_string());
            }
            run_command_with_timeout("pnpm", args, &request.workdir, request.options.timeout())?;
        }
        Ok(PluginSetupResponse::success("pnpm setup complete"))
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
    run_plugin(&PnpmPlugin)
}
