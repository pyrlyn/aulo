---
status: todo
priority: P2
complexity: 3
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T7.11. TTS: Windows system voices

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

Execution plan: 1. Windows engine in aulo-speech-system behind cfg(windows): WinRT SpeechSynthesizer.SynthesizeTextToStreamAsync through the windows module, WAV stream parsed and chunked into the bounded poll queue, voices from AllVoices. 2. Platform-neutral parsing and queue logic tested on every host. 3. `./gradlew build --target x86_64-pc-windows-msvc -p aulo-speech-system` locally; the real test runs on the Windows CI runner after a push. 4. Verify ktlint, fmt, workspace tests.

WinRT SpeechSynthesizer via the windows module, streamed into aulo-audio.

Done when:

- test on the Windows runner
