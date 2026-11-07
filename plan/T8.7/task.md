---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T8.7. End-of-turn detection

Stage: S8 · Area: voice · Depends on: T8.2 · Blocks: 0 task(s)

Silence timeout (700 ms default) plus optional Smart Turn v3 semantic end-of-turn model (BSD-2, ONNX) to avoid cutting the user off mid-thought.

Done when:

- fixture test: a mid-sentence pause does not end the turn
