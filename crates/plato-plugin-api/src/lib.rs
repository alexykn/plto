use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

pub const PLUGIN_API_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub supported_api_versions: Vec<u16>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    Setup,
    Validate,
    Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSetupRequest {
    pub api_version: u16,
    pub plugin: String,
    pub project_name: String,
    pub target_path: PathBuf,
    pub source_path: PathBuf,
    pub workdir: PathBuf,
    #[serde(default)]
    pub template_path: Option<PathBuf>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub context: serde_json::Value,
    pub options: PluginOptions,
    pub environment: PluginEnvironment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOptions {
    pub dry_run: bool,
    pub verbose: bool,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

impl PluginOptions {
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout_secs.map(Duration::from_secs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEnvironment {
    pub plato_version: String,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSetupResponse {
    pub ok: bool,
    #[serde(default)]
    pub messages: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub created_files: Vec<PathBuf>,
    #[serde(default)]
    pub modified_files: Vec<PathBuf>,
    #[serde(default)]
    pub error: Option<PluginError>,
}

impl PluginSetupResponse {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            messages: vec![message.into()],
            warnings: Vec::new(),
            created_files: Vec::new(),
            modified_files: Vec::new(),
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: serde_json::Value,
}
