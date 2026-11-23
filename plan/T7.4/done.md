# T7.4. Done

PR: https://github.com/pyrlyn/aulo/pull/115 (stacked on #110, T7.3; carries #108, T7.2, and #112, the T0.5 spike). Needs pyrlyn/packages#9 (`spawnPipes`); CI pins kotlin-kit to its head until it merges.

## What was done

- The creator's decisions: sherpa-onnx through a local Conan recipe over the official prebuilt archives; the engine runs in a separate helper process; WER at most 10% for Parakeet in en and ru on FLEURS dev.
- `aulo.conan` `bundleSharedLibraries` copies a package's shared libraries next to executables and test binaries.
- `:aulo-speech-helper`:
  - `Wire.kt`, the pipe protocol, with every payload capped before it is read;
  - `HelperStt`, an `SttEngine` that never blocks, uses a bounded queue that counts drops, and stops its helper off the caller's thread. A crash, garbage, a bad frame or a missing final is a `SpeechException`, so the registry falls back;
  - `serveStt`, the helper side;
  - `ProcessLink`, a helper over kotlin-kit `spawnPipes`, whose stderr is drained and discarded;
  - `registerSherpa`, which adds `sherpa-parakeet` and `sherpa-moonshine` to an `SttRegistry`.
- kotlin-kit `process` gained `spawnPipes`/`PipedChild` (pyrlyn/packages#9): a long-lived child in its own process group or job, with reads polled under a guard so `close` never frees a pipe under a call, and SIGPIPE kept from ending the parent.
- `:aulo-speech-sherpa`, the helper executable: the only module that links sherpa-onnx. Parakeet gives one final per utterance; Moonshine re-decodes the growing utterance for partials.
- `aulo-models`: Moonshine base en quantized, pinned with its sources in `research.md` §4.14.
- Docs: AGENTS.md layout bullet and a toolchain row for sherpa-onnx.

## How it was checked

- macOS arm64:
  - every test in both modules, `ktlintCheck` and `checkModuleGraph`;
  - `HelperProcessTest` with the Gradle-linked helper (a missing model, a missing helper, a mismatched model, and a full Russian utterance);
  - Gradle passing `AULO_SHERPA_HELPER` to the test task, confirmed with a throwaway probe test.
- Linux arm64 in Colima:
  - 22 helper tests;
  - `HelperProcessTest`, including the full utterance through the real helper.
- All four targets link with the shared libraries bundled.
- WER on FLEURS dev:
  - Parakeet en_us 7.87% (394 clips) and ru_ru 7.75% (356 clips), against a 10% threshold;
  - Moonshine en_us 13.09%, against 15%.
- Not run locally: the kit's Windows tests. CI's windows-2025 job covers `spawnPipes` through `HelperProcessTest`.

## Left for later

aulod does not build a voice pipeline yet. The task that wires engines in calls `registerSherpa` and adds the helper lookup and packaging.
