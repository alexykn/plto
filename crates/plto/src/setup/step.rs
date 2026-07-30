use std::path::PathBuf;
use std::time::Duration;

use crate::plugins::id::PluginId;

#[derive(Debug, Clone)]
pub(crate) struct SetupStep {
    pub(crate) plugin: PluginId,
    pub(crate) source_path: PathBuf,
    pub(crate) workdir: PathBuf,
    pub(crate) config: serde_json::Value,
    pub(crate) timeout: Duration,
}
