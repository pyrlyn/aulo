---
status: in progress
priority: P0
complexity: 2
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T7.1. aulo-speech: engine contracts

Stage: S7 · Area: speech · Depends on: T2.4 · Blocks: 11 task(s)

Traits: SttEngine (push frames, partial and final transcripts), TtsEngine (stream text in, audio chunks out, voices, rate), Vad, KeywordSpotter, TurnDetector. Capabilities: streaming, languages, offline, needs network.

Done when:

- contract tests with fake engines

## Execution plan

The Rust workspace had this contract (`crates/aulo-speech`, commit f9f53af, removed in #81). Port its design to Kotlin, not its code line by line.

1. New contract module `:aulo-speech` (`ModuleRole.CONTRACT`, common source set, depends only on `:aulo-types`); add it to `settings.gradle.kts`.
2. `commonMain/kotlin/aulo/speech/`:
   - `SpeechException`: Unavailable, Unsupported, Failed (engine detail capped at 512 bytes, one line), Invalid, OutOfOrder; `shouldFallBack`.
   - `AudioFormat`, `AudioFrame` (a window into the caller's `FloatArray`, so pushing copies nothing), `PIPELINE_SAMPLE_RATE_HZ`.
   - `EngineId` (config alphabet, 64 bytes), `EngineKind`, `LanguageSupport`, `Capabilities`, `EngineInfo`; untrusted names capped.
   - `SttEngine`, `Transcript` (a reused buffer, text capped like `AuloEvent.Transcript`), `SttPoll`.
   - `TtsEngine`, `Voice` (id at most 128 bytes, as `aulo.v1.Voice`), `SpeechRate`, `TtsRequest`, `TtsPoll`.
   - `Vad`, `KeywordSpotter`, `TurnDetector` and their results.
   - Push then poll, no suspend functions: engines run on the speech worker and must not block.
3. `commonTest`: unit tests per type, and contract tests that drive one fake engine per interface through the interface: partials then one final, buffer reuse, bounded queue, call order, fallback past a failing engine, TTS chunks and cancel, detectors.
4. Docs: `AGENTS.md` bullet, `toolchain.md` if anything new.
5. Verify: `ktlintCheck`, `checkModuleGraph`, tests on macOS, Linux x64/arm64 on Colima, mingwX64 link; push, PR, CI.
