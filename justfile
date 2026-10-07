default: check

check:
    cargo check --workspace --all-targets

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

fmt:
    cargo fmt --all

test:
    cargo test --workspace

# Regenerates docs/api from the protos. Needs network: the plugin runs on the BSR.
api-docs:
    buf generate proto
