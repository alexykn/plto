release:
    cargo build --release

build:
    cargo build

test:
    @cargo test

fmt:
    @cargo fmt --all

clippy:
    @cargo clippy --fix --all-targets --allow-dirty -- -D warnings -W clippy::pedantic

package-api:
    cargo package -p plato-plugin-api --no-verify

# Run after plato-plugin-api and plato-plugin-support are published in that order.
package-dependent:
    cargo package -p plato-plugin-support --no-verify
    cargo package -p plato-plugin-cargo --no-verify
    cargo package -p plato-plugin-git --no-verify
    cargo package -p plato-plugin-pip --no-verify
    cargo package -p plato-plugin-pnpm --no-verify
    cargo package -p plato-plugin-precommit --no-verify
    cargo package -p plato-plugin-uv --no-verify
    cargo package -p plato --no-verify

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
