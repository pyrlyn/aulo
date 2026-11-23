# T7.1 done

The speech engine contract, in the new contract module `:aulo-speech`. PR: https://github.com/pyrlyn/aulo/pull/108.

- Interfaces: `SttEngine`, `TtsEngine`, `Vad`, `KeywordSpotter` and `TurnDetector`, on a shared `SpeechEngine` that carries `info` and `close`. Kotlin/Native has no destructors, so the registry closes engines it drops.
- How engines run: push, then poll; nothing suspends or blocks; output goes into caller buffers (a reused `Transcript`, a caller `FloatArray`).
- Allocation-free results:
  - `TtsPoll` is a value class;
  - a full STT queue is reported as dropped samples from `push`, not as an exception.
- Descriptions: `EngineId`, `EngineKind`, `LanguageSupport`, `Capabilities` (streaming, languages, offline, needs network) and `EngineInfo`.
- TTS values: `Voice`, `SpeechRate` (0.5 to 2.0, NaN rejected) and `TtsRequest`.
- Audio: `AudioFormat`, and `AudioFrame` as a window into the caller's array.
- `SpeechException` is the only error. Unavailable, Unsupported and Failed set `shouldFallBack`; Invalid and OutOfOrder do not.
- Caps on untrusted values:
  - error details: 512 bytes, one line;
  - engine ids: 64 bytes in the config alphabet;
  - names and voice ids: 128 bytes, one line;
  - language tags: BCP 47-shaped, 35 bytes, at most 256 per engine;
  - transcripts: `AuloEvent.Transcript`'s limits, applied without allocation.
- Design ported from the retired Rust crate (`crates/aulo-speech`, f9f53af). No new dependencies; the module depends only on `:aulo-types`.

How it was checked:

- `ktlintCheck` and `checkModuleGraph` (`CONTRACT`) pass.
- 27 tests (`TypesTest`, `ContractTest` with one fake engine per interface) pass on macOS arm64 and on Linux x64 and arm64 in Ubuntu 24.04 on Colima.
- The mingwX64 test binary links.
- The aulo tests still pass on macOS.
- CI runs on the PR.
