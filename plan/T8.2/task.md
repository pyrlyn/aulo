---
status: todo
priority: P0
complexity: 2
readiness: 0%
agent:
---

# T8.2. VAD backends

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 3 task(s)

earshot (pure Rust) by default, Silero through sherpa-onnx as an alternative; thresholds and hangover in config.

Done when:

- fixture test: speech/no-speech boundaries within 60 ms
