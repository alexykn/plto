#[cfg(unix)]
#[path = "support/fail_plugin.rs"]
mod fail_plugin;
#[cfg(unix)]
#[path = "support/temp.rs"]
mod temp;

#[cfg(unix)]
use fail_plugin::write_failing_plugin;
#[cfg(unix)]
use temp::TestEnv;

#[cfg(unix)]
#[test]
fn removes_new_target_after_plugin_failure() {
    let env = TestEnv::new("init-cleanup-new");
    write_failing_plugin(&env, "fail");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "fail"
        "#,
    );
    env.write("template/README.md", "# Project\n");

    let output = env
        .command()
        .args(["init", "--path", "template", "generated"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(!env.root().join("generated").exists());
}

#[cfg(unix)]
#[test]
fn preserves_pre_existing_forced_target_after_plugin_failure() {
    let env = TestEnv::new("init-cleanup-force");
    write_failing_plugin(&env, "fail");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "fail"
        "#,
    );
    env.write("template/README.md", "# Project\n");
    env.write("generated/sentinel.txt", "keep me\n");

    let output = env
        .command()
        .args(["init", "--path", "template", "--force", "generated"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(env.root().join("generated").exists());
    assert_eq!(
        std::fs::read_to_string(env.root().join("generated/sentinel.txt")).unwrap(),
        "keep me\n"
    );
}

#[cfg(unix)]
#[test]
fn force_replaces_existing_target_without_leaving_stale_files() {
    let env = TestEnv::new("init-force-replace");
    env.write("template/plto.toml", "");
    env.write("template/new.txt", "new\n");
    env.write("generated/stale.txt", "stale\n");

    let output = env
        .command()
        .args(["init", "--path", "template", "--force", "generated"])
        .output()
        .expect("plto should run");

    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(env.root().join("generated/new.txt")).unwrap(),
        "new\n"
    );
    assert!(!env.root().join("generated/stale.txt").exists());
}

#[cfg(unix)]
#[test]
fn force_rejects_a_symlink_target() {
    use std::os::unix::fs::symlink;

    let env = TestEnv::new("init-force-symlink");
    env.write("template/plto.toml", "");
    env.write("template/new.txt", "new\n");
    std::fs::create_dir_all(env.root().join("outside")).unwrap();
    symlink(env.root().join("outside"), env.root().join("generated")).unwrap();

    let output = env
        .command()
        .args(["init", "--path", "template", "--force", "generated"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not be a symlink"));
    assert!(!env.root().join("outside/new.txt").exists());
}

#[cfg(unix)]
#[test]
fn preserves_executable_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let env = TestEnv::new("init-executable-file");
    env.write("template/plto.toml", "");
    env.write("template/tool", "#!/bin/sh\necho plto\n");
    let source = env.root().join("template/tool");
    let mut permissions = std::fs::metadata(&source).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(source, permissions).unwrap();

    let output = env
        .command()
        .args(["init", "--path", "template", "generated"])
        .output()
        .expect("plto should run");

    assert!(output.status.success());
    let permissions = std::fs::metadata(env.root().join("generated/tool"))
        .unwrap()
        .permissions();
    assert_eq!(permissions.mode() & 0o111, 0o111);
}
