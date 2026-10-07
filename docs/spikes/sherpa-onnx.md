# Spike T0.5: sherpa-onnx on macOS arm64

Verdict: **GO** for sherpa-onnx as the default local speech backend, with one open licence question (espeak-ng is GPL-3.0 and is statically linked, see Risks R1).

Checked 2026-10-07. Scratch project (not in the repo): `/private/tmp/claude-501/-Users-listepo-GitHub-listepo/565734e4-fe95-469c-8f24-2a200fc4f56b/scratchpad/t0.5`.

## Setup

| Item | Value |
| --- | --- |
| Machine | Apple M3 Max (`sysctl -n machdep.cpu.brand_string`), 64 GiB RAM (`hw.memsize` = 68719476736), macOS 27.0.1 |
| Toolchain | rustc 1.99.0 (b940084d7 2026-09-28) via `mise exec rust@1.99.0 --` |
| Crate | `sherpa-onnx` =1.13.8, Apache-2.0, published 2026-09-11, default feature `static`; depends on `sherpa-onnx-sys` =1.13.8 ([crates.io API](https://crates.io/api/v1/crates/sherpa-onnx), checked 2026-10-07). 1.13.8 is the latest on crates.io. |
| Native lib | `sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2` (21 MB), downloaded by `sherpa-onnx-sys/build.rs` from the official GitHub release at build time ([build.rs in crate 1.13.8](https://crates.io/api/v1/crates/sherpa-onnx-sys/1.13.8/download), [release v1.13.8, 2026-09-10](https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.13.8)). `SHERPA_ONNX_ARCHIVE_DIR` / `SHERPA_ONNX_LIB_DIR` allow an offline build (same build.rs). |
| Harness | One small binary (`spike`) written against the crate API, following the official examples `nemo_parakeet.rs`, `keyword_spotter.rs`, `kokoro_tts_zh_en.rs`, `silero_vad_remove_silence.rs` ([rust-api-examples @ v1.13.8](https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.8/rust-api-examples/examples)). |
| Test audio | English and Russian WAVs from macOS `say` (Samantha, Milena) converted with `afconvert` to 16 kHz mono 16-bit; plus Kokoro output resampled to 16 kHz. All timings are wall clock on an otherwise idle machine, CPU provider, 4 threads unless stated. |

## Models (all from the official k2-fsa release pages, github.com/k2-fsa/sherpa-onnx/releases/download/...)

| Role | Asset (release tag) | Archive | On disk | SHA-256 of archive | Licence |
| --- | --- | --- | --- | --- | --- |
| VAD | `silero_vad_v5.onnx` (asr-models) | 2.3 MB | 2.2 MiB | `6b99cbfd...c274396` (file itself) | MIT ([silero-vad LICENSE](https://github.com/snakers4/silero-vad/blob/master/LICENSE), latest tag v6.2.3 2026-09-23; sherpa ships v4 and v5 only) |
| STT | `sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2` (asr-models) | 487 MB | 640 MiB | `5793d0fd...eb16bf` | CC-BY-4.0 (attribution required) ([nvidia/parakeet-tdt-0.6b-v3 card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), last modified 2026-08-05; 25 languages incl. en, ru) |
| KWS | `sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2` (kws-models); English only | 17.6 MB | 19 MiB | `f170013b...9b6561a` | Apache-2.0 (stated in the README inside the archive) |
| TTS | `kokoro-multi-lang-v1_0.tar.bz2` (tts-models), fp32 | 350 MB | 394 MiB (model.onnx 325 MB) | `c5f7e2d2...ea3298` | Apache-2.0 ([hexgrad/Kokoro-82M card](https://huggingface.co/hexgrad/Kokoro-82M), 2025-04-10) |
| TTS | `kokoro-int8-multi-lang-v1_0.tar.bz2`, int8 | 132 MB | 186 MiB (model.int8.onnx 114 MB) | `4c3052ab...a247e4e` | Apache-2.0 |
| TTS | `kokoro-int8-multi-lang-v1_1.tar.bz2` (v1.1 is the zh+en model, [README in archive](https://huggingface.co/hexgrad/Kokoro-82M-v1.1-zh)) | 147 MB | 206 MiB | `a1e94694...cafc6` | Apache-2.0 (LICENSE file in archive) |

Asset sizes are from `gh release view <tag> -R k2-fsa/sherpa-onnx` (checked 2026-10-07). Parakeet TDT v3 and Kokoro v1.0 were both available as official sherpa-onnx assets, so nothing was substituted.

## Results

| Measurement | Result |
| --- | --- |
| Cold release build (`rm -rf target`, registry cached, includes the 21 MB native-lib download) | 16.0 s wall (20.1 s on the very first build with a cold registry). No C++ compilation: the static libs are prebuilt. |
| Release binary (all of VAD+STT+KWS+TTS linked, default profile) | 27.8 MB; 19.6 MB after `strip`. Dynamic deps: only libc++, libSystem, Foundation, CoreFoundation. |
| Native lib unpacked in `target/` | 112 MB (not shipped; linked statically) |
| Silero VAD, RTF | 0.003 (18 s clip in 56 ms, 1 thread). On a 10 s clip of speech/1 s silence/speech it returned 3 segments at the right places. Peak RSS 45-55 MB. |
| Parakeet TDT v3 int8, STT RTF, 4 threads | 0.07 on 18 s en and 17 s ru clips (about 1.3 s each), 0.07-0.08 on 3-4 s clips (0.2-0.3 s). Model load 0.8 s. |
| Same, 2 threads / 1 thread | RTF 0.13 / 0.25 |
| STT peak RSS (max resident set size) | 1.21 GB (3.8 s clip) to 1.42 GB (18 s clip), 4 threads |
| STT output | en: correct on the 18 s clip, with punctuation and casing. ru (macOS Milena input): 3 small errors in 17 s (e.g. "леса прыгай"); the Kokoro-generated ru, es, fr and en clips round-tripped with no errors. This is TTS audio, not human speech, so it is not a WER measurement. |
| Kokoro fp32, 4 threads, RTF | 0.23 (en), 0.20-0.23 (es, fr), 0.23 (ru), 0.27 (1.6 s clip). Load 0.6 s. |
| Kokoro fp32, 2 threads / 1 thread, RTF | 0.41 / 0.78 |
| Kokoro int8, 4 threads, RTF | 0.65 (about 3x slower than fp32 on this CPU) |
| Kokoro time to first audio (first `generate_with_config` callback, fp32, 4 threads) | 0.24-0.30 s when the text starts with a short sentence ("Sure."), 0.43-0.46 s for a first sentence of about 1.5 s of audio, 0.55-0.64 s for a 2-3 s single sentence. The callback fires per sentence, not per audio chunk: a long first sentence delays first audio. |
| Kokoro peak RSS | fp32 0.85-0.88 GB, int8 0.64 GB |
| KWS (zipformer 3.3M, int8 joiner) | RTF 0.015, load 0.25 s, peak RSS about 57 MB |
| KWS fires on a test phrase | Yes. Built-in keywords: "Alexa, play some music." fired `ALEXA` (macOS voice and Kokoro); Kokoro "Hey Siri, what is the weather like today?" fired `HEY SIRI`; macOS-voice "Hey Siri" did **not** fire at thresholds 0.25/1.0, 0.1/2.0 and 0.05/3.0. No false fires on 3 negative clips (2 s English sentence, Russian sentence, Spanish sentence). Custom keyword `HEY_AULO` with hand-written BPE tokens fired on the Kokoro and macOS clips only for the variant `▁HE Y ▁A LO` with boost 3.0 and threshold 0.01. Official test clip + official `test_keywords.txt`: detected `LOVELY CHILD` and `FOREVER`. |
| CoreML provider (`provider = "coreml"`) | No measurable change in STT or TTS timing; not verified whether the CoreML EP is compiled into the prebuilt lib. |

Not measured: all four engines resident in one process (the sum of the peaks above is roughly 2.4 GB, an upper bound), streaming transducer STT, quality judged by ear.

## Findings

- Kokoro does **not** support Russian. Its language list is American/British English, Japanese, Mandarin, Spanish, French, Hindi, Italian, Brazilian Portuguese ([VOICES.md](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md), checked 2026-10-07). Setting `lang = "ru"` with an English voice produced audio that Parakeet transcribed back correctly, but naturalness is unjudged and unsupported by the model authors. Russian TTS needs another engine (a Piper/VITS Russian voice, or Silero).
- `kokoro-multi-lang-v1_0` is the real multi-language bundle; `v1_1` is zh+en only. Speaker ids used: 3 = `af_heart`, 28 = `ef_dora` (es), 30 = `ff_siwis` (fr) ([generate_voices_bin.py @ v1.13.8](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.8/scripts/kokoro/v1.0/generate_voices_bin.py)).
- Parakeet TDT v3 is an offline (non-streaming) model; live use means VAD segmentation plus decoding each segment (the crate ships `parakeet_tdt_simulate_streaming_microphone.rs`). End of speech to text for a 3 s utterance is about 0.2 s here.
- A bad model/config pair aborts the whole process (C++ exception, see R5), not a Rust `Err`.

## Risks for T5.x / T6.x

- **R1 (licence, needs creator decision):** the prebuilt static lib contains `libespeak-ng.a` and `libpiper_phonemize.a`, and the final binary exports `espeak_*` symbols (`nm -gU`). espeak-ng is GPL-3.0 ([repo licence, checked 2026-10-07](https://github.com/espeak-ng/espeak-ng)); sherpa-onnx's own build pulls a fork of it ([cmake/espeak-ng-for-piper.cmake @ v1.13.8](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.8/cmake/espeak-ng-for-piper.cmake)). AGENTS.md says GPL code only runs as a separate plugin process, so linking this crate into `aulod` conflicts with that rule. Options: run TTS (or all of sherpa-onnx) as a separate process; build sherpa-onnx from source without espeak/TTS and use VAD+KWS+STT in-process; or accept GPL. I have not decided this.
- **R2 (build):** `build.rs` downloads a 21 MB archive from GitHub at build time. CI and reproducible/offline builds must set `SHERPA_ONNX_ARCHIVE_DIR` or vendor the archive, and pin its checksum (`build.rs` in 1.13.8 contains no checksum or digest verification).
- **R3 (memory/latency):** Parakeet needs about 1.4 GB RSS and 640 MiB on disk; Kokoro fp32 about 0.9 GB. Ship fp32 Kokoro (int8 is slower on Apple silicon). Keep the first sentence of a reply short to get time-to-first-audio under 0.3 s; use at least 4 threads (1 thread gives Kokoro RTF 0.78 and STT 0.25).
- **R4 (languages):** Russian STT works (Parakeet v3), Russian TTS does not exist in Kokoro; the aulo default voice stack for ru needs a second TTS engine behind the same trait.
- **R5 (KWS and robustness):** the `-mobile` gigaspeech KWS bundle crashes in onnxruntime (`/downsample/Reshape_1`, input `{17,1,128}` vs requested `{8,2,1,128}`) and the Rust process aborts with "Rust cannot catch foreign exceptions"; the non-mobile bundle works. Wrap sherpa calls in a child process or a catch boundary, since a bad model aborts the daemon (rule: a broken engine is skipped, never fatal). KWS is English only; wake-word accuracy depends on per-keyword boost/threshold tuning and on BPE tokenization of the custom phrase (tokenize with the official tooling, not by hand), and was tested only on synthetic voices, so false-accept/false-reject rates are unknown.
- **R6 (attribution):** Parakeet is CC-BY-4.0; ship the attribution with the model download or docs.
- **R7 (maintenance):** the crate is released about every 2-4 weeks (1.13.4 to 1.13.8 between 2026-07-08 and 2026-09-11), and its version is pinned to `sherpa-onnx-sys`; pin `=1.13.x` and bump with creator approval.
