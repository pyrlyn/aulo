# T8.1. aulo-voice: conversation pipeline

Stage: S8 · Area: voice · Depends on: T6.2, T7.2, T7.15, T4.8 · Blocks: 10 task(s)

frames, VAD, STT, end of turn, Submission, streamed reply, normalizer, TTS, playback as tokio tasks with bounded channels. States: Idle, Listening, Thinking, Speaking. One audio owner at a time.

Done when:

- end-to-end test with fake engines drives a full turn
