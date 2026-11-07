---
status: in progress
priority: P0
complexity: 4
readiness: 70%
agent: Claude Code / claude-sonnet-5-5
---

# T1.15. Extract agent-loop: neutral turn state machine

Stage: S1 · Area: shared · Depends on: T1.1, T0.7 · Blocks: 2 task(s)

Execution plan: in a packages/crates worktree branched from t1-1-llm-wire, create agent-loop (publish = false, dual licence, rust-version 1.98) from the neutral part of cox-core: Submission/Event, parallel and exclusive tool dispatch, approvals, interrupt via CancellationToken, max steps; tools and approvals behind traits; coding-specific parts stay in cox; tests with a scripted provider for tool calls, approval and interrupt; verify cargo clippy/fmt/test. cox adoption (T1.16) waits for publication.

Per the T0.7 decision, extract the turn state machine (Submission/Event, parallel and exclusive tool dispatch, approvals, interrupt via CancellationToken, max steps) from cox-core into packages/crates/agent-loop. Coding-specific parts stay in the coding agent.

Done when:

- agent-loop tests with a scripted provider cover tool calls, approval and interrupt
