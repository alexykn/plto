# Configuration reference

Template configuration lives in `plato.toml` at the template root. The file is configuration only and is not copied into the generated project.

Plato rejects unknown core configuration fields. This makes misspelled options fail instead of silently changing behavior. Plugin-specific tables are validated by their plugin.

## Top-level sections

```toml
[template.context]
# Values available to MiniJinja templates.

[path.replace]
# Named path rewrite rules.

[path.exclude]
# Named path exclusion rules.

[plugins.<name>]
# Plugin-specific setup config.

[[setup.steps]]
# Ordered post-render setup steps.
```

## Template context

Plato always provides project name variants:

```text
project_name    # exactly as passed on the CLI
project_kebab   # my-project
project_snake   # my_project
project_pascal  # MyProject
```

Plugin config is also exposed under `config.plugins`:

```jinja2
{{ config.plugins.uv.python }}
```

Values from `[template.context]` and CLI overrides can add or override context values.

```toml
[template.context]
author = "Jane Doe"
include_docs = true
```

CLI overrides:

```bash
plato init api my-api -s include_docs=false --set-string version=1.0
```

## Path rewrites

`[path.replace]` renames files or directories in the in-memory workspace before template contents are rendered.

```toml
[path.replace]
package = { path = "src/package_template", replace = "src/{{ project_snake }}" }
```

`path` must match a relative path in the template source tree. `replace` is rendered with the same MiniJinja environment as file contents.

Every rendered output path must be unique. For example, a template cannot contain both `README.md` and `README.md.j2`, because both would produce `README.md`.

## Path excludes

`[path.exclude]` removes files or directories before path rewrites and rendering.

```toml
[path.exclude]
docs = { path = "docs", unless = "include_docs" }
```

If `unless` is set, the path is excluded unless that top-level context value is truthy. Missing exclude paths are ignored.

## Plugin config

Plugin-specific settings live under `[plugins.<name>]` and are validated by that plugin.

```toml
[plugins.uv]
python = "3.12"
scope = "install"
setup = "editable"
```

Core Plato does not understand the full schema of every plugin. It parses the TOML, merges setup-step overrides, converts the result to JSON, and sends it to the plugin.

## Setup steps

Setup steps run after the rendered project has been written to disk.

```toml
[[setup.steps]]
plugin = "uv"
source_path = "backend"
timeout_secs = 600
```

Fields:

- `plugin`: plugin name; Plato resolves `uv` to `plato-plugin-uv`.
- `source_path`: optional relative path inside the generated project. Defaults to `.`. The directory must exist in the rendered workspace.
- `timeout_secs`: optional plugin setup timeout. Defaults to 600 seconds. Must be greater than zero.
- any other keys: plugin config overrides for this step.

Merge rule:

```text
effective config = [plugins.<plugin>] + step-specific override keys
```

Example with two runs of the same plugin:

```toml
[plugins.uv]
python = "3.12"
scope = "install"
setup = "sync"
groups = ["dev"]

[[setup.steps]]
plugin = "uv"
source_path = "backend"

[[setup.steps]]
plugin = "uv"
source_path = "tools"
groups = ["lint"]
```

## Groups

Optional group files live next to `plato.toml`:

```text
plato.toml
plato.docker.toml
plato.frontend.toml
```

Apply them with:

```bash
plato init api my-api -g docker -g frontend
```

Groups can merge:

- `[template.context]`
- `[path.replace]`
- `[path.exclude]`
- `[plugins.<name>]`
- `[[setup.steps]]`

Setup steps from groups are appended in CLI order.

## Rendering rules

Files ending in `.j2` or `.mj` are rendered with MiniJinja and written without that extension. Non-template files are copied as bytes. Symlinks inside templates are rejected for safety.

On Unix, Plato preserves source permission bits, including executable scripts. Template-root paths must be directories; a regular file is never treated as an empty template.

Plato also registers Ansible-style regex filters:

```jinja2
{{ value | regex_replace('^py3-', '') }}
{{ value | regex_search('\d+') }}
{{ value | regex_findall('\d+') }}
{{ value | regex_escape }}
```
