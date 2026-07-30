use anyhow::Result;
use std::path::PathBuf;

use crate::config::get_global_plto_dir;

pub(crate) fn managed_plugin_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("PLTO_HOME") {
        return Ok(PathBuf::from(path).join("plugins"));
    }
    Ok(get_global_plto_dir()?.join("plugins"))
}

pub(crate) fn managed_plugin_bin_dir() -> Result<PathBuf> {
    Ok(managed_plugin_root()?.join("bin"))
}
