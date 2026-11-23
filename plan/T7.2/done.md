# T7.2. Done

PR: https://github.com/pyrlyn/aulo/pull/109 (stacked on #108, T7.1).

## What was done

- `:aulo-speech` gained `EngineSpec` and `EngineChoice`. Their model and voice labels are bounded, and a fallback list holds at most 16 engines.
- New `:aulo-voice` module:
  - `SttRegistry` and `TtsRegistry` refuse duplicate ids. An unknown id counts as unavailable, so it falls back.
  - `candidates` resolves chat over bot over config.
  - `ActiveStt` and `ActiveTts` build engines lazily and fall back with a capped WARN notice per skipped engine.
  - `report` retires an engine that faults mid-reply, and `switch` takes effect at the next begin.
  - Dropped engines are closed. After every candidate has failed, the next begin starts over.
  - Anything an engine or factory throws besides `SpeechException` is treated as an engine fault.
- New `:aulo-testkit` module: the fake engines, moved out of `aulo-speech`'s `ContractTest` and now shared by the `aulo-speech` and `aulo-voice` tests.
- `:aulo-config`: `EngineConfig.choice(key, provenance)` builds an `EngineChoice`. Errors name the key and the layer, never the value.
- The design is ported from the retired Rust crates (`aulo-voice`, `aulo-speech`; commit 0508ff9).
- Out of scope, left to later tasks: wiring `VoiceService.SetVoice`, `aulo voice use` and voice commands to `switch`.

## How it was checked

- `ktlintCheck` and `checkModuleGraph` pass.
- macOS arm64: `aulo-speech` 27 tests, `aulo-voice` 14, `aulo-config` 27.
- Linux x64 and arm64 on Colima: the same test binaries pass.
- mingwX64: the test binaries link.
