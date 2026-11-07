---
status: in progress
priority: P2
complexity: 3
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.7. Opus for remote audio streams

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Optional Opus encoding of VoiceService frames for low-bandwidth remote clients (PCM16 16 kHz stays the default).

Execution plan: optional Opus encode/decode of 20 ms PCM16 16 kHz frames (stacked on t6-1-audio-capture), behind a feature, PCM16 stays default; round-trip SNR test.

Status: implemented on branch t6-7-opus (`opus` 0.4 over bundled libopus 1.6.1 behind the off-by-default `opus` feature; proto value AUDIO_FORMAT_OPUS_16KHZ_MONO; decoder caps 1500 bytes and 6 frames; round-trip SNR about 10.4 dB at 24 kbit/s against a proposed 8 dB target). Merges with T6.1; Windows and Linux arm64 builds of the C codec are unverified.

Done when:

- round-trip test within the SNR target
