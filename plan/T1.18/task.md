---
status: in progress
priority: P2
complexity: 2
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.18. Extract agent-host-config: register MCP servers in agent hosts

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create agent-host-config (publish = false, dual licence, MSRV of rtok) from rtok-agent-sdk register_mcp/unregister_mcp: touch only our entry, keep foreign bytes, write atomically, back up with file-backup (path dep in the same workspace); round-trip test keeps foreign keys byte-for-byte; crate-level project files like change-preview; verify cargo clippy/fmt/test. rtok adoption waits for publication.

Move register_mcp/unregister_mcp, backup and atomic JSON edits from rtok-agent-sdk into packages/crates/agent-host-config (foreign-config rule: touch only our entry, keep bytes, write atomically, back up with file-backup).

Done when:

- The token-reduction CLI adopts it; round-trip test keeps foreign keys byte-for-byte
