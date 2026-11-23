# Spike T0.5: sherpa-onnx from Kotlin/Native

Verdict: **GO** for sherpa-onnx 2.0.3 as the default local backend for speech-to-text (Parakeet TDT v3),
VAD (Silero) and keyword spotting. **Conditional** for Kokoro TTS: since 2.0.0 sherpa-onnx has no
phonemizer, and with the shipped lexicons Kokoro silently drops 1.5–6.2% of English words (R6). Kokoro
needs a phonemizer outside sherpa-onnx before it can be the default voice; which one is the creator's call.

Checked 2026-11-23. This pass replaces the one of 2026-10-07, which used sherpa-onnx 1.13.8 through the
Rust crate. The harness source is in [`sherpa-onnx/`](sherpa-onnx/); it was built as a throwaway Gradle
module and is not part of the build.

## Setup

| Item | Value |
| --- | --- |
| Machine | Apple M3 Max (12 performance + 4 efficiency cores, `sysctl hw.perflevel0/1.physicalcpu`), 64 GiB, macOS 27.0.1 |
| Linux | Colima 8-CPU aarch64 VM on the same machine, `ubuntu:24.04` (glibc 2.39) containers; x86_64 under emulation |
| Toolchain | Kotlin/Native 2.4.21, Conan 2.33.0 (both pinned in the repository) |
| sherpa-onnx | v2.0.3, published 2026-11-22, the latest release ([release](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v2.0.3), GitHub API checked 2026-11-23). Apache-2.0 ([LICENSE @ v2.0.3](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.3/LICENSE)). In neither Conan Center nor vcpkg. |
| Binding | The C API (`sherpa-onnx/c-api/c-api.h` @ v2.0.3) through cinterop. The official Kotlin API is JNI (Android and the JVM), which Kotlin/Native cannot use. |
| Package | Local Conan recipe `conan/recipes/sherpa-onnx`: downloads the official prebuilt archive of the host platform and checks its pinned SHA-256 (`conandata.yml`). |
| Test audio | FLEURS dev sets, `en_us` (394 clips, 62.8 min) and `ru_ru` (356 clips, 64.7 min), converted to 16 kHz 16-bit PCM with `afconvert` ([google/fleurs](https://huggingface.co/datasets/google/fleurs) at commit `70bb2e84`, CC-BY-4.0, HF API checked 2026-11-23); the official KWS test clips. All timings are wall clock, CPU provider, 4 threads unless stated. |

### Which archive each platform links

| Platform | Archive (v2.0.3) | Size | How it links | Shipped next to the binary |
| --- | --- | --- | --- | --- |
| macOS arm64 | `osx-arm64-static-lib` | 22.1 MB | 13 static libraries + libc++ + Foundation | nothing |
| Linux x86_64 | `linux-x64-shared-lib` | 10.1 MB | `-lsherpa-onnx-c-api`, RPATH `$ORIGIN` | `libsherpa-onnx-c-api.so` 5.8 MB, `libonnxruntime.so` 27.0 MB |
| Linux arm64 | `linux-aarch64-shared-cpu-lib` | 12.8 MB | the same | 5.2 MB + 34.6 MB |
| Windows x86_64 | `win-x64-shared-MT-Release-lib` | 8.6 MB | import library `sherpa-onnx-c-api.lib` (lld, mingw) | `sherpa-onnx-c-api.dll`, `onnxruntime.dll`, `onnxruntime_providers_shared.dll` |

The Linux static archives do not link: they need a newer libstdc++ (`std::filesystem`, `std::pmr`,
`__throw_bad_array_new_length`) than Kotlin/Native's GCC 8 sysroot has. The shared ones need only
GLIBC 2.17 and GLIBCXX 3.4.19 (`objdump -T`). The Windows static archives are MSVC C++ objects, which a
mingw link cannot take; the MT build links the C runtime statically, so no Visual C++ redistributable is
needed. macOS and Linux checksums match the release's `checksum.txt`; it lists no Windows archive, so that
one was hashed here.

## Models

All from the k2-fsa release pages (`github.com/k2-fsa/sherpa-onnx/releases/download/<tag>/<asset>`);
digests from the GitHub API, checked 2026-11-23.

| Role | Asset (tag) | Archive | On disk | SHA-256 | Licence |
| --- | --- | --- | --- | --- | --- |
| VAD | `silero_vad_v5.onnx` (asr-models) | 2.3 MB | 2.2 MiB | `6b99cbfd…c274396` | MIT ([silero-vad](https://github.com/snakers4/silero-vad/blob/master/LICENSE)) |
| STT | `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2` (asr-models, updated 2026-11-07) | 487 MB | 643 MiB | `1495e91b…72b14b0f9` | CC-BY-4.0, attribution required ([model card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)) |
| STT | `sherpa-onnx-moonshine-base-en-quantized-2026-02-27.tar.bz2` (asr-models) | 111 MB | 135 MiB | `43232c1d…c0e891` | MIT (`LICENSE` in the archive) |
| KWS | `sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2` (kws-models), English | 17.6 MB | 19 MiB | `f170013b…af9b6561a` | Apache-2.0 (README in the archive) |
| TTS | `kokoro-multi-lang-v1_0.tar.bz2` (tts-models, updated 2026-10-26) | 350 MB | 394 MiB | `60703707…0e0f7a4` | Apache-2.0 ([model card](https://huggingface.co/hexgrad/Kokoro-82M)); the archive still carries `espeak-ng-data` (GPL-3.0), which 2.x never reads |

Full digests: Parakeet `1495e91b59d7fc53ddca91f3431758ec99f8fa253fdb0e1e851be0f72b14b0f9`, Moonshine
`43232c1d13013d37317163baec3135bd771a186a4356f28c889bab453bb0e891`, KWS
`f170013b4716e41b62b9bfd809687c207cef798ef9bc6534d524e17af9b6561a`, Kokoro
`60703707bc05ccd48d60e1eb723f38b07d6e474795a0b6a588d7220400e0f7a4` (was `c5f7e2d2…` on 2026-10-07: the
asset was re-uploaded), Silero `6b99cbfd39246b6706f98ec13c7c50c6b299181f2474fa05cbc8046acc274396`.

## Results

### Build

| Measurement | Result |
| --- | --- |
| Cold build of the harness, macOS (module cleaned, package removed from the Conan cache, Gradle daemon warm) | 27 s, including the archive download and checksum |
| macOS executable, all four engines | 28.8 MB; 20.1 MB stripped. No sherpa or onnxruntime library to ship. |
| Linux executables | 1.2 MB each, plus the two shared libraries above |
| Windows executable | 1.2 MB, plus the three DLLs. Linked, not run: there is no Windows host here. |
| Conan package size | macOS 97 MB (static libraries), Linux x86_64 32 MB, Linux arm64 38 MB, Windows 23 MB |

### Speech-to-text

WER with a plain normalizer (NFKC, lower case, ё→е, punctuation removed; `wer.py` in the harness
directory). FLEURS writes numbers as digits and both models say them as words, so the WER without the
utterances that contain digits is given too.

| Model | Set | WER | WER, no digits | RTF | Load | Peak RSS |
| --- | --- | --- | --- | --- | --- | --- |
| Parakeet TDT v3 int8 | en, 394 clips | **7.84%** | 5.88% | 0.071 | 0.81 s | 1.51 GB |
| Parakeet TDT v3 int8 | ru, 356 clips | **7.75%** | 6.02% | 0.070 | 0.82 s | 1.79 GB |
| Moonshine base en, quantized | en, 394 clips | 13.09% | 10.58% | 0.017 | 0.31 s | 0.57 GB |
| Same, without its 12 empty outputs | en, 382 clips | 10.27% | 8.02% | | | |

- Parakeet threads (one 18 s clip): 4 → RTF 0.070, 2 → 0.130, 1 → 0.250; RSS 1.38–1.49 GB.
- Moonshine returned an empty transcript for 12 of 394 clips (6–29 s long), every time; Parakeet
  transcribed them correctly.
- Linux arm64 in the VM, same five clips, same text as macOS: **RTF 0.030 against 0.073 on macOS**.
  The prebuilt macOS static library is 2.4 times slower than the Linux arm64 shared one on the same
  silicon; the cause is not known (R4).
- Linux x86_64 under emulation produced the same text and VAD segments (timings meaningless).

### VAD, keyword spotting, TTS

| Measurement | Result |
| --- | --- |
| Silero VAD v5, 24 s clip, 1 thread | RTF 0.0027, load 0.04 s, peak RSS 53 MB. 3 segments (3.16–5.86, 6.55–10.34, 14.46–20.74 s) on a clip of two utterances: the first split at a 0.7 s pause (minimum silence 0.25 s). Identical on Linux arm64 and x86_64. |
| KWS zipformer 3.3M | RTF 0.0135, load 0.25 s, peak RSS 108 MB. Official clips with `test_keywords.txt`: `LIGHT UP`, `LOVELY CHILD`, `FOREVER`, all correct. |
| KWS false accepts | **0** over all FLEURS dev clips, en and ru (2.09 h), with the 9 keywords of the bundled `keywords.txt`, threshold 0.25, score 1.0 |
| Kokoro v1.0 fp32, RTF | 4 threads 0.23, 2 threads 0.41, 1 thread 0.78; load 0.61–0.65 s; peak RSS 0.69–0.77 GB |
| Kokoro time to first audio, 4 threads | 0.42–0.43 s when the text has more than one sentence; the whole utterance (1.1 s for 4.5 s of audio) when it is one sentence; 0.26 s for "Hello." |
| Kokoro words dropped | see R6 |

## Findings

- **Kokoro in 2.x needs its own phonemizer.** 2.0.0 removed espeak-ng and piper-phonemize
  ([CHANGELOG @ v2.0.3](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.3/CHANGELOG.md), #4055); `lang`
  and `data_dir` are ignored with a warning. Text goes through a lexicon, and a word missing from it is
  skipped with a log line, not spoken. The alternative is pre-phonemized input
  (`SherpaOnnxGenerationConfig.phoneme_codepoints`, documented in `c-api.h` @ v2.0.3).
- **2.0.0 makes failures visible to the caller**: `SherpaOnnxCreate*` catch C++ exceptions and return
  NULL (#4044). Errors during decoding are still not covered: with a wrong sample rate onnxruntime
  logged a broadcast error and went on, and a harness bug that fed garbage made one process grow to
  18–33 GB RSS.
- The C API is complete enough for all four engines from Kotlin/Native; the harness is 305 lines.
- Parakeet TDT v3 is offline (non-streaming): live use is VAD segments decoded one by one.

## Risks for T7.4 and the TTS tasks

- **R1 (process isolation; decided):** sherpa-onnx runs in a separate helper process,
  `aulo-speech-sherpa`, talking to `aulod` over a pipe (creator's decision for T7.4). A crash or a
  runaway allocation then costs one engine, which is skipped (the fail-open rule); the helper should run
  under a memory limit.
- **R2 (shipping on Linux and Windows):** the Linux and Windows builds are not self-contained. Packaging
  must put the two `.so` files (33–40 MB) or the three DLLs next to the helper. Linux needs glibc ≥ 2.17.
  The Windows binary has not been run anywhere yet; T7.4's CI should run `version` on a Windows runner.
- **R3 (build):** archives come from GitHub at build time and are checked against pinned SHA-256s. Any
  edit to the recipe changes its revision; the lock of every module that requires it must then be
  recreated (delete it first: `conan lock create` reads an existing lock next to `conanfile.txt`).
- **R4 (macOS speed):** RTF 0.07 on macOS against 0.03 for Linux arm64 on the same machine. Worth one
  check in T7.4 (the macOS shared-library archive, thread placement on performance cores); 0.07 is
  already well inside real time.
- **R5 (Moonshine):** about 3% of English clips give an empty transcript. Parakeet stays the default;
  Moonshine is a fallback for low memory (0.57 GB against 1.5–1.8 GB) only after that is understood.
- **R6 (Kokoro words dropped):** share of FLEURS en dev words (8,154) missing from the lexicon, all of
  which Kokoro would skip:

  | Lexicon | Entries | Missing words |
  | --- | --- | --- |
  | `lexicon-us-en.txt` in `kokoro-multi-lang-v1_0` | 178,401 | 6.21% (plurals and verb forms: `countries`, `students`, `opened`) |
  | `lexicon-en-us.txt`, tts-models release, 2026-10-13, `sha256:8b67ae54…` | 167,982 | 2.72% (also `or`, `her`, `seem`) |
  | both together | 297,426 | 1.52% (names, possessives) |

  A Parakeet round trip confirms the gaps: "I opened the settings" came back as "I the settings".
  Options for the TTS tasks: espeak-ng as a separate phonemizer process feeding `phoneme_codepoints`
  (GPL code in its own process is allowed by AGENTS.md), another grapheme-to-phoneme model, or a merged
  lexicon with a fallback. This touches spec §17 D9, which accepted espeak-ng because sherpa-onnx linked
  it; 2.x links nothing GPL. The decision is the creator's.
- **R7 (languages, unchanged):** Kokoro has no Russian voice; Russian TTS needs another engine.
  Parakeet is CC-BY-4.0: ship the attribution with the model.
- **R8 (maintenance):** four releases between 2026-10-23 (2.0.0) and 2026-11-22 (2.0.3), GitHub API. The recipe pins
  2.0.3; bumps need the creator's approval.

## WER threshold for T7.4

Proposed: on a fixed set of FLEURS dev clips per language, Parakeet TDT v3 int8 through
`aulo-speech-sherpa` must reach **WER ≤ 10% for en and for ru** with the normalizer above (measured 7.84%
and 7.75%, so about two points of margin for a smaller set). Moonshine, if wired, ≤ 15% for en counting
empty outputs as errors. The threshold awaits the creator's confirmation.

## Reproduce

1. Add the files in [`sherpa-onnx/`](sherpa-onnx/) as a module (`Main.kt` under
   `src/nativeMain/kotlin/spike/`, `sherpa.def` under `src/nativeInterop/cinterop/`), include it in
   `settings.gradle.kts`, and create its lock with the command in AGENTS.md.
2. `./gradlew :<module>:linkSpikeReleaseExecutable<Target>`.
3. `sherpa-spike.kexe stt parakeet <model dir> 4 @<list of wavs>`; then
   `python3 -I wer.py <fleurs dev.tsv> <output tsv>`. Also `vad`, `kws` and `tts`; run the binary with
   no arguments for usage.
