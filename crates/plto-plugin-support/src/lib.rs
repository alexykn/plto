pub mod command;

use anyhow::{Context, Result, bail};
use plto_plugin_api::{PluginError, PluginMetadata, PluginSetupRequest, PluginSetupResponse};
use std::io::{Read, Write};
use std::process::ExitCode;

pub trait SetupPlugin {
    fn metadata(&self) -> PluginMetadata;

    /// Runs plugin setup.
    ///
    /// # Errors
    /// Returns an error when the plugin cannot complete setup for the supplied request.
    fn setup(&self, request: PluginSetupRequest) -> Result<PluginSetupResponse>;
}

pub fn run<P: SetupPlugin>(plugin: &P) -> ExitCode {
    match run_inner(plugin) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_inner<P: SetupPlugin>(plugin: &P) -> Result<()> {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "metadata".to_string());
    match command.as_str() {
        "metadata" => write_json(&plugin.metadata()),
        "setup" => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            let request = serde_json::from_str::<PluginSetupRequest>(&input)
                .context("Invalid plugin setup request JSON")?;
            let response = match plugin.setup(request) {
                Ok(response) => response,
                Err(error) => PluginSetupResponse {
                    ok: false,
                    messages: Vec::new(),
                    warnings: Vec::new(),
                    created_files: Vec::new(),
                    modified_files: Vec::new(),
                    error: Some(PluginError {
                        code: "plugin_error".to_string(),
                        message: format!("{error:#}"),
                        details: serde_json::Value::Null,
                    }),
                },
            };
            write_json(&response)
        }
        other => bail!("Unknown plugin command {other:?}. Expected 'metadata' or 'setup'."),
    }
}

fn write_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    Ok(())
}
