use serde::Deserialize;
use std::path::PathBuf;

fn default_initial_commit_message() -> String {
    "Initial commit".to_string()
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GitPluginConfig {
    #[serde(default = "default_true")]
    pub init: bool,
    #[serde(default)]
    pub initial_branch: Option<String>,
    #[serde(default)]
    pub initial_commit: bool,
    #[serde(default = "default_initial_commit_message")]
    pub message: String,
    #[serde(default)]
    pub user: GitUserConfig,
    #[serde(default)]
    pub commit: GitCommitConfig,
    #[serde(default)]
    pub core: GitCoreConfig,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GitUserConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub signing_key: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GitCommitConfig {
    #[serde(default)]
    pub gpgsign: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GitCoreConfig {
    #[serde(default)]
    pub hooks_path: Option<PathBuf>,
    #[serde(default)]
    pub autocrlf: Option<GitAutoCrlfConfig>,
    #[serde(default)]
    pub eol: Option<GitEolConfig>,
    #[serde(default)]
    pub filemode: Option<bool>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum GitAutoCrlfConfig {
    Bool(bool),
    Mode(GitAutoCrlfMode),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitAutoCrlfMode {
    Input,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitEolConfig {
    Lf,
    Crlf,
    Native,
}
