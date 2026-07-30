use serde::Deserialize;

fn default_toolchain() -> String {
    "stable".to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CargoConfig {
    #[serde(default = "default_toolchain")]
    pub(crate) toolchain: String,
    #[serde(default)]
    pub(crate) components: Vec<String>,
    #[serde(default)]
    pub(crate) targets: Vec<String>,
    #[serde(default)]
    pub(crate) scope: CargoScope,
    #[serde(default)]
    pub(crate) cargo_init: bool,
    #[serde(default)]
    pub(crate) project_type: RustProjectType,
}

impl Default for CargoConfig {
    fn default() -> Self {
        Self {
            toolchain: default_toolchain(),
            components: Vec::new(),
            targets: Vec::new(),
            scope: CargoScope::Base,
            cargo_init: false,
            project_type: RustProjectType::Binary,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CargoScope {
    #[default]
    Base,
    Fetch,
    Build,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RustProjectType {
    #[default]
    #[serde(alias = "bin")]
    Binary,
    #[serde(alias = "lib")]
    Library,
}
