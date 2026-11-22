---
status: todo
priority: P2
complexity: 3
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.7. Opus for remote audio streams

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Optional Opus encoding of VoiceService frames for low-bandwidth remote clients (PCM16 16 kHz stays the default).

Execution plan: optional Opus encode/decode of 20 ms PCM16 16 kHz frames (stacked on t6-1-audio-capture), behind a feature, PCM16 stays default; round-trip SNR test.

Done when:

- round-trip test within the SNR target
