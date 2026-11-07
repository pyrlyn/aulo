---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T8.16. End-to-end voice test with fixtures

Stage: S8 · Area: voice · Depends on: T8.4, T4.9 · Blocks: 2 task(s)

CI test: WAV fixture in, real local STT, scripted provider, real local TTS, audio out; asserts transcript and non-empty audio. Runs headless.

Done when:

- runs in CI on Linux and macOS
