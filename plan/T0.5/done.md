# T0.5. Done

sherpa-onnx 2.0.3 runs from Kotlin/Native through its C API, packaged by a local Conan recipe over the
official prebuilt archives (`conan/recipes/sherpa-onnx`), on macOS arm64, Linux x86_64 and arm64, and
Windows x86_64 (linked; not run). `docs/spikes/sherpa-onnx.md` has the numbers, sources and verdict: GO
for STT (Parakeet TDT v3), VAD and KWS; Kokoro TTS is conditional on a phonemizer outside sherpa-onnx.
The proposed WER threshold for T7.4 is ≤ 10% for en and ru on FLEURS dev clips; it awaits the creator's
confirmation, and so does the effect of the espeak-ng removal on spec §17 D9.

The build gained local Conan recipes (`conan/recipes/<name>`, exported before every install) and
`conan { cinterop() }` without a static library, for packages of several or shared libraries.

Checked: the harness (`docs/spikes/sherpa-onnx/`) linked for all four targets and ran on macOS and, in
Colima `ubuntu:24.04` containers, on Linux arm64 and x86_64 (emulated); FLEURS en and ru dev WER runs;
`./gradlew test ktlintCheck` and build-logic's `test ktlintCheck` pass.
