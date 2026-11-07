---
status: todo
priority: P2
complexity: 3
readiness: 0%
agent:
---

# T8.15. Keep talking while the agent works

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 0 task(s)

New utterances during a running turn are queued as steering messages instead of being dropped or forcing an interrupt.

Done when:

- test: steering text reaches the running turn
