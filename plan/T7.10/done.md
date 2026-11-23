# T7.10 done

aulod speaks with the installed macOS voices through AVSpeechSynthesizer's `writeUtterance:toBufferCallback:`, so the audio comes back into the pipeline instead of going to the speaker.

- `:aulo-speech-system`, a new surface module, is the `aulo-speech-system tts` helper (macOS only). Its main thread runs `CFRunLoopRun`, because the synthesizer delivers buffers only there; the pipe protocol runs on an I/O thread. Each voice's rate is listed with it, so nothing is resampled. `SpeechRate` maps onto Apple's rate scale by points measured on macOS 27.
- `:aulo-speech-helper`:
  - `TtsWire.kt` adds the `SPEAK`, `TEXT`, `VOICE` and `END` frames, all bounded.
  - `HelperTts` is the `TtsEngine` aulod drives. It picks voices by id, then by language, using the system defaults.
  - `serveTts` is the helper side over a `SpeechSynth`.
  - `HelperPipe` now holds the queue, writer and failure handling shared with `HelperStt`.
  - `registerMacVoices` adds the `macos-voices` engine.
- `:aulo-voice`'s `AudioRing` is the bounded float ring each reply's audio goes through. A full ring holds the helper back; a cancel drops late audio.
- The helper stdio pipes moved from `aulo-speech-sherpa` into `aulo-speech-helper`.
- Sources and measurements are in `research.md` §4.18.

PR: https://github.com/pyrlyn/aulo/pull/123

Checked with:

- `./gradlew :aulo-speech-helper:allTests :aulo-speech-system:allTests :aulo-speech-sherpa:allTests :aulo-voice:allTests ktlintCheck checkModuleGraph`;
- compiling for linuxX64, linuxArm64 and mingwX64;
- `SystemHelperTest`, which drives the linked helper: voice list, non-silent speech, cancel, and an unknown voice;
- a manual listen to Milena (ru-RU) and Samantha (en-US). 180 voices were listed.
