---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T9.1. Shell tool

Stage: S9 · Area: control · Depends on: T5.4, T5.2 · Blocks: 2 task(s)

Run a command in the sandbox with timeout, output cap, streamed output events, cwd and env allowlist. Long output is archived losslessly and the model sees a truncated view with an id (cox expand pattern).

Done when:

- tests: timeout kills the process tree; truncated output is recoverable
