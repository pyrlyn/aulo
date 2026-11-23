---
status: in progress
priority: P0
complexity: 3
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T0.5. Spike: sherpa-onnx on macOS arm64

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Build the official sherpa-onnx Kotlin module (1.13.x) and run its examples for Parakeet TDT v3 STT, Silero VAD, keyword spotting and Kokoro TTS. Measure build time, binary size, model sizes, real-time factor and time to first audio. Write docs/spikes/sherpa-onnx.md with numbers and sources.

Done when:

- docs/spikes/sherpa-onnx.md with measured numbers
- go/no-go for sherpa-onnx as the default local speech backend

## Execution plan

The retired Rust spike (2026-10-07, sherpa-onnx 1.13.8 through the Rust crate) is the baseline. This
pass redoes it for the Kotlin/Native stack: there is no official Kotlin/Native module (the official
Kotlin API is JNI for Android and the JVM), so the harness binds the C API with cinterop. sherpa-onnx
2.0.3 is the latest release; it is used instead of 1.13.x.

1. Conan: sherpa-onnx is in neither Conan Center nor vcpkg. A recipe in `conan/recipes/sherpa-onnx`
   packages the official prebuilt release archives, checked against pinned SHA-256s (static libraries
   on macOS and Linux, the DLL build on Windows, since the static Windows archives are MSVC C++).
   `ConanInstallTask` exports the repository's recipes before installing; `conan { cinterop() }` lets
   a `.def` file list several static libraries.
2. Harness: a throwaway `spike-sherpa` module (not committed; its source is kept in
   `docs/spikes/sherpa-onnx/`) links the binding into one executable with `stt`, `vad`, `kws` and
   `tts` subcommands.
3. Measure on macOS arm64: build time, binary size, model sizes, load time, real-time factor, peak
   RSS, time to first audio; WER of Parakeet TDT v3 (en, ru) and Moonshine (en) on the FLEURS dev
   sets. Link for linuxX64, linuxArm64 and mingwX64; run the Linux binaries on Colima.
4. Set the WER threshold for T7.4 from those numbers.
5. Rewrite `docs/spikes/sherpa-onnx.md` with numbers, primary sources and dates, and a go/no-go.
   Record the separate helper process (`aulo-speech-sherpa`, creator's decision for T7.4).

Check: `docs/spikes/sherpa-onnx.md` has the measured numbers, the WER threshold and the verdict.
