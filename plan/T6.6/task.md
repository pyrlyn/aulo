---
status: in progress
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.6. Microphone permission handling

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Detect a denied microphone (macOS TCC, Windows privacy settings) and report a clear notice with the fix instead of silence.

Execution plan: a permission probe trait in aulo-audio (stacked on t6-1-audio-capture) with macOS TCC and Windows privacy implementations behind cfg, mapping a denied microphone to a notice with the fix; fake-probe tests for the notice text.

Status: implemented on branch t6-6-mic-permission (MicPermission probe: macOS AVAudioApplication.recordPermission, Windows consent-store registry via windows-registry, Linux Unknown; notices name the settings path; `Capture::start_checked`; SilenceWatch for all-zero input). NotDetermined blocks capture too (creator's decision: macOS can kill a launchd process that prompts, so the desktop app asks). The Windows probe is compiled, never run. Merges with T6.1.

Done when:

- notice text covered by a test with a fake permission probe
