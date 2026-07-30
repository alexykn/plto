use anyhow::{Result, bail};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::config::PathExcludeConfig;
use crate::context::TemplateContext;
use crate::fs::path::reject_parent_components;
use crate::workspace::content::WorkspaceMap;

pub(super) fn apply_path_excludes(
    files: &mut WorkspaceMap,
    template_context: &TemplateContext,
    excludes: &BTreeMap<String, PathExcludeConfig>,
) -> Result<()> {
    for (name, exclude) in excludes {
        validate_exclude_path(name, &exclude.path)?;
        if should_keep(template_context, exclude)? {
            continue;
        }
        files.retain(|path, _| path != &exclude.path && !path.starts_with(&exclude.path));
    }
    Ok(())
}

fn should_keep(context: &TemplateContext, exclude: &PathExcludeConfig) -> Result<bool> {
    let Some(variable) = &exclude.unless else {
        return Ok(false);
    };
    if variable.is_empty() || variable.contains('.') {
        bail!(
            "Invalid path exclude condition {variable:?}: only top-level context variables are supported"
        );
    }
    Ok(context.get(variable).is_some_and(is_truthy))
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Bool(true) => true,
        Value::Number(number) => {
            number.as_i64().is_some_and(|value| value != 0)
                || number.as_u64().is_some_and(|value| value != 0)
                || number.as_f64().is_some_and(|value| value != 0.0)
        }
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn validate_exclude_path(name: &str, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("Invalid [path.exclude.{name}]: path must not be empty");
    }
    reject_parent_components(path, &format!("Invalid [path.exclude.{name}]: path"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::TemplateContext;
    use crate::workspace::content::WorkspaceEntry;
    use std::path::PathBuf;

    fn content() -> WorkspaceMap {
        WorkspaceMap::from([
            (
                PathBuf::from("Dockerfile"),
                WorkspaceEntry::rendered(Vec::<u8>::new()),
            ),
            (
                PathBuf::from("src"),
                WorkspaceEntry::rendered(Vec::<u8>::new()),
            ),
            (
                PathBuf::from("src/main.py"),
                WorkspaceEntry::rendered(Vec::<u8>::new()),
            ),
            (
                PathBuf::from("README.md"),
                WorkspaceEntry::rendered(Vec::<u8>::new()),
            ),
        ])
    }

    fn exclude(path: &str, unless: Option<&str>) -> PathExcludeConfig {
        PathExcludeConfig {
            path: PathBuf::from(path),
            unless: unless.map(str::to_string),
        }
    }

    #[test]
    fn excludes_exact_file() {
        let mut content = content();
        let rules = BTreeMap::from([("docker".to_string(), exclude("Dockerfile", None))]);

        apply_path_excludes(&mut content, &TemplateContext::new(), &rules).unwrap();

        assert!(!content.contains_key(Path::new("Dockerfile")));
        assert!(content.contains_key(Path::new("README.md")));
    }

    #[test]
    fn excludes_directory_descendants() {
        let mut content = content();
        let rules = BTreeMap::from([("src".to_string(), exclude("src", None))]);

        apply_path_excludes(&mut content, &TemplateContext::new(), &rules).unwrap();

        assert!(!content.contains_key(Path::new("src")));
        assert!(!content.contains_key(Path::new("src/main.py")));
    }

    #[test]
    fn keeps_when_unless_variable_is_truthy() {
        let mut files = content();
        let mut template_context = TemplateContext::new();
        template_context.merge(BTreeMap::from([("docker".to_string(), Value::Bool(true))]));
        let rules = BTreeMap::from([("docker".to_string(), exclude("Dockerfile", Some("docker")))]);

        apply_path_excludes(&mut files, &template_context, &rules).unwrap();

        assert!(files.contains_key(Path::new("Dockerfile")));
    }

    #[test]
    fn excludes_when_unless_variable_is_false_or_missing() {
        let mut content = content();
        let rules = BTreeMap::from([("docker".to_string(), exclude("Dockerfile", Some("docker")))]);

        apply_path_excludes(&mut content, &TemplateContext::new(), &rules).unwrap();

        assert!(!content.contains_key(Path::new("Dockerfile")));
    }

    #[test]
    fn missing_exclude_path_is_noop() {
        let mut content = content();
        let rules = BTreeMap::from([("missing".to_string(), exclude("missing", None))]);

        apply_path_excludes(&mut content, &TemplateContext::new(), &rules).unwrap();

        assert!(content.contains_key(Path::new("Dockerfile")));
    }

    #[test]
    fn rejects_unsafe_paths() {
        let mut content = content();
        let rules = BTreeMap::from([("escape".to_string(), exclude("../secret", None))]);

        let error = apply_path_excludes(&mut content, &TemplateContext::new(), &rules).unwrap_err();

        assert!(error.to_string().contains("'..'"));
    }
}
