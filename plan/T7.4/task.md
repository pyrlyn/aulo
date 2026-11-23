---
status: in progress
priority: P0
complexity: 4
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T7.4. STT: sherpa-onnx (Parakeet TDT v3, Moonshine)

Stage: S7 · Area: speech · Depends on: T7.3, T0.5 · Blocks: 1 task(s)

Default local STT. Parakeet TDT v3 (25 languages incl. ru, uk) on VAD segments; Moonshine streaming for low-latency English partials.

Done when:

- WER test on fixture WAVs under the threshold from the spike

## Execution plan

Branch `t7.4-sherpa-stt` merges `t7.3-models`, `t7.2-registry` and `t0.5-sherpa-spike`; the PR targets
`t7.3-models`. sherpa-onnx runs in a helper process (creator's choice): a crash or a hang of the
native code is an engine failure, the registry falls back, and `aulod` keeps running.

1. `:aulo-speech-sherpa` (SURFACE, the one sherpa-onnx owner) links the helper executable
   `aulo-speech-sherpa`.
   - `conanfile.txt` and `conan.lock` pin the local recipe `sherpa-onnx/2.0.3`; `sherpa.def` comes from
     the spike.
   - `OfflineStt` wraps `SherpaOnnxOfflineRecognizer` for Parakeet TDT v3 (`nemo_transducer`) and
     Moonshine; a NULL from `SherpaOnnxCreate*` (a C++ exception, sherpa-onnx #4044) is an error.
   - The helper's main loop serves the pipe protocol on stdin and stdout and logs to stderr.
   - Linux `.so` files and Windows DLLs are copied next to the executable.
2. `:aulo-speech-helper` (LIBRARY): the aulod side.
   - The pipe protocol: typed frames with a length prefix; every frame and every text is capped, and
     a frame over its cap ends the link.
   - `HelperStt : SttEngine` over a duplex byte link. `push` and `poll` never block: a writer and a
     reader run on their own threads behind bounded queues. Each utterance has a sequence number,
     so a late result of a cancelled utterance is dropped.
   - The helper exiting, a bad frame or no reply in time is `SpeechException.Failed`; a helper that
     cannot load its model is `Unavailable`. Both fall back.
   - Engines: `sherpa-parakeet` (25 languages, final on `finish`, the utterance being the VAD
     segment) and `sherpa-moonshine` (en; partials by decoding the growing buffer at a fixed
     interval).
3. Spawning the helper with stdin and stdout piped. kotlin-kit's `process` has no long-lived piped
   child yet; how it gets one is the creator's call.
4. `aulo-models`: add Moonshine base en (quantized), pinned to its Hugging Face commit.
5. Tests:
   - protocol: round trip, every cap, truncated and oversized frames;
   - `HelperStt` against an in-memory helper: order rules, cancel, stale results, helper crash and
     garbage leading to fallback, bounded queues counting drops;
   - WER, when `AULO_SHERPA_MODELS` and `AULO_FLEURS` are set: FLEURS dev `en_us` (394 clips) and
     `ru_ru` (356 clips), the normalizer of `docs/spikes/sherpa-onnx/wer.py`; Parakeet ≤ 10% for en
     and ru, Moonshine ≤ 15% for en (spike proposal, empty outputs count as errors).
6. ktlint, `checkModuleGraph`; tests on macOS and on Linux x64/arm64 (Colima); mingw link. Then
   `AGENTS.md`, `research.md`, `toolchain.md` (sherpa-onnx row) and the PR.
