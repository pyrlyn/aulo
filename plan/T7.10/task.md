---
status: in progress
priority: P2
complexity: 4
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T7.10. TTS: macOS system voices

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

AVSpeechSynthesizer writing buffers into aulo-audio (not straight to the speaker, so barge-in and AEC still work). Lists installed voices.

Done when:

- manual check; voice-list test behind a macOS cfg

## Execution plan

A spike on macOS 27 settled two facts that shape the design:

- `AVSpeechSynthesizer.write(_:toBufferCallback:)` delivers its buffers through the main run
  loop. With no run loop on the main thread no buffer ever arrives. aulod's main thread is busy,
  so the synthesizer runs in a helper process whose main thread runs the run loop. This is the
  same helper-process pattern as the sherpa STT engines (T7.4), and a crash in the speech
  framework stays out of aulod.
- Each voice's `audioFileSettings[AVSampleRateKey]` matches the rate of the buffers it writes
  (16 000, 22 050 or 48 000 Hz, mono float). The voice list can therefore carry each voice's rate,
  `begin` can return that format at once, and nothing needs resampling.

Steps:

1. Move the stdin and stdout pipes of `aulo-speech-sherpa` into `aulo-speech-helper`, so a second
   helper executable reuses them.
2. Add `AudioRing` to `aulo-voice`: a fixed float ring with one writer and one reader, where a
   full ring makes the writer wait and closing it releases the writer. Once T7.13 (PR #120) is on
   main, `OpenAiTts` moves to it as well.
3. Add TTS frames to the wire (`Wire.kt`):
   - `SPEAK` (voice, language, rate), `TEXT`, `FINISH` and `CANCEL`, from aulod;
   - `VOICES` (id, name, language and rate per voice, capped), `AUDIO` and `END`, from the helper.

   Every payload is capped before it is read.
4. `HelperTts`, the `TtsEngine` aulod drives over a `HelperLink`. Its factory waits for the
   helper's voice list, bounded by a startup timeout. `begin` returns the voice's rate. A reader
   coroutine fills each reply's own `AudioRing`; a full ring stops the pipe read, and a cancelled
   reply's audio is dropped. Helper failures fall back as with `HelperStt`.
5. `serveTts`, the helper side, over a `SpeechSynth` interface: it queues the texts of a reply,
   speaks them in order, sends `END` after `FINISH`, and on `CANCEL` stops and drops late audio.
6. A new module, `aulo-speech-system` (SURFACE): the executable `aulo-speech-system tts`. On
   macOS it serves on a background thread while the main thread runs `CFRunLoopRun`; on other
   targets it says it is unsupported. `registerMacVoices(helper)` in `aulo-speech-helper` adds
   the `macos-voices` engine.
7. Tests:
   - wire round trips and caps;
   - `HelperTts` and `serveTts` over the in-memory link with a fake synth (order, back-pressure,
     cancel, errors, a broken helper);
   - on macOS, a test that runs the linked helper, lists the voices and speaks a sentence.
8. AGENTS.md and research.md (Apple's documentation and the spike).

Check: `./gradlew :aulo-speech-helper:allTests :aulo-speech-system:allTests :aulo-speech-sherpa:allTests ktlintCheck checkModuleGraph`, every target links, and a manual listen to the helper's audio.
