use serde::Deserialize;

fn default_python() -> String {
    "3".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PipConfig {
    #[serde(default = "default_python")]
    pub(crate) python: String,
    #[serde(default)]
    pub(crate) scope: PythonScope,
    #[serde(default)]
    pub(crate) groups: Vec<String>,
    #[serde(default)]
    pub(crate) extras: Vec<String>,
}

impl Default for PipConfig {
    fn default() -> Self {
        Self {
            python: default_python(),
            scope: PythonScope::Base,
            groups: Vec::new(),
            extras: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PythonScope {
    #[default]
    Base,
    Install,
    Requirements,
}
