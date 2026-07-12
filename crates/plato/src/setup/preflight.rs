use anyhow::{Context, Result};
use plato_plugin_api::PluginMetadata;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::GlobalConfig;
use crate::plugins::command::{read_metadata, validate_setup_metadata};
use crate::plugins::discovery::{PluginLocationKind, resolve_plugin_command};
use crate::plugins::id::PluginId;
use crate::setup::plan::SetupPlan;
use crate::setup::step::SetupStep;

const PLUGIN_METADATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSetupPlan {
    pub(crate) steps: Vec<ResolvedSetupStep>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSetupStep {
    pub(crate) step: SetupStep,
    pub(crate) command: PathBuf,
    pub(crate) kind: PluginLocationKind,
    pub(crate) metadata: PluginMetadata,
}

pub(crate) fn preflight_setup_plan(
    global_config: &GlobalConfig,
    plan: &SetupPlan,
) -> Result<ResolvedSetupPlan> {
    let mut plugins = BTreeMap::new();
    let mut steps = Vec::with_capacity(plan.steps.len());

    for step in &plan.steps {
        let resolved = resolve_plugin(global_config, &step.plugin, &mut plugins)?;
        steps.push(ResolvedSetupStep {
            step: step.clone(),
            command: resolved.command.clone(),
            kind: resolved.kind.clone(),
            metadata: resolved.metadata.clone(),
        });
    }
    Ok(ResolvedSetupPlan { steps })
}

#[derive(Debug, Clone)]
struct ResolvedPlugin {
    command: PathBuf,
    kind: PluginLocationKind,
    metadata: PluginMetadata,
}

fn resolve_plugin(
    global_config: &GlobalConfig,
    plugin: &PluginId,
    plugins: &mut BTreeMap<PluginId, ResolvedPlugin>,
) -> Result<ResolvedPlugin> {
    if let Some(resolved) = plugins.get(plugin) {
        return Ok(resolved.clone());
    }

    let command = resolve_plugin_command(global_config, plugin)?;
    let metadata = read_metadata(&command.command, PLUGIN_METADATA_TIMEOUT)
        .with_context(|| format!("Failed to read metadata for plugin {plugin}"))?;
    validate_setup_metadata(plugin, &metadata)?;
    let resolved = ResolvedPlugin {
        command: command.command,
        kind: command.kind,
        metadata,
    };
    plugins.insert(plugin.clone(), resolved.clone());
    Ok(resolved)
}
