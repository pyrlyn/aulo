---
status: in progress
priority: P0
complexity: 3
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.13. Extract speech-capture: microphone capture and resampling

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 2 task(s)

Execution plan: in a packages/crates worktree branched from s1-setup, create speech-capture (publish = false, dual licence) merging cox-voice capture (cpal default device, downmix, rubato to 16 kHz) with runa-media decode/resample/energy VAD; whisper behind a feature; crate-level project files like change-preview; verify cargo clippy/fmt/test. cox and runa adoption waits for publication.

Merge cox-voice capture (cpal default device, downmix, rubato to 16 kHz) with runa-media decode/resample/energy VAD into packages/crates/speech-capture. The coding agent and the local model runner adopt it; whisper transcription stays behind a feature.

Done when:

- The coding agent and runa build against it; duplicated rubato/whisper code removed
