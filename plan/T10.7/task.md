---
status: todo
priority: P2
complexity: 2
readiness: 0%
agent:
---

# T10.7. Register aulo in agent hosts

Stage: S10 · Area: mcp · Depends on: T10.5, T1.18 · Blocks: 0 task(s)

aulo mcp install/uninstall --host claude-code|cursor|codex through agent-host-config; touches only aulo's entry, backs up, writes atomically.

Done when:

- round-trip test keeps other entries byte-for-byte
