use anyhow::{Context, Result};
use directories::BaseDirs;
use std::ffi::OsStr;
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::fs::path::resolve_safe_subpath;
use crate::source::git::cache::GitCacheLock;
use crate::source::git::spec::GitTemplateSpec;
use crate::util::execute_command;

pub(crate) struct GitTemplateFetcher {
    cache_root: PathBuf,
}

impl GitTemplateFetcher {
    pub(crate) fn from_user_cache_dir() -> Result<Self> {
        let base_dirs = BaseDirs::new().context("Could not find home directory")?;
        Ok(Self::new(base_dirs.home_dir().join(".cache/plto/git")))
    }

    pub(crate) fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    pub(crate) fn prepare_checkout(&self, spec: &GitTemplateSpec) -> Result<GitCheckout> {
        create_dir_all(&self.cache_root).with_context(|| {
            format!(
                "Could not create Git cache dir {}",
                self.cache_root.display()
            )
        })?;

        let cache_path = self.cache_path(&spec.url);
        let lock_path = self.lock_path(&spec.url);
        let _lock = GitCacheLock::acquire(&lock_path)?;

        if cache_path.exists() {
            run_git(
                [
                    "fetch",
                    "--prune",
                    "--tags",
                    "origin",
                    "+refs/heads/*:refs/heads/*",
                ],
                &cache_path,
                "fetch remote template updates",
            )?;
        } else {
            let cache_path_arg = cache_path.to_string_lossy().into_owned();
            run_git(
                [
                    "clone",
                    "--bare",
                    spec.url.as_str(),
                    cache_path_arg.as_str(),
                ],
                &self.cache_root,
                "clone remote template into cache",
            )?;
        }

        let checkout_root = self.create_checkout_dir()?;
        let cache_path_arg = cache_path.to_string_lossy().into_owned();
        let checkout_arg = checkout_root.to_string_lossy().into_owned();
        run_git(
            ["clone", cache_path_arg.as_str(), checkout_arg.as_str()],
            &self.cache_root,
            "create temporary remote template checkout",
        )?;

        if let Some(revision) = &spec.revision {
            run_git(
                ["checkout", revision.as_str()],
                &checkout_root,
                "checkout remote template revision",
            )?;
        }

        let source_path = resolve_safe_subpath(&checkout_root, spec.subpath.as_deref())?;
        Ok(GitCheckout {
            source_path,
            cleanup: TempCheckout::new(checkout_root),
        })
    }

    fn cache_path(&self, url: &str) -> PathBuf {
        self.cache_root
            .join(format!("{}.git", blake3::hash(url.as_bytes()).to_hex()))
    }

    fn lock_path(&self, url: &str) -> PathBuf {
        self.cache_root
            .join(format!("{}.lock", blake3::hash(url.as_bytes()).to_hex()))
    }

    fn create_checkout_dir(&self) -> Result<PathBuf> {
        let temp_root = self.cache_root.join("checkouts");
        create_dir_all(&temp_root).with_context(|| {
            format!(
                "Could not create Git checkout temp dir {}",
                temp_root.display()
            )
        })?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System clock is before UNIX epoch")?
            .as_nanos();
        let checkout_path = temp_root.join(format!("{}-{timestamp}", std::process::id()));
        create_dir_all(&checkout_path).with_context(|| {
            format!(
                "Could not create Git checkout dir {}",
                checkout_path.display()
            )
        })?;
        Ok(checkout_path)
    }
}

pub(crate) struct GitCheckout {
    pub(crate) source_path: PathBuf,
    cleanup: TempCheckout,
}

impl GitCheckout {
    pub(crate) fn into_cleanup(self) -> TempCheckout {
        self.cleanup
    }
}

pub(crate) struct TempCheckout {
    path: PathBuf,
}

impl TempCheckout {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempCheckout {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(crate) fn run_git<I, S>(args: I, current_dir: &Path, context: &str) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    execute_command("git", args, current_dir)
        .with_context(|| format!("Could not {context}. Verify Git authentication, SSH agent, or credential helper setup if this is a private repository."))
}
