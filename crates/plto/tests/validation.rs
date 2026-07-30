#[cfg(unix)]
#[path = "support/temp.rs"]
mod temp;

#[cfg(unix)]
use temp::TestEnv;

#[cfg(unix)]
#[test]
fn validation_rejects_parent_setup_source_path() {
    let env = TestEnv::new("validation-parent-source");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "uv"
        source_path = "../outside"
        "#,
    );
    env.write("template/README.md", "# Project\n");

    let output = env
        .command()
        .args(["val", "--path", "template", "project"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source_path"));
    assert!(!env.root().join("project").exists());
}

#[cfg(unix)]
#[test]
fn validation_rejects_missing_setup_source_directory() {
    let env = TestEnv::new("validation-missing-source");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "uv"
        source_path = "backend"
        "#,
    );
    env.write("template/README.md", "# Project\n");

    let output = env
        .command()
        .args(["val", "--path", "template", "project"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not rendered"));
    assert!(!env.root().join("project").exists());
}

#[cfg(unix)]
#[test]
fn validation_accepts_existing_setup_source_directory() {
    let env = TestEnv::new("validation-source-ok");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "uv"
        source_path = "backend"
        "#,
    );
    env.write(
        "template/backend/pyproject.toml",
        "[project]\nname = 'demo'\n",
    );

    let output = env
        .command()
        .args(["val", "--path", "template", "project"])
        .output()
        .expect("plto should run");

    assert!(output.status.success());
    assert!(!env.root().join("project").exists());
}

#[cfg(unix)]
#[test]
fn validation_rejects_template_symlink() {
    use std::os::unix::fs::symlink;

    let env = TestEnv::new("validation-symlink");
    env.write("template/plto.toml", "");
    env.write("outside-secret.txt", "secret\n");
    symlink(
        env.root().join("outside-secret.txt"),
        env.root().join("template/secret-link"),
    )
    .expect("symlink should be created");

    let output = env
        .command()
        .args(["val", "--path", "template", "project"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported symlink"));
    assert!(!env.root().join("project").exists());
}
