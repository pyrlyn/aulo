---
status: todo
priority: P2
complexity: 4
readiness: 0%
agent:
---

# T11.7. WASM plugins

Stage: S11 · Area: plugins · Depends on: T11.2 · Blocks: 0 task(s)

In-process sandboxed plugins (extism, as cox-plugin, or wasmtime components) for tools and hooks; deadline and memory cap per call; network only through an allowlisted host function.

Done when:

- tests: deadline kills a looping guest; denied host is blocked
