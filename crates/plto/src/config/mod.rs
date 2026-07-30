pub(crate) mod global;
pub(crate) mod group;
pub(crate) mod template;
#[cfg(test)]
mod tests;

pub(crate) use global::{
    GitProvider, GlobalConfig, TemplateEntry, get_global_config_path, get_global_plto_dir,
    parse_global_config_file,
};
pub(crate) use template::{
    Config, GroupConfig, PathExcludeConfig, PathReplacementConfig, SetupStepConfig,
    parse_config_file,
};
