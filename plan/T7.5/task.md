---
status: in progress
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T7.5. STT: whisper.cpp via whisper-rs

Stage: S7 · Area: speech · Depends on: T7.3, T1.13 · Blocks: 0 task(s)

Whisper backend reusing the speech-capture whisper feature (cox-voice Transcriber).

Execution plan: whisper STT engine in aulo-speech-system (or its own crate) over speech-capture's `whisper` feature (path dependency until the crate is published; branch stays unmerged until then), registered as an engine; fixture transcription test.

Status: implemented on branch t7-5-whisper (crate aulo-speech-whisper over speech-capture's `whisper` feature; the shared offline engine moved to aulo-speech-local on main). Fixture run with ggml-tiny.en: WER 0.000 over 22 words (about 0.1 s per 2.2 s clip on Apple Silicon), threshold 0.15 kept. Merges once speech-capture is published.

Done when:

- fixture transcription test
