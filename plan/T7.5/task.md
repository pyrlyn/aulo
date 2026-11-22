---
status: todo
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T7.5. STT: whisper.cpp via whisper-rs

Stage: S7 · Area: speech · Depends on: T7.3, T1.13 · Blocks: 0 task(s)

Whisper backend reusing the speech-capture whisper feature (cox-voice Transcriber).

Execution plan: whisper STT engine in aulo-speech-system (or its own module) over speech-capture's `whisper` feature (path dependency until the module is published; branch stays unmerged until then), registered as an engine; fixture transcription test.

Done when:

- fixture transcription test
