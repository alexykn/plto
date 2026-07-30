use std::path::PathBuf;

use crate::config::{Config, GlobalConfig, TemplateEntry, parse_global_config_file};

#[test]
fn deserializes_global_template_entries() {
    let raw = r#"
[plato]
default_git_provider = "gitlab"

[plato.git_hosts]
gitlab = "gitlab.company.test"

[templates]
py = { path = "~/templates/py" }
api = { git = "gitlab:group/api", rev = "main", subpath = "templates/api" }

[template_configs]
api = "~/.config/plato/template_configs/api.toml"

[plugin_registry.uv]
command = "~/.local/share/plato/plugins/bin/plto-plugin-uv"
source = "cargo:plto-plugin-uv"
"#;
    let config: GlobalConfig = toml::from_str(raw).unwrap();

    assert!(matches!(config.templates["py"], TemplateEntry::Path { .. }));
    let TemplateEntry::Git { git, rev, subpath } = &config.templates["api"] else {
        panic!("expected git template");
    };
    assert_eq!(git, "gitlab:group/api");
    assert_eq!(rev.as_deref(), Some("main"));
    assert_eq!(
        subpath.as_deref(),
        Some(PathBuf::from("templates/api").as_path())
    );
    assert_eq!(
        config.template_configs["api"],
        PathBuf::from("~/.config/plato/template_configs/api.toml")
    );
    assert_eq!(
        config.plugin_registry["uv"].command,
        PathBuf::from("~/.local/share/plato/plugins/bin/plto-plugin-uv")
    );
}

#[test]
fn malformed_global_config_fails() {
    let path = std::env::temp_dir().join(format!(
        "plato-bad-global-{}-{}.toml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "[templates\n").unwrap();
    let error = parse_global_config_file(&path).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Invalid format in global config")
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn global_config_rejects_unknown_and_ambiguous_template_fields() {
    let unknown = r#"
    [plato]
    default_git_providre = "gitlab"
    "#;
    assert!(toml::from_str::<GlobalConfig>(unknown).is_err());

    let ambiguous = r#"
    [templates]
    app = { path = "templates/app", git = "github:org/app" }
    "#;
    assert!(toml::from_str::<GlobalConfig>(ambiguous).is_err());
}

#[test]
fn global_config_rejects_orphan_template_config_entries() {
    let raw = r#"
    [template_configs]
    missing = "template.toml"
    "#;
    let config: GlobalConfig = toml::from_str(raw).unwrap();
    assert!(config.validate().is_err());
}

#[test]
fn deserializes_template_path_replacements() {
    let raw = r#"
[template.context]
package_name = "py3-requests"
package_deps = "deps-runtime"

[path.replace]
source = { path = "src/py3-something", replace = "src/{{ package_name | regex_replace('^py3-', '') }}" }
deps = { path = "deps/funny", replace = "deps/{{ package_deps | regex_replace('^deps', 'stuff') }}" }
"#;
    let config: Config = toml::from_str(raw).unwrap();

    assert_eq!(
        config.path.replace["source"].path,
        PathBuf::from("src/py3-something")
    );
    assert_eq!(
        config.path.replace["source"].replace,
        "src/{{ package_name | regex_replace('^py3-', '') }}"
    );
    assert_eq!(
        config.path.replace["deps"].path,
        PathBuf::from("deps/funny")
    );
}

#[test]
fn deserializes_plugin_setup_steps() {
    let raw = r#"
[plugins.uv]
sync = true
locked = true

[plugins.pnpm]
install = true

[[setup.steps]]
plugin = "uv"
source_path = "backend"

[[setup.steps]]
plugin = "pnpm"
source_path = "frontend"
frozen_lockfile = true
"#;
    let config: Config = toml::from_str(raw).unwrap();

    assert!(config.plugins.contains_key("uv"));
    assert_eq!(config.setup.steps.len(), 2);
    assert_eq!(config.setup.steps[0].plugin, "uv");
    assert_eq!(config.setup.steps[0].source_path, PathBuf::from("backend"));
    assert_eq!(
        config.setup.steps[1].config_overrides["frozen_lockfile"].as_bool(),
        Some(true)
    );
}

#[test]
fn rejects_removed_language_config() {
    let raw = r#"
[python]
language_version = "3.12"
"#;
    let error = toml::from_str::<Config>(raw).unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}
