use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

use crate::config::{Config, TemplateEntry, parse_config_file};
use crate::fs::path::expand_tilde;
use crate::source::registry::{TemplateRecord, TemplateRegistry};

pub(crate) fn config_path_for(registry: &TemplateRegistry, template_name: &str) -> Result<PathBuf> {
    let Some(record) = registry.get(template_name) else {
        bail!("No configured template found for {template_name:?}");
    };

    if let Some(config_override) = record.override_path() {
        return Ok(config_override.to_path_buf());
    }

    match &record.entry {
        TemplateEntry::Path { path } => {
            let source_path = expand_tilde(path)?;
            let source_config = source_path.join("plto.toml");
            if source_config.exists() {
                return Ok(source_config);
            }
            bail!(
                "Template {template_name:?} has no source plto.toml and no [template_configs] override. Add a [template_configs] entry for this template."
            )
        }
        TemplateEntry::Git { .. } => {
            bail!(
                "Remote template {template_name:?} has no [template_configs] override. Add a [template_configs] entry for this template."
            )
        }
    }
}

pub(crate) fn select_named_config(
    name: &str,
    source_path: &Path,
    record: &TemplateRecord,
) -> Result<Config> {
    let source_config = source_path.join("plto.toml");
    if let Some(config_override) = record.override_path() {
        if source_config.exists() {
            eprintln!(
                "WARNING: Template {name:?} has both [template_configs] override and source plto.toml. Using override config."
            );
        }
        return parse_config_file(config_override);
    }

    if source_config.exists() {
        return parse_config_file(&source_config);
    }

    eprintln!(
        "WARNING: Template {name:?} has no plto.toml and no [template_configs] override. Using default template configuration."
    );
    Ok(Config::default())
}

pub(crate) fn select_ad_hoc_config(source_path: &Path, label: &str) -> Result<Config> {
    let source_config = source_path.join("plto.toml");
    if source_config.exists() {
        return parse_config_file(&source_config);
    }

    eprintln!("WARNING: {label} has no plto.toml. Using default template configuration.");
    Ok(Config::default())
}
