release:
    cargo build -p plto --bins --release --locked

build:
    cargo build --workspace --bins

test:
    @cargo test --workspace --all-targets

fmt:
    @cargo fmt --all

clippy:
    @cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic

package-api:
    cargo package -p plto-plugin-api --locked --no-verify

# Run after plto-plugin-api and plto-plugin-support are published in that order.
package-dependent:
    cargo package -p plto-plugin-support --locked --no-verify
    cargo package -p plto-plugin-cargo --locked --no-verify
    cargo package -p plto-plugin-git --locked --no-verify
    cargo package -p plto-plugin-pip --locked --no-verify
    cargo package -p plto-plugin-pnpm --locked --no-verify
    cargo package -p plto-plugin-precommit --locked --no-verify
    cargo package -p plto-plugin-uv --locked --no-verify
    cargo package -p plto --locked --no-verify

check: fmt clippy test

release-check: check package-api

major_upgrade:
    @cargo upgrade -i

minor_upgrade:
    @cargo upgrade

cargo_update:
    @cargo update

update: minor_upgrade cargo_update

upgrade: major_upgrade cargo_update
