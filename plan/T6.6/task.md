---
status: todo
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T6.6. Microphone permission handling

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Detect a denied microphone (macOS TCC, Windows privacy settings) and report a clear notice with the fix instead of silence.

Execution plan: a permission probe trait in aulo-audio (stacked on t6-1-audio-capture) with macOS TCC and Windows privacy implementations behind cfg, mapping a denied microphone to a notice with the fix; fake-probe tests for the notice text.

Done when:

- notice text covered by a test with a fake permission probe
