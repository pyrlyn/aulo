---
status: in progress
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.2. aulo-audio: playback

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 4 task(s)

Streaming playback (rodio on cpal) with volume, immediate stop for barge-in and a played-position counter for echo reference.

Execution plan: playback in aulo-audio (stacked on branch t6-1-audio-capture, so it merges with T6.1 once speech-capture is published); reuse a shared playback crate if one exists (check speech-capture and packages/), else rodio on cpal; volume, stop within 50 ms, played-position counter; fake-sink tests.

Status: implemented on branch t6-2-playback (`Playback` over speech-capture's new streaming `OutputDevice`, no rodio so cpal stays in one crate; volume with a ramp, stop() silences the next callback, played-position and underrun counters, backpressure on a full ring). Left: the ignored real-speaker test (`cargo test -p aulo-audio --test real_device -- --ignored --nocapture --test-threads=1`). Merges with T6.1.

Done when:

- stop() silences within 50 ms (measured with a fake sink)
