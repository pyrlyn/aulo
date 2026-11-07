---
status: in progress
priority: P1
complexity: 2
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.14. Extract config-schema: schema-checked config helpers

Stage: S1 · Area: shared · Depends on: none · Blocks: 0 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create config-schema (publish = false, dual licence, MSRV low enough for ketch and rtok) from the duplicates in cox-config, rtok config/layers.rs and ketch toml_file.rs: stale-schema test helper, figment layering with per-key provenance, toml_edit editing; union of their tests; crate-level project files like change-preview; verify cargo clippy/fmt/test. Adoption by ketch or rtok waits for publication.

rust.md lists "config with schema and one owner" as a known duplicate (cox-config, rtok config/layers.rs, the package manager toml_file.rs). Extract the stale-schema test helper, figment layering with per-key provenance and toml_edit editing into packages/crates/config-schema.

Done when:

- The package manager or the token-reduction CLI adopts it; helper tests pass
