# Plato

Plato is a local project scaffolding tool for one-command project setup. It renders a template directory with MiniJinja, writes the generated project, then runs ordered setup plugins such as `uv`, `pip`, `cargo`, `pnpm`, `git`, or `precommit`.

The core renderer stays deterministic: template context, path rewrites, path excludes, and file rendering are handled by Plato. Project initialization happens after rendering through external plugin binaries.

## Install

```bash
cargo install --path crates/plato
```

First-party plugins are separate binaries. Install them locally from this workspace:

```bash
plato plugin install uv --path plugins/plato-plugin-uv
plato plugin install git --path plugins/plato-plugin-git
```

## CLI

```bash
plato init <template_name> <project_name>
plato init --git <git_spec> <project_name>
plato init --path <template_dir> <project_name>
plato val <template_name> [project_name]
plato val --path <template_dir> [project_name]
plato config <template_name>
plato list [-v|--verbose]
plato plugin list
plato plugin install <name>
```

Common options for `init` and `val`:

```text
--rev <rev>              Git branch, tag, or commit
--subpath <path>         Subdirectory inside a Git template
-g, --group <group>      Apply plato.<group>.toml
-s, --set <key=value>    Typed template context override
--set-string <key=value> String template context override
```

`--rev` and `--subpath` apply only to remote templates. `--path` always requires a directory and cannot be combined with Git-specific options. `plato init --force` transactionally replaces an existing target directory and restores the original directory if rendering or setup fails.

## Quick start

Register templates in `~/.config/plato/config.toml`:

```toml
[templates]
python312 = { path = "~/.config/plato/py312" }
```

Then generate a project:

```bash
plato init python312 my-project
```

A template-local `plato.toml` controls rendering and setup. For example, a Python template can run `uv` and initialize Git after rendering:

```toml
[plugins.uv]
python = "3.12"
scope = "install"
setup = "editable"

[plugins.git]
init = true

[[setup.steps]]
plugin = "uv"

[[setup.steps]]
plugin = "git"
```

More complete examples live in [`docs/examples/`](docs/examples/).

## Documentation

- [Configuration reference](docs/configuration.md): `plato.toml`, template context, path rewrites/excludes, setup steps, and groups.
- [Plugin system](docs/plugins.md): external plugin protocol, discovery, installation, and Rust plugin authoring.
- [Config examples](docs/examples/): practical `plato.toml` snippets for first-party plugins.
- [Release guide](docs/releasing.md): staged package publication for the core and plugin crates.

## Core concepts

- **Templates** are normal directories. Files ending in `.j2` or `.mj` are rendered and written without that extension. Output-path collisions are rejected.
- **Symlinks** inside templates are rejected for safety.
- **Permissions** on source files, including executable bits on Unix, are preserved in generated files.
- **Template context** provides project name variants such as `project_name`, `project_kebab`, `project_snake`, and `project_pascal`.
- **Path rewrites/excludes** are core rendering behavior and happen before template contents are rendered.
- **Plugins** are external executables named `plato-plugin-<name>` that run after the rendered project is written.
- **Setup steps** are ordered. Each step can run in the project root or a subdirectory via `source_path`.

## Global configuration

Global configuration lives at:

```text
~/.config/plato/config.toml
```

See [config.example.toml](config.example.toml) for a complete global configuration example.

Minimal example:

```toml
[plato]
default_git_provider = "github"

[templates]
py = { path = "~/.config/plato/templates/py" }
api = { git = "gitlab:platform/api-template", rev = "main" }

[template_configs]
api = "~/.config/plato/template_configs/api.toml"
```

Configured Git templates can be used directly:

```bash
plato init api my-api
```

Ad-hoc Git templates use `--git`:

```bash
plato init --git gitlab:group/repo my-api
```

Supported Git specs include provider shorthand, SSH remotes, SCP-like SSH syntax, and HTTPS URLs. Plato rejects embedded credentials in Git URLs; use SSH keys or system Git credential helpers instead.

## Validation

`plato val` validates Plato rendering mechanics without creating a project or running setup plugins. It catches invalid config, template syntax errors, invalid path rewrites, duplicate rendered paths, undefined template variables, invalid setup-step structure, and setup `source_path` directories that are not rendered.

It does not prove that setup tools such as `uv`, `pip`, `cargo`, `pnpm`, or `git` will succeed. Use `plato init` in a temporary directory for full setup smoke tests.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
