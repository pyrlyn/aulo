# T0.5. Spike: sherpa-onnx on macOS arm64

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Build the official sherpa-onnx Rust crate (1.13.x) and run its examples for Parakeet TDT v3 STT, Silero VAD, keyword spotting and Kokoro TTS. Measure build time, binary size, model sizes, real-time factor and time to first audio. Write docs/spikes/sherpa-onnx.md with numbers and sources.


Done when:

- docs/spikes/sherpa-onnx.md with measured numbers
- go/no-go for sherpa-onnx as the default local speech backend
