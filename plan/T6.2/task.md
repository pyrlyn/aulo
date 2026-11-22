---
status: todo
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.2. aulo-audio: playback

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 4 task(s)

Streaming playback with volume, immediate stop for barge-in and a played-position counter for echo reference.

Execution plan: playback in aulo-audio (stacked on branch t6-1-audio-capture, so it merges with T6.1 once speech-capture is published); reuse a shared playback module if one exists (check speech-capture and packages/), else a platform playback path; volume, stop within 50 ms, played-position counter; fake-sink tests.

Done when:

- stop() silences within 50 ms (measured with a fake sink)
