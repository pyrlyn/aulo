---
status: in progress
priority: P1
complexity: 2
readiness: 50%
agent: Claude Code / claude-sonnet-5-5
---

# T1.11. Extract text-sanitize: terminal and bidi guard

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create text-sanitize (publish = false, dual licence, rust-version 1.86 so ketch can adopt it) merging cox-sanitize, rtok src/sanitize.rs and ketch changelog::sanitize, with the union of their tests; crate-level project files like change-preview; verify cargo clippy/fmt/test. Adoption by cox, rtok and ketch and the rust.md duplicate note wait for publication and creator permission.

Merge cox-sanitize, the token-reduction CLI src/sanitize.rs and ketch changelog::sanitize (a known duplicate in rust.md) into packages/crates/text-sanitize. All three projects adopt it.

Done when:

- one crate, three adopters, rust.md duplicate note removed
