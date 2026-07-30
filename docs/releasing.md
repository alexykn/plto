# Releasing Plto crates

Plto's packages depend on one another through versioned crates.io dependencies. Publish them in this order so Cargo can resolve every dependent package during packaging and installation:

1. `plto-plugin-api`
2. `plto-plugin-support`
3. First-party plugins (`plto-plugin-cargo`, `plto-plugin-git`, `plto-plugin-pip`, `plto-plugin-pnpm`, `plto-plugin-precommit`, and `plto-plugin-uv`)
4. `plto`

`plto` installs the `plto` executable and every first-party plugin executable together.
Release archives contain the same binaries under `bin/`.

Before publishing each package, run:

```bash
cargo package -p <package> --locked --no-verify
cargo publish --dry-run -p <package> --locked
```

After the API and support crates are published, `just package-dependent` packages every dependent
plugin and the bundled CLI. `just release-check` always validates formatting, Clippy, tests, and
the independently packageable protocol crate.
