---
status: in progress
priority: P2
complexity: 3
readiness: 80%
agent: Claude Code / claude-sonnet-5-5
---

# T1.20. The local model runner: library target for in-process local inference

Stage: S1 · Area: shared · Depends on: T1.19 · Blocks: 0 task(s)

The local model runner is bin-only (serve, pool and mcp are pub(crate)). Expose a library crate (for example runa-serve) so aulod can embed local inference without a second process.

Execution plan: tracked in the local model runner as P16.2 (branch p16-2-lib, stacked on P16.1; done there as the runa-pool crate, waiting for a local model runner PR to merge); expose a narrow library API for the pool and a generation stream, with an integration test outside the local model runner binary.

Done when:

- aulo can call the local model runner's pool from a test
