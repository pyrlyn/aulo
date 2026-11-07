---
status: todo
priority: P0
complexity: 4
readiness: 0%
agent:
---

# T12.1. aulo-app: UI-agnostic client core

Stage: S12 · Area: desktop · Depends on: T0.2, T3.7 · Blocks: 2 task(s)

Rust core the native apps drive, modelled on cox-app: a tonic client of aulod (local socket or remote URL with token and pin, starts the local daemon when absent), a Timeline fold (events in, keyed block patches out), a Controller that coalesces patches to at most one batch per 16 ms frame, chat list and inbox (approvals, takeovers, finished turns), one Intent enum dispatched to submissions. No UI toolkit in its dependency tree.

Done when:

- scenario tests: a replayed event log folds to the same patches as the live run
- deps test: no UI or CLI crate in aulo-app's tree
