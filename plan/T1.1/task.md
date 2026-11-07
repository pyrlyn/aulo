---
status: in progress
priority: P0
complexity: 4
readiness: 80%
agent: Claude Code / claude-opus-5-5
---

# T1.1. Extract llm-wire: provider contract and neutral types

Stage: S1 · Area: shared · Depends on: T0.1, T0.7 · Blocks: 3 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create llm-wire (publish = false, dual licence) with the provider trait and neutral types copied from cox-protocol plus their tests; Replay/Scripted fakes behind `test-util`; crate-level AGENTS.md, README.md, plan.md, todo.md, done.md, roadmap.md, ideas.md, toolchain.md like change-preview; verify cargo clippy/fmt/test in packages/crates. cox re-export waits until the crate is published (crates.io or a git dependency), because a path dependency breaks cox CI.

Move the provider trait and the neutral types (Request, ProviderEvent, Caps, ToolSpec, Usage, Risk) out of cox-protocol into packages/crates/llm-wire. No behaviour change. Keep the Replay/Scripted fakes behind a test-util feature.

Done when:

- llm-wire builds and its tests pass in packages/crates
- cox-protocol re-exports the moved types (no API break for the coding agent)

Status: the first criterion holds. `llm-wire` lives at `rust/llm-wire` in pyrlyn/crates; aulo pins it at `f0deed54` (`Cargo.toml` `[workspace.dependencies]`) and `aulo-providers`, `aulo-server` and `llm-router` build on it. At that rev `cargo test --locked --all-features --manifest-path <checkout>/rust/llm-wire/Cargo.toml` passes 24 of 24 tests, `test-util` included. No aulo crate redefines a moved type (`aulo_store::models::Usage` is a database row, not the wire `Usage`). The coding agent re-export is the remaining criterion. It is work in the coding agent repository and waits until `llm-wire` is published, because a path dependency breaks the coding agent CI. Nothing is left in this repository.
