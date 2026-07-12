use anyhow::{Context, Result};
use plato_plugin_api::{PLUGIN_API_VERSION, PluginEnvironment, PluginOptions, PluginSetupRequest};
use std::path::{Path, PathBuf};

use crate::context::TemplateContext;
use crate::plugins::command::run_setup;
use crate::setup::preflight::ResolvedSetupPlan;

#[derive(Debug, Clone)]
pub(crate) struct SetupRunnerContext {
    pub(crate) project_name: String,
    pub(crate) target_path: PathBuf,
    pub(crate) template_path: PathBuf,
    pub(crate) template_context: TemplateContext,
    pub(crate) dry_run: bool,
    pub(crate) verbose: bool,
}

pub(crate) fn run_setup_plan(plan: &ResolvedSetupPlan, ctx: &SetupRunnerContext) -> Result<()> {
    for resolved in &plan.steps {
        let step = &resolved.step;
        if ctx.verbose {
            eprintln!(
                "Running plugin {} from {:?}: {}",
                step.plugin,
                resolved.kind,
                resolved.command.display()
            );
        }
        let request = PluginSetupRequest {
            api_version: PLUGIN_API_VERSION,
            plugin: step.plugin.to_string(),
            project_name: ctx.project_name.clone(),
            target_path: ctx.target_path.clone(),
            source_path: step.source_path.clone(),
            workdir: step.workdir.clone(),
            template_path: Some(ctx.template_path.clone()),
            config: step.config.clone(),
            context: ctx.template_context.clone().into_value(),
            options: PluginOptions {
                dry_run: ctx.dry_run,
                verbose: ctx.verbose,
                timeout_secs: Some(step.timeout.as_secs()),
            },
            environment: PluginEnvironment {
                plato_version: env!("CARGO_PKG_VERSION").to_string(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
            },
        };
        let response = run_setup(&resolved.command, &request, step.timeout)
            .with_context(|| format!("Failed to run plugin {}", resolved.metadata.name))?;
        for message in response.messages {
            println!("{message}");
        }
        for warning in response.warnings {
            eprintln!("WARNING: {warning}");
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _path_debug(path: &Path) -> String {
    path.display().to_string()
}
