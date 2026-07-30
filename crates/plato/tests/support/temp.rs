use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TestEnv {
    root: PathBuf,
    home: PathBuf,
    bin: PathBuf,
}

impl TestEnv {
    pub fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "plato-{label}-{}-{unique}-{counter}",
            std::process::id()
        ));
        let home = root.join("home");
        let bin = root.join("bin");
        std::fs::create_dir_all(&home).expect("test home should be created");
        std::fs::create_dir_all(&bin).expect("test bin should be created");
        Self { root, home, bin }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, relative: impl AsRef<Path>, content: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("test parent directory should be created");
        }
        std::fs::write(path, content).expect("test file should be written");
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_plto"));
        let existing_path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(self.bin.clone()).chain(std::env::split_paths(&existing_path)),
        )
        .expect("test PATH should be valid");
        command
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("PATH", path);
        command
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
