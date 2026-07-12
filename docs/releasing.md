# Releasing Plato crates

Plato's packages depend on one another through versioned crates.io dependencies. Publish them in this order so Cargo can resolve every dependent package during packaging and installation:

1. `plato-plugin-api`
2. `plato-plugin-support`
3. First-party plugins (`plato-plugin-cargo`, `plato-plugin-git`, `plato-plugin-pip`, `plato-plugin-pnpm`, `plato-plugin-precommit`, and `plato-plugin-uv`)
4. `plato`

Before publishing each package, run:

```bash
cargo package -p <package> --no-verify
cargo publish --dry-run -p <package>
```

After the API and support crates are published, `just package-dependent` packages every dependent plugin and the core CLI. `just release-check` always validates formatting, Clippy, tests, and the independently packageable protocol crate.
