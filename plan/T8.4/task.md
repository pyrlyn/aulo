---
status: todo
priority: P0
complexity: 4
readiness: 0%
agent:
---

# T8.4. VoiceService over gRPC

Stage: S8 · Area: voice · Depends on: T8.1, T3.4 · Blocks: 4 task(s)

Bidi Talk stream: clients send PCM (or Opus) frames and control; the server runs the pipeline and streams state, transcripts and TTS audio back. Lets the desktop app and remote clients use the server-side pipeline; local mode uses the daemon's own devices.

Done when:

- integration test streams a WAV in and receives transcript and audio out
