#[cfg(unix)]
#[path = "support/sleep_plugin.rs"]
mod sleep_plugin;
#[cfg(unix)]
#[path = "support/temp.rs"]
mod temp;

#[cfg(unix)]
use sleep_plugin::write_sleeping_plugin;
#[cfg(unix)]
use temp::TestEnv;

#[cfg(unix)]
#[test]
fn plugin_setup_timeout_fails_and_cleans_new_target() {
    let env = TestEnv::new("plugin-timeout-new");
    write_sleeping_plugin(&env, "sleep");
    env.write(
        "template/plto.toml",
        r#"
        [[setup.steps]]
        plugin = "sleep"
        timeout_secs = 1
        "#,
    );
    env.write("template/README.md", "# Project\n");

    let output = env
        .command()
        .args(["init", "--path", "template", "generated"])
        .output()
        .expect("plto should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("timed out"));
    assert!(!env.root().join("generated").exists());
}
