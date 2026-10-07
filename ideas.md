# aulo — ideas

Not approved. Nothing here moves to `roadmap.md` or `plan.md` without the creator's approval.

- **grpc-web for browsers.** tonic-web in aulod so a web client can connect without a proxy.
- **OpenAI-compatible HTTP facade.** `/v1/chat/completions` on top of the agent, for tools that only speak that API.
- **Mobile clients.** iOS and Android apps on the gRPC API (voice over `VoiceService`).
- **Owner voice only.** Speaker identification (sherpa-onnx speaker embeddings) so aulo answers only its owner's voice, or treats other voices as untrusted.
- **Harness emulation per model family.** Prompt and tool formats tuned per local model family, as Open Interpreter does.
- **Remote nodes.** A desktop install acts as a node that a remote aulod controls, under the node's own local policy (OpenClaw nodes).
- **Fully local full-duplex front.** Kyutai Moshi or Qwen3-Omni as a local realtime voice front instead of a cloud one.
- **Satellite microphones.** Small devices (ESP32 or a phone) that stream audio to aulod in other rooms.
- **Voice cloning for TTS.** Where the engine and licence allow it, with explicit consent.
- **"What am I looking at".** Answer questions about the current screen on a hotkey (screenshot + vision model), read-only.
- **Chat channels.** Slack, Discord and Signal bridges like the Telegram plugin (T14.6).
- **Plugin marketplace site.** A browsable index on top of the signed plugin index (T11.11).
- **Moonshine English partials.** T7.4 shipped Parakeet only: sherpa-onnx 1.13.8 has Moonshine as an offline model, so partials would mean re-decoding the buffer, and the catalog has no verified Moonshine bundle.
- **sherpa-onnx in a child process.** Removes spike risk R5: an onnxruntime C++ exception or a model damaged in place still aborts the daemon (T7.4).
- **Checksum the sherpa-onnx prebuilt archive.** The sherpa-onnx-sys build script downloads it without a digest (spike risk R2); vendor it or verify it in our own build step.
- **Shared reply scaffolding in aulo-speech-system.** The macOS, espeak and Windows engines repeat the same push/poll state, caps, overflow and generation fence (noted in T7.11); one module would remove the copies.
