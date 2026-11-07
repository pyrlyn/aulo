# T2.1. Cargo workspace skeleton

Stage: S2 · Area: infra · Depends on: T0.1 · Blocks: 6 task(s)

Virtual manifest (members = crates/*, resolver 3, edition 2024), workspace.package, workspace.dependencies with one-line reasons, workspace.lints (no unwrap/expect/panic outside tests), dev and dist profiles, mise.toml pinning Rust, justfile, .gitignore.

Execution plan: 1. Root Cargo.toml: virtual workspace, members crates/*, resolver 3, edition 2024, rust-version 1.99, license expression from D1, workspace.lints (deny unwrap_used, expect_used, panic, todo outside tests), dev profile line-tables-only, dist profile (fat LTO, codegen-units 1, strip). 2. crates/aulo: the surface crate with `aulo` and `aulod` bin targets printing their version, so the workspace has a default member. 3. mise.toml pinning Rust 1.99.0 with rustfmt and clippy; justfile (check, test, lint, fmt); rustfmt.toml. 4. Verify: `mise exec -- cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, run both binaries. 5. toolchain.md rows for rustc, cargo, mise, just.

Done when:

- cargo check, clippy and fmt --check pass on an empty workspace
