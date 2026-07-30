use serde::Deserialize;

fn default_python() -> String {
    "3".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UvConfig {
    #[serde(default = "default_python")]
    pub(crate) python: String,
    #[serde(default)]
    pub(crate) scope: PythonScope,
    #[serde(default)]
    pub(crate) setup: UvSetup,
    #[serde(default)]
    pub(crate) groups: Vec<String>,
    #[serde(default)]
    pub(crate) extras: Vec<String>,
    #[serde(default)]
    pub(crate) locked: bool,
    #[serde(default)]
    pub(crate) frozen: bool,
}

impl Default for UvConfig {
    fn default() -> Self {
        Self {
            python: default_python(),
            scope: PythonScope::Base,
            setup: UvSetup::Editable,
            groups: Vec::new(),
            extras: Vec::new(),
            locked: false,
            frozen: false,
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

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UvSetup {
    #[default]
    Editable,
    Sync,
}
