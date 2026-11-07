---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T11.4. Streaming plugins over gRPC (speech engines, providers)

Stage: S11 · Area: plugins · Depends on: T11.2, T7.2, T3.2 · Blocks: 2 task(s)

Streaming kinds (STT, TTS, LLM provider) use aulo.plugin.v1 gRPC services over a Unix socket: the host spawns the plugin, health-checks and restarts it, and registers the engine in the registry.

Done when:

- example TTS plugin is switchable like a built-in engine
