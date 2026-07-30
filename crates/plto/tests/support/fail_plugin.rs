use std::os::unix::fs::PermissionsExt;

use crate::temp::TestEnv;

pub fn write_failing_plugin(env: &TestEnv, name: &str) {
    let path = env.root().join("bin").join(format!("plto-plugin-{name}"));
    let script = format!(
        r#"#!/bin/sh
case "$1" in
  metadata)
    cat <<'JSON'
{{
  "name": "{name}",
  "version": "0.0.0",
  "supported_api_versions": [1],
  "capabilities": ["setup"],
  "description": null
}}
JSON
    ;;
  setup)
    cat <<'JSON'
{{
  "ok": false,
  "messages": [],
  "warnings": [],
  "created_files": [],
  "modified_files": [],
  "error": {{
    "code": "test_failure",
    "message": "intentional failure",
    "details": null
  }}
}}
JSON
    ;;
  *)
    echo "unknown command" >&2
    exit 1
    ;;
esac
"#
    );
    std::fs::write(&path, script).expect("test plugin should be written");
    let mut permissions = std::fs::metadata(&path)
        .expect("test plugin metadata should be available")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("test plugin should be executable");
}
