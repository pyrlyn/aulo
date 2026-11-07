---
status: in progress
priority: P2
complexity: 3
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T7.8. STT: Apple SpeechAnalyzer (macOS 26+)

Stage: S7 · Area: speech · Depends on: T7.1, T0.3 · Blocks: 0 task(s)

Execution plan: 1. Check whether SpeechAnalyzer/SpeechTranscriber is reachable from objc2-speech (it is a Swift-only API); if not, fall back to SFSpeechRecognizer with requiresOnDeviceRecognition through objc2-speech, or report back before building a Swift bridge. 2. SttEngine in aulo-speech-system behind cfg(target_os = "macos"), offered only when the probe says on-device recognition is available for the language. 3. Bounded audio path, cancel, transcript cap; same main-run-loop rules as T7.10. 4. Capability probe test plus a manual check on fixture WAVs. 5. Verify clippy, fmt, workspace tests.

On-device system recognizer via objc2-speech; only offered when available.

Done when:

- capability probe test; manual check
