---
status: in progress
priority: P1
complexity: 3
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.17. Extract agent-ext: skills and shell hooks

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 2 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create agent-ext (publish = false, dual licence) from cox-ext: SKILL.md discovery (index line, deferred skill tool) and Claude-Code-style shell hooks (JSON on stdin, exit 2 blocks, HookChain, fail open); crate-level project files like change-preview; verify cargo clippy/fmt/test. cox adoption waits for publication.

Move SKILL.md discovery (index line in prompt, deferred skill tool) and Claude-Code-style shell hooks (JSON on stdin, exit 2 blocks, HookChain) from cox-ext into packages/crates/agent-ext. The coding agent adopts it.

Done when:

- skills and hooks tests pass in both repos
