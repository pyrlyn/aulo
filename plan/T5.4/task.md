---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T5.4. Sandbox profiles for commands

Stage: S5 · Area: safety · Depends on: T5.1, T1.9 · Blocks: 1 task(s)

proc-sandbox modes read-only, workspace-write (default, workspace ~/aulo/workspace) and full access. A sandbox denial can be retried unsandboxed only after approval.

Done when:

- tests on macOS and Linux: write outside the workspace is denied
