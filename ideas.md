# aulo — ideas

Not approved. Nothing here moves to `roadmap.md` or `plan/` without the creator's approval.

- **grpc-web for browsers.** gRPC-web in aulod so a web client can connect without a proxy.
- **OpenAI-compatible HTTP facade.** `/v1/chat/completions` on top of the agent, for tools that only speak that API.
- **Voice on mobile.** `aulo-chat` (D12) is text chat on iOS and Android; voice over `VoiceService` from a phone is not planned yet.
- **Attachments in `UserTurn`.** A repeated attachment field (kind, name, bytes or a reference) in `aulo.v1`, so `aulo-chat` can send images and voice notes instead of only staging them (T12.22; open in spec §17).
- **Real attachment pickers in aulo-chat.** A file and image picker per platform (for example FileKit) and a voice recorder behind `AttachmentPicker`; today it returns sample files (T12.22).
- **aulo-chat screens beyond the chat.** The conversation list, the model sheet and the attachments sheet are designed (`aulo-chat/design/`) but not built (T12.22).
- **Platform widgets in aulo-chat.** The UI is Material 3 with each platform's colours and radii; the workspace's glass, fluent and adwaita Compose packages target JVM and wasmJs only, so they cannot serve Android and iOS yet (T12.22).
- **aulo-chat reconnects by itself.** Backoff reconnect instead of only the Retry button; a light theme; Russian UI (T12.22).
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
- **Per-engine sherpa workers.** The Kokoro and Parakeet engines from one factory share a worker thread, so an engine that stops polling without cancelling stalls the others on a full audio queue (T7.9).
- **One word error rate.** `aulo-speech-bench` has its own word-level `WordErrors` (case, punctuation, `ё` and `’` normalised) and runa-media has `word_error_rate`; once a shared module exists both should use it (T7.16).
- **Evict idle chat sessions.** `SessionApi` keeps every chat attached since start in memory with its full history (T4.8); dropping a chat with no client and no running turn, and rebuilding it from the store on the next Attach, would bound that.
- **Store-backed standing grants in the session gate.** T4.8 wires `PolicyGate` with no standing grants; once T5.3 fixes how a stored grant maps to a `(tool, subject)` pair, read them from `aulo-store`.
- **One Conan plugin for Kotlin/Native builds.** pyrlyn-kit's `gradle/conan.gradle.kts` and aulo's `aulo.conan` plugin (T3.3) both install C libraries per target with Conan. aulo's also builds the Linux targets from source with Kotlin/Native's sysroots, waits for Kotlin's toolchain download and keeps its own Conan home; a shared Gradle plugin in `packages/` would give pyrlyn-kit the same and remove the second copy.
- **nghttp2 1.71.** It hardens integer overflow checks but fixes no advisory; Conan Center's newest recipe is 1.68.1 (T3.3, `research.md` 4.10). Move when the recipe catches up.
- **Durable append in pyrlyn-kit.** `fs.appendSink` cannot fsync, so `aulo-models` renames a verified download without one (T7.3, `research.md` 4.14); a `sync()` on the kit's sink, or a durable append, would close that gap.
