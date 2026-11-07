---
status: in progress
priority: P0
complexity: 3
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.1. aulo-audio: capture

Stage: S6 · Area: audio · Depends on: T1.13, T2.4 · Blocks: 3 task(s)

speech-capture behind an aulo trait: device by name or default, 16 kHz mono f32 frames of 20 ms into a lock-free ring buffer (ringbuf), overflow counted, never blocks the audio thread.

Execution plan: new crate aulo-audio with a capture trait over speech-capture (path dependency on the extracted crate until it is published; branch stays unmerged until then), device by name or default, 20 ms 16 kHz mono frames into a ringbuf, overflow counter; fake-device tests.

Status: implemented on branch t6-1-audio-capture (aulo-audio wraps speech-capture's new streaming `InputDevice`, added in packages/crates branch t1-13-speech-capture); merges once speech-capture is published. Manual check left: `cargo test -p aulo-audio --test real_device -- --ignored --nocapture` on macOS.

Done when:

- test with a fake device; manual check on macOS
