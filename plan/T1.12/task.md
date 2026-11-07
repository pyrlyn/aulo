---
status: in progress
priority: P0
complexity: 4
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.12. Extract mcp-host: MCP client with discovery and OAuth

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create mcp-host (publish = false, dual licence) from the cox-mcp client parts on rmcp 3.x with the fail-open and discovery tests; crate-level project files like change-preview; verify cargo clippy/fmt/test. cox adoption waits for publication.

Move cox-mcp client parts (connect_all that turns a failing server into a notice, .mcp.json discovery with ${VAR} expansion, OAuth with injectable store, elicitation mapping, deferred tools) into packages/crates/mcp-host on rmcp 3.x. The coding agent adopts it.

Done when:

- fail-open and discovery tests pass; the coding agent adopts it
