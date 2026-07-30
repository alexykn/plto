use anyhow::{Context, Result, bail};
use directories::BaseDirs;
use std::path::{Component, Path, PathBuf};

pub(crate) fn expand_tilde(path: &Path) -> Result<PathBuf> {
    let raw = path.to_string_lossy();
    if raw == "~" {
        let base_dirs = BaseDirs::new().context("Could not find home directory")?;
        return Ok(base_dirs.home_dir().to_path_buf());
    }

    let Some(rest) = raw.strip_prefix("~/") else {
        return Ok(path.to_path_buf());
    };

    let base_dirs = BaseDirs::new().context("Could not find home directory")?;
    Ok(base_dirs.home_dir().join(rest))
}

pub(crate) fn reject_parent_components(path: &Path, label: &str) -> Result<()> {
    if path.is_absolute() {
        bail!("{label} must be relative, got {}", path.display());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "{label} must not contain '..' components: {}",
            path.display()
        );
    }
    Ok(())
}

pub(crate) fn resolve_safe_subpath(root: &Path, subpath: Option<&Path>) -> Result<PathBuf> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("Could not canonicalize root {}", root.display()))?;
    let selected = match subpath {
        Some(subpath) => {
            reject_parent_components(subpath, "Template subpath")?;
            root.join(subpath)
        }
        None => root.to_path_buf(),
    };
    if !selected.exists() {
        bail!("Template subpath {} does not exist", selected.display());
    }
    let canonical_selected = selected.canonicalize().with_context(|| {
        format!(
            "Could not canonicalize template subpath {}",
            selected.display()
        )
    })?;
    if !canonical_selected.starts_with(&canonical_root) {
        bail!(
            "Template subpath {} escapes root {}",
            canonical_selected.display(),
            canonical_root.display()
        );
    }
    Ok(canonical_selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn allows_dotdot_inside_component_name() {
        let path = Path::new("foo..bar/repo");
        reject_parent_components(path, "path").unwrap();
    }

    #[test]
    fn rejects_parent_component() {
        let error = reject_parent_components(Path::new("foo/../repo"), "path").unwrap_err();
        assert!(error.to_string().contains("'..'"));
    }

    #[test]
    fn resolves_safe_subpath() {
        let root = unique_test_dir("safe-path");
        let nested = root.join("foo..bar/repo");
        create_dir_all(&nested).unwrap();
        let resolved = resolve_safe_subpath(&root, Some(Path::new("foo..bar/repo"))).unwrap();
        assert_eq!(resolved, nested.canonicalize().unwrap());
        remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = unique_test_dir("safe-path-root");
        let outside = unique_test_dir("safe-path-outside");
        symlink(&outside, root.join("escape")).unwrap();

        let error = resolve_safe_subpath(&root, Some(Path::new("escape"))).unwrap_err();
        assert!(error.to_string().contains("escapes root"));

        remove_dir_all(root).unwrap();
        remove_dir_all(outside).unwrap();
    }

    fn unique_test_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "plto-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        create_dir_all(&path).unwrap();
        path
    }
}
