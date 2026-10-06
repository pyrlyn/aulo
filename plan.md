# aulo — plan

Repository: https://github.com/pyrlyn/aulo

aulo is a local-first voice agent that controls the computer (shell, apps, browser, desktop), speaks MCP both ways, takes plugins, and runs on local or remote models. A daemon (`aulod`) serves a gRPC API to the desktop chat app, the CLI and remote clients. See `spec.md` for the specification and `research.md` for the sources.

Task ids are `T<stage>.<n>`. Each card lists its stage, area and dependencies. Stages are in `spec.md` §16.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T0.5 | in progress | P0 | 2 | 0% | Claude Code / claude-sonnet-5-5 |
| T0.6 | in progress | P1 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T1.1 | todo | P0 | 4 | 0% | |
| T1.2 | todo | P0 | 3 | 0% | |
| T1.3 | todo | P0 | 3 | 0% | |
| T1.4 | todo | P0 | 3 | 0% | |
| T1.5 | todo | P0 | 2 | 0% | |
| T1.6 | todo | P0 | 3 | 0% | |
| T1.7 | todo | P1 | 3 | 0% | |
| T1.8 | todo | P0 | 3 | 0% | |
| T1.9 | todo | P0 | 3 | 0% | |
| T1.10 | todo | P0 | 2 | 0% | |
| T1.11 | todo | P1 | 2 | 0% | |
| T1.12 | todo | P0 | 4 | 0% | |
| T1.13 | todo | P0 | 3 | 0% | |
| T1.14 | todo | P1 | 2 | 0% | |
| T1.15 | todo | P0 | 4 | 0% | |
| T1.16 | todo | P1 | 3 | 0% | |
| T1.17 | todo | P1 | 3 | 0% | |
| T1.18 | todo | P2 | 2 | 0% | |
| T1.19 | todo | P1 | 3 | 0% | |
| T1.20 | todo | P2 | 3 | 0% | |
| T2.2 | todo | P0 | 2 | 0% | |
| T2.3 | todo | P0 | 2 | 0% | |
| T2.4 | in progress | P0 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T2.5 | in progress | P0 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T2.6 | todo | P1 | 1 | 0% | |
| T2.7 | todo | P0 | 2 | 0% | |
| T2.9 | in progress | P0 | 2 | 0% | Claude Code / claude-sonnet-5-5 |
| T2.10 | todo | P2 | 2 | 0% | |
| T3.1 | todo | P0 | 3 | 0% | |
| T3.2 | todo | P0 | 2 | 0% | |
| T3.3 | todo | P0 | 3 | 0% | |
| T3.4 | todo | P0 | 3 | 0% | |
| T3.5 | todo | P0 | 2 | 0% | |
| T3.6 | todo | P0 | 2 | 0% | |
| T3.7 | todo | P0 | 3 | 0% | |
| T3.8 | todo | P0 | 3 | 0% | |
| T3.9 | todo | P2 | 3 | 0% | |
| T4.1 | todo | P0 | 3 | 0% | |
| T4.2 | todo | P0 | 2 | 0% | |
| T4.3 | todo | P1 | 2 | 0% | |
| T4.4 | todo | P0 | 3 | 0% | |
| T4.5 | todo | P1 | 3 | 0% | |
| T4.6 | todo | P0 | 4 | 0% | |
| T4.7 | todo | P0 | 2 | 0% | |
| T4.8 | todo | P0 | 3 | 0% | |
| T4.9 | todo | P0 | 2 | 0% | |
| T4.10 | todo | P1 | 3 | 0% | |
| T4.11 | todo | P1 | 2 | 0% | |
| T4.12 | todo | P2 | 2 | 0% | |
| T5.1 | todo | P0 | 3 | 0% | |
| T5.2 | todo | P0 | 2 | 0% | |
| T5.3 | todo | P0 | 3 | 0% | |
| T5.4 | todo | P0 | 3 | 0% | |
| T5.5 | todo | P0 | 2 | 0% | |
| T5.6 | todo | P0 | 2 | 0% | |
| T5.7 | todo | P1 | 2 | 0% | |
| T5.8 | todo | P1 | 3 | 0% | |
| T5.9 | todo | P1 | 3 | 0% | |
| T5.10 | todo | P1 | 4 | 0% | |
| T5.11 | todo | P1 | 2 | 0% | |
| T5.12 | todo | P1 | 3 | 0% | |
| T5.13 | todo | P1 | 2 | 0% | |
| T6.1 | todo | P0 | 3 | 0% | |
| T6.2 | todo | P0 | 2 | 0% | |
| T6.3 | todo | P1 | 2 | 0% | |
| T6.4 | todo | P1 | 4 | 0% | |
| T6.5 | todo | P1 | 3 | 0% | |
| T6.6 | todo | P1 | 2 | 0% | |
| T6.7 | todo | P2 | 3 | 0% | |
| T7.1 | todo | P0 | 3 | 0% | |
| T7.2 | todo | P0 | 3 | 0% | |
| T7.3 | todo | P0 | 3 | 0% | |
| T7.4 | todo | P0 | 3 | 0% | |
| T7.5 | todo | P1 | 2 | 0% | |
| T7.6 | todo | P1 | 3 | 0% | |
| T7.7 | todo | P2 | 3 | 0% | |
| T7.8 | todo | P2 | 3 | 0% | |
| T7.9 | todo | P0 | 3 | 0% | |
| T7.10 | todo | P1 | 3 | 0% | |
| T7.11 | todo | P2 | 3 | 0% | |
| T7.12 | todo | P2 | 2 | 0% | |
| T7.13 | todo | P1 | 2 | 0% | |
| T7.14 | todo | P2 | 2 | 0% | |
| T7.15 | todo | P0 | 3 | 0% | |
| T7.16 | todo | P2 | 2 | 0% | |
| T8.1 | todo | P0 | 4 | 0% | |
| T8.2 | todo | P0 | 2 | 0% | |
| T8.3 | todo | P0 | 2 | 0% | |
| T8.4 | todo | P0 | 4 | 0% | |
| T8.5 | todo | P0 | 3 | 0% | |
| T8.6 | todo | P1 | 3 | 0% | |
| T8.7 | todo | P1 | 3 | 0% | |
| T8.8 | todo | P1 | 2 | 0% | |
| T8.9 | todo | P1 | 3 | 0% | |
| T8.10 | todo | P1 | 4 | 0% | |
| T8.11 | todo | P1 | 4 | 0% | |
| T8.12 | todo | P2 | 3 | 0% | |
| T8.13 | todo | P2 | 3 | 0% | |
| T8.14 | todo | P2 | 3 | 0% | |
| T8.15 | todo | P2 | 3 | 0% | |
| T8.16 | todo | P0 | 3 | 0% | |
| T8.17 | todo | P1 | 2 | 0% | |
| T9.1 | todo | P0 | 3 | 0% | |
| T9.2 | todo | P1 | 3 | 0% | |
| T9.3 | todo | P0 | 3 | 0% | |
| T9.4 | todo | P2 | 3 | 0% | |
| T9.5 | todo | P2 | 3 | 0% | |
| T9.6 | todo | P0 | 4 | 0% | |
| T9.7 | todo | P0 | 3 | 0% | |
| T9.8 | todo | P1 | 3 | 0% | |
| T9.9 | todo | P2 | 1 | 0% | |
| T9.10 | todo | P2 | 3 | 0% | |
| T9.11 | todo | P1 | 4 | 0% | |
| T9.12 | todo | P1 | 3 | 0% | |
| T9.13 | todo | P2 | 4 | 0% | |
| T9.14 | todo | P2 | 4 | 0% | |
| T9.15 | todo | P1 | 2 | 0% | |
| T9.16 | todo | P1 | 3 | 0% | |
| T9.17 | todo | P1 | 2 | 0% | |
| T9.18 | todo | P1 | 2 | 0% | |
| T9.19 | todo | P2 | 1 | 0% | |
| T9.20 | todo | P3 | 5 | 0% | |
| T10.1 | todo | P0 | 3 | 0% | |
| T10.2 | todo | P1 | 2 | 0% | |
| T10.3 | todo | P2 | 3 | 0% | |
| T10.4 | todo | P1 | 2 | 0% | |
| T10.5 | todo | P0 | 3 | 0% | |
| T10.6 | todo | P1 | 3 | 0% | |
| T10.7 | todo | P2 | 2 | 0% | |
| T10.8 | todo | P1 | 1 | 0% | |
| T11.1 | todo | P0 | 2 | 0% | |
| T11.2 | todo | P0 | 3 | 0% | |
| T11.3 | todo | P0 | 2 | 0% | |
| T11.4 | todo | P1 | 3 | 0% | |
| T11.5 | todo | P1 | 3 | 0% | |
| T11.6 | todo | P2 | 2 | 0% | |
| T11.7 | todo | P2 | 4 | 0% | |
| T11.8 | todo | P1 | 2 | 0% | |
| T11.9 | todo | P1 | 3 | 0% | |
| T11.10 | todo | P1 | 2 | 0% | |
| T11.11 | todo | P3 | 3 | 0% | |
| T12.1 | todo | P0 | 4 | 0% | |
| T12.2 | todo | P0 | 3 | 0% | |
| T12.3 | todo | P0 | 3 | 0% | |
| T12.4 | todo | P0 | 3 | 0% | |
| T12.5 | todo | P0 | 4 | 0% | |
| T12.6 | todo | P0 | 3 | 0% | |
| T12.7 | todo | P0 | 3 | 0% | |
| T12.8 | todo | P1 | 3 | 0% | |
| T12.9 | todo | P1 | 3 | 0% | |
| T12.10 | todo | P1 | 3 | 0% | |
| T12.11 | todo | P1 | 2 | 0% | |
| T12.12 | todo | P1 | 3 | 0% | |
| T12.13 | todo | P1 | 3 | 0% | |
| T12.14 | todo | P2 | 3 | 0% | |
| T12.15 | todo | P1 | 3 | 0% | |
| T12.16 | todo | P1 | 4 | 0% | |
| T12.17 | todo | P1 | 3 | 0% | |
| T12.18 | todo | P1 | 4 | 0% | |
| T12.19 | todo | P2 | 3 | 0% | |
| T12.20 | todo | P2 | 2 | 0% | |
| T12.21 | todo | P2 | 2 | 0% | |
| T13.1 | todo | P0 | 2 | 0% | |
| T13.2 | todo | P1 | 3 | 0% | |
| T13.3 | todo | P1 | 3 | 0% | |
| T13.4 | todo | P2 | 2 | 0% | |
| T13.5 | todo | P1 | 3 | 0% | |
| T13.6 | todo | P2 | 2 | 0% | |
| T14.1 | todo | P1 | 3 | 0% | |
| T14.2 | todo | P1 | 2 | 0% | |
| T14.3 | todo | P2 | 3 | 0% | |
| T14.4 | todo | P2 | 3 | 0% | |
| T14.5 | todo | P3 | 5 | 0% | |
| T14.6 | todo | P3 | 3 | 0% | |
| T15.1 | todo | P1 | 2 | 0% | |
| T15.2 | todo | P1 | 3 | 0% | |
| T15.3 | todo | P2 | 3 | 0% | |
| T15.4 | todo | P1 | 2 | 0% | |
| T15.5 | todo | P1 | 2 | 0% | |

## S0. Decisions and spikes

### T0.5. Spike: sherpa-onnx on macOS arm64

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Build the official sherpa-onnx Rust crate (1.13.x) and run its examples for Parakeet TDT v3 STT, Silero VAD, keyword spotting and Kokoro TTS. Measure build time, binary size, model sizes, real-time factor and time to first audio. Write docs/spikes/sherpa-onnx.md with numbers and sources.

Execution plan: 1. Scratch Cargo project outside the workspace (scratchpad) depending on the official sherpa-onnx crate. 2. Run Silero VAD, Parakeet TDT v3 offline STT on en and ru WAVs, keyword spotting, Kokoro TTS, following the crate examples; download models from the official release URLs. 3. Measure cold build time, release binary size, model sizes, RTF, time to first audio, peak RSS. 4. Write docs/spikes/sherpa-onnx.md with numbers, versions, sources and a go/no-go.

Done when:

- docs/spikes/sherpa-onnx.md with measured numbers
- go/no-go for sherpa-onnx as the default local speech backend

### T0.6. Spike: echo cancellation from Rust

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Compare macOS Voice Processing I/O (AVAudioEngine setVoiceProcessingEnabled via objc2-avf-audio) with webrtc-audio-processing 2.x (AEC3). Play TTS through speakers while capturing; measure residual echo and false barge-ins. Write docs/spikes/aec.md.

Execution plan: 1. Scratch project: macOS Voice Processing I/O via objc2-avf-audio (AVAudioEngine, setVoiceProcessingEnabled on input and output nodes). 2. Second scratch: webrtc-audio-processing 2.x AEC3 with the playback stream as far-end reference. 3. Play a speech WAV through the speakers while recording; measure residual echo energy (dB) and VAD false triggers for both, plus build complexity. 4. Write docs/spikes/aec.md with a recommendation per OS.

Done when:

- docs/spikes/aec.md with a recommendation per OS

## S1. Shared crates in packages/

### T1.1. Extract llm-wire: provider contract and neutral types

Stage: S1 · Area: shared · Depends on: T0.1, T0.7 · Blocks: 3 task(s)

Move the provider trait and the neutral types (Request, ProviderEvent, Caps, ToolSpec, Usage, Risk) out of cox-protocol into packages/crates/llm-wire. No behaviour change. Keep the Replay/Scripted fakes behind a test-util feature.

Done when:

- llm-wire builds and its tests pass in packages/crates
- cox-protocol re-exports the moved types (no API break for cox)

### T1.2. Extract llm-http: credentials, retries, SSE framing

Stage: S1 · Area: shared · Depends on: T1.1 · Blocks: 2 task(s)

Move resolve_key (env var, then keyring), retry policy and SSE framing from cox-provider-http into packages/crates/llm-http.

Done when:

- llm-http tests cover env-first, keyring fallback and retry backoff

### T1.3. Extract llm-openai: Chat and Responses wires

Stage: S1 · Area: shared · Depends on: T1.2 · Blocks: 2 task(s)

Move the OpenAI Chat and Responses wires from cox-provider-openai into packages/crates/llm-openai. They also serve every OpenAI-compatible endpoint (Ollama, LM Studio, vLLM, OpenRouter, DeepSeek, Gemini compat, xAI, runa).

Done when:

- streaming text and tool-call tests pass against recorded fixtures

### T1.4. Extract llm-anthropic: Messages wire

Stage: S1 · Area: shared · Depends on: T1.2 · Blocks: 1 task(s)

Move the Anthropic Messages wire (typify-generated types, cache breakpoints, thinking, effort) into packages/crates/llm-anthropic.

Done when:

- recorded-fixture tests pass

### T1.5. Extract llm-catalog: model catalog and prices

Stage: S1 · Area: shared · Depends on: T1.1 · Blocks: 2 task(s)

Move cox-models (built-in rows, config overlay, prices.toml, overlay of served models) into packages/crates/llm-catalog.

Done when:

- Catalog::load, get and effort_for tests pass

### T1.6. Switch cox to the extracted llm crates

Stage: S1 · Area: shared · Depends on: T1.3, T1.4, T1.5 · Blocks: 2 task(s)

cox depends on llm-wire, llm-http, llm-openai, llm-anthropic and llm-catalog; the old code is deleted. Update cox tests/deps.rs, cox toolchain.md and rust.md.

Done when:

- cox CI green with no duplicated provider code left

### T1.7. llm-router: one provider per tier

Stage: S1 · Area: shared · Depends on: T1.6 · Blocks: 1 task(s)

cox-session builds one provider per session from tiers.code.provider. Add packages/crates/llm-router that builds a provider per tier and routes jobs to tiers, so the voice front, the main agent and the sentinel can use different providers. cox adopts it.

Done when:

- a test routes two tiers to two different providers

### T1.8. Extract perm-rules: permission rule engine

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Move cox-permission (rule grammar such as Bash(git commit:*), Edit(src/**), mcp__srv__*, decision order, risk fallback) into packages/crates/perm-rules with neutral subject types so aulo can add App(...) and Site(...) subjects. cox adopts it.

Done when:

- rule-grammar and decision-order tests pass in both repos

### T1.9. Extract proc-sandbox: Seatbelt, bwrap, Landlock+seccomp

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Move cox-sandbox (SandboxMode, command wrapping, argv for PTYs, path::confine) into packages/crates/proc-sandbox. cox adopts it.

Done when:

- sandbox tests pass on macOS and Linux CI runners

### T1.10. Extract shell-classify: bash risk classifier

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Move the tree-sitter bash classifier from cox-tools/src/bash/classify.rs into packages/crates/shell-classify (ReadOnly, Write, Exec, Destructive).

Done when:

- classifier fixture tests pass; cox adopts it

### T1.11. Extract text-sanitize: terminal and bidi guard

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Merge cox-sanitize, rtok src/sanitize.rs and ketch changelog::sanitize (a known duplicate in rust.md) into packages/crates/text-sanitize. All three projects adopt it.

Done when:

- one crate, three adopters, rust.md duplicate note removed

### T1.12. Extract mcp-host: MCP client with discovery and OAuth

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Move cox-mcp client parts (connect_all that turns a failing server into a notice, .mcp.json discovery with ${VAR} expansion, OAuth with injectable store, elicitation mapping, deferred tools) into packages/crates/mcp-host on rmcp 3.x. cox adopts it.

Done when:

- fail-open and discovery tests pass; cox adopts it

### T1.13. Extract speech-capture: microphone capture and resampling

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 2 task(s)

Merge cox-voice capture (cpal default device, downmix, rubato to 16 kHz) with runa-media decode/resample/energy VAD into packages/crates/speech-capture. cox and runa adopt it; whisper transcription stays behind a feature.

Done when:

- cox and runa build against it; duplicated rubato/whisper code removed

### T1.14. Extract config-schema: schema-checked config helpers

Stage: S1 · Area: shared · Depends on: none · Blocks: 0 task(s)

rust.md lists "config with schema and one owner" as a known duplicate (cox-config, rtok config/layers.rs, ketch toml_file.rs). Extract the stale-schema test helper, figment layering with per-key provenance and toml_edit editing into packages/crates/config-schema.

Done when:

- ketch or rtok adopts it; helper tests pass

### T1.15. Extract agent-loop: neutral turn state machine

Stage: S1 · Area: shared · Depends on: T1.1, T0.7 · Blocks: 2 task(s)

Per the T0.7 decision, extract the turn state machine (Submission/Event, parallel and exclusive tool dispatch, approvals, interrupt via CancellationToken, max steps) from cox-core into packages/crates/agent-loop. Coding-specific parts stay in cox.

Done when:

- agent-loop tests with a scripted provider cover tool calls, approval and interrupt

### T1.16. Switch cox to agent-loop

Stage: S1 · Area: shared · Depends on: T1.15 · Blocks: 0 task(s)

cox-core uses agent-loop; duplicated loop code is deleted.

Done when:

- cox CI green

### T1.17. Extract agent-ext: skills and shell hooks

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 2 task(s)

Move SKILL.md discovery (index line in prompt, deferred skill tool) and Claude-Code-style shell hooks (JSON on stdin, exit 2 blocks, HookChain) from cox-ext into packages/crates/agent-ext. cox adopts it.

Done when:

- skills and hooks tests pass in both repos

### T1.18. Extract agent-host-config: register MCP servers in agent hosts

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Move register_mcp/unregister_mcp, backup and atomic JSON edits from rtok-agent-sdk into packages/crates/agent-host-config (foreign-config rule: touch only our entry, keep bytes, write atomically, back up with file-backup).

Done when:

- rtok adopts it; round-trip test keeps foreign keys byte-for-byte

### T1.19. runa: stream tokens as they are generated

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

runa serve builds the whole reply before replaying it as SSE (serve.rs generate_events), so time to first token equals generation time. Change it to stream GenEvents as they arrive. Work happens in the runa repository; aulo only tracks it.

Done when:

- runa streams the first token before generation ends (test with a timer)

### T1.20. runa: library target for in-process local inference

Stage: S1 · Area: shared · Depends on: T1.19 · Blocks: 0 task(s)

runa is bin-only (serve, pool and mcp are pub(crate)). Expose a library crate (for example runa-serve) so aulod can embed local inference without a second process.

Done when:

- aulo can call runa's pool from a test

## S2. Workspace foundations

### T2.2. Dependency-graph test

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

crates/aulo/tests/deps.rs reads cargo metadata and enforces crate roles: contracts have no I/O deps, only surfaces depend on clap/anyhow, one owner crate per heavy dependency (diesel, tonic, sherpa-onnx, wasmtime).

Done when:

- the test fails when a contract crate pulls tokio::fs or reqwest

### T2.3. CI workflows

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

ci.yml calls pyrlyn/ci ci-rust.yml (Linux x86_64 and aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), commit-subject lint, pipeline.yml (CodeQL, Semgrep). Linux job installs libasound2-dev.

Done when:

- CI green on the skeleton

### T2.4. aulo-types: domain contract

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 4 task(s)

Ids (ULID): BotId, ChatId, TurnId, CallId, ClientId. aulo events beyond agent-loop: VoiceState, Transcript (partial/final), SpeechStarted/Ended, TtsChunk, ApprovalRequired, TakeoverRequested, Notice. No I/O dependencies.

Execution plan: 1. crates/aulo-types: ULID-based BotId, ChatId, TurnId, CallId, ClientId (serde, Display, FromStr). 2. Event types beyond agent-loop: VoiceState, Transcript (partial/final), SpeechStarted/Ended, TtsChunk metadata, ApprovalRequired, TakeoverRequested, Notice. 3. No I/O dependencies (serde, ulid, thiserror only). 4. Tests: serde round-trip and id parsing; clippy, fmt, nextest.

Done when:

- types serialize round-trip; deps test passes

### T2.5. aulo-config: typed config with schema and layers

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 5 task(s)

Config types with schemars; committed schemas/aulo.schema.json with a stale test; figment layers default < ~/.aulo/config.toml < project .aulo.toml < AULO_* env < flags, with per-key provenance. The only crate that touches toml/toml_edit/figment. Use config-schema when T1.14 lands.

Execution plan: 1. crates/aulo-config: Config types (deny_unknown_fields, schemars) for daemon, providers, voice, mcp, plugins, policy sections at the level the spec defines. 2. figment layers: defaults < ~/.aulo/config.toml (AULO_HOME override) < project .aulo.toml < AULO_* env < overrides from the caller, with per-key provenance. 3. schemas/aulo.schema.json committed plus a test that fails when it is stale (AULO_UPDATE_SCHEMA=1 regenerates). 4. Errors name the file. 5. Tests for layering, provenance, unknown keys, schema.

Done when:

- schema test fails when types change without regenerating
- unknown keys are rejected with the file name in the error

### T2.6. Embedded default config

Stage: S2 · Area: infra · Depends on: T2.5 · Blocks: 0 task(s)

config/default.toml with every assignment commented out; a test checks it parses to Config::default(). `aulo config show --origin` prints values with provenance.

Done when:

- test passes; trycmd fixture for config show

### T2.7. aulo-telemetry: logs and traces

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 1 task(s)

tracing-subscriber JSON logs in ~/.aulo/logs with rotation, level from config/env, optional OTLP behind a feature. Secrets are never logged (redaction layer).

Done when:

- a test proves a key-like value is redacted

### T2.9. aulo-store: grants and audit tables

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 2 task(s)

Tables for permission grants (subject, scope, decision, expiry) and the audit log (seq, prev_hash, hash, kind, payload).

Execution plan: 1. New migration directory in crates/aulo-store/migrations for grants (id, subject, scope, decision, expires_at, created_at; index on subject+scope) and audit (seq INTEGER PRIMARY KEY, prev_hash, hash, kind, payload, created_at). 2. schema.rs and models.rs rows. 3. Store API: put/list/revoke grants, list active grants for a subject (expiry filtered in the query), append audit row and list audit rows after seq. Hash computation is left to aulo-policy (T5.9); the store only persists. 4. Tests: migrations up/down, grant CRUD and expiry, audit append order.

Done when:

- migrations and CRUD tests pass

### T2.10. aulo-store: full-text search over messages

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 1 task(s)

FTS5 virtual table over message text. Raw SQL stays in the storage crate with a comment that Diesel cannot model FTS5.

Done when:

- search test finds a message by a word

## S3. gRPC API and daemon

### T3.1. aulo-proto: v1 service definitions

Stage: S3 · Area: api · Depends on: T2.4 · Blocks: 2 task(s)

proto/aulo/v1/*.proto: ChatService (Create/List/Get/Rename/Delete chats, ListMessages), SessionService (bidi Converse: client submissions, server events), VoiceService (bidi Talk: PCM frames and control in; transcripts, state, TTS audio out), ApprovalService, ConfigService (providers, models, voices, devices), McpService, PluginService, AuditService. buf.yaml with lint rules.

Done when:

- buf lint passes; spec.md API section matches the protos

### T3.2. aulo-proto: code generation and breaking-change check

Stage: S3 · Area: api · Depends on: T3.1 · Blocks: 3 task(s)

tonic-build/prost code generation; CI runs buf breaking against main.

Done when:

- generated crate builds; CI fails on a breaking field change

### T3.3. aulo-server: tonic server scaffold

Stage: S3 · Area: api · Depends on: T3.2, T2.5, T2.7 · Blocks: 4 task(s)

tonic server with tonic-health and reflection, listeners for a Unix socket / Windows named pipe (local) and TCP (remote), graceful shutdown, request size limits.

Done when:

- integration test connects over UDS and calls health

### T3.4. aulo-server: authentication

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 3 task(s)

Local socket: trust by file owner and 0600 permissions. TCP: bearer token required, compared in constant time, interceptor rejects missing tokens. The first token is generated on first run and stored in the OS keychain.

Done when:

- tests: TCP without token is rejected; UDS from owner is accepted

### T3.5. aulo-server: ChatService

Stage: S3 · Area: api · Depends on: T3.3, T2.8 · Blocks: 5 task(s)

Implement chat CRUD and message listing over aulo-store with pagination.

Done when:

- grpc integration tests for every RPC

### T3.6. aulod binary

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 2 task(s)

clap surface: aulod run, --config, --listen; single-instance lock per home; pidfile; SIGINT/SIGTERM shutdown.

Done when:

- second instance exits with a clear message; trycmd fixtures

### T3.7. aulo CLI: text chat client

Stage: S3 · Area: api · Depends on: T3.5, T3.6 · Blocks: 2 task(s)

aulo chat (rustyline REPL over Converse), aulo chats ls/rm, aulo status. Renders streamed text and tool calls; Ctrl-C sends Interrupt.

Done when:

- trycmd fixtures against a test daemon with a scripted provider

### T3.8. aulo-server: TLS for TCP listeners

Stage: S3 · Area: api · Depends on: T3.4 · Blocks: 1 task(s)

rustls; user-provided certificate or a generated self-signed one with fingerprint pinning in clients (`aulo connect --pin`).

Done when:

- test: client with wrong pin is refused

### T3.9. Run aulod as a user service

Stage: S3 · Area: api · Depends on: T3.6 · Blocks: 0 task(s)

aulod service install/uninstall: launchd agent on macOS, systemd user unit on Linux, Task Scheduler entry on Windows.

Done when:

- install and uninstall are idempotent (tested with a fake service manager)

## S4. Models, providers and agent loop

### T4.1. aulo-providers: build providers from config

Stage: S4 · Area: providers · Depends on: T1.6, T2.5 · Blocks: 4 task(s)

Assembly crate that builds llm-wire providers from [providers.*]: OpenAI (chat/responses), Anthropic, and OpenAI-compatible sections without per-vendor code (Ollama, LM Studio, vLLM, OpenRouter, DeepSeek, Gemini compat, xAI, runa, custom). Fail open: a broken provider becomes a warning.

Done when:

- tests build every preset from fixtures; a bad section only warns

### T4.2. Credentials in the OS keychain

Stage: S4 · Area: providers · Depends on: T4.1 · Blocks: 2 task(s)

aulo auth set/rm/ls <provider>: keys go to the keychain (keyring), env vars win, inline keys in config are rejected (runa pattern). gRPC ConfigService can set a key but never return it.

Done when:

- test: config with an inline key fails with the file name

### T4.3. Local server presets and auto-detection

Stage: S4 · Area: providers · Depends on: T4.1 · Blocks: 0 task(s)

Presets for runa (127.0.0.1:8080/v1), Ollama (11434), LM Studio (1234, native model list). Probe /v1/models at start and list what is served.

Done when:

- wiremock tests detect each server kind

### T4.4. Model catalog and per-chat model switching

Stage: S4 · Area: providers · Depends on: T4.1, T1.5, T3.5 · Blocks: 3 task(s)

ConfigService lists providers and models (catalog plus served overlay). A chat stores its provider/model; SwitchModel works mid-chat.

Done when:

- test switches model between turns and the next request uses it

### T4.5. Tiers and job routing

Stage: S4 · Area: providers · Depends on: T4.4, T1.7 · Blocks: 2 task(s)

Tiers: voice (fast, short answers), main (tools), sentinel (reviewer), cheap (titles, summaries). Each tier may use a different provider (llm-router).

Done when:

- test routes main and cheap to different providers

### T4.6. aulo-agent: session on agent-loop

Stage: S4 · Area: agent · Depends on: T1.15, T4.1, T2.4 · Blocks: 6 task(s)

Session per chat on agent-loop: submissions in, events out, cancellation, max steps, parallel/exclusive tools. Maps agent-loop events to aulo-types events.

Done when:

- scripted-provider tests: text turn, tool turn, interrupt

### T4.7. Tool registry and deferred tools

Stage: S4 · Area: agent · Depends on: T4.6 · Blocks: 3 task(s)

Registry with naming (builtin, mcp__server__tool, plugin__id__tool), risk levels, deferred tools hidden until found by tool_search.

Done when:

- test: deferred tool appears only after tool_search

### T4.8. Converse RPC and chat persistence

Stage: S4 · Area: agent · Depends on: T4.6, T3.5 · Blocks: 9 task(s)

SessionService.Converse drives aulo-agent; every turn, tool call and usage row is persisted; a chat resumes after a daemon restart.

Done when:

- test restarts the daemon and continues the chat with full history

### T4.9. Scripted-provider test harness

Stage: S4 · Area: agent · Depends on: T4.6 · Blocks: 1 task(s)

aulo-testkit: scripted provider (from llm-wire test-util), test daemon on a temp home, helpers to drive Converse.

Done when:

- used by T4.6 and T4.8 tests

### T4.10. Context compaction and chat titles

Stage: S4 · Area: agent · Depends on: T4.5, T4.8 · Blocks: 0 task(s)

Summarize old turns with the cheap tier near the context window; generate chat titles after the first turn.

Done when:

- test: long chat stays under the window; title is set

### T4.11. Persona and spoken-answer prompting

Stage: S4 · Area: agent · Depends on: T4.6 · Blocks: 0 task(s)

Bot name (also the wake word), language, and a voice-mode prompt: short spoken answers, no markdown, "look at the screen" for code and long output.

Done when:

- prompt snapshot tests (insta)

### T4.12. Usage and cost ledger

Stage: S4 · Area: agent · Depends on: T4.8 · Blocks: 0 task(s)

Every provider call writes a usage row (Priced wrapper); aulo usage shows cost per chat and provider.

Done when:

- test sums usage across two providers

## S5. Safety core

### T5.1. aulo-policy: rule engine for every tool call

Stage: S5 · Area: safety · Depends on: T1.8, T4.7 · Blocks: 15 task(s)

Every tool call passes perm-rules before running. Subjects: Shell(cmd), App(bundle), Site(host), File(path), Mcp(server, tool), Input(kind). Decisions: Allow, Deny, Ask. Policy is deterministic code outside the model and fails closed.

Done when:

- table tests for rule grammar per subject

### T5.2. Shell risk classification

Stage: S5 · Area: safety · Depends on: T5.1, T1.10 · Blocks: 1 task(s)

shell-classify sets the risk of shell commands (ReadOnly, Write, Exec, Destructive); risk feeds the default decision when no rule matches.

Done when:

- fixtures: rm -rf is Destructive and asks; ls is ReadOnly

### T5.3. Approval flow and grants

Stage: S5 · Area: safety · Depends on: T5.1, T4.8, T2.9 · Blocks: 11 task(s)

ApprovalRequired event; any attached client answers once / always for this chat / always / never; grants are persisted with scope and expiry; no answer before the timeout means deny.

Done when:

- tests: timeout denies; "always" grant is reused next turn

### T5.4. Sandbox profiles for commands

Stage: S5 · Area: safety · Depends on: T5.1, T1.9 · Blocks: 1 task(s)

proc-sandbox modes read-only, workspace-write (default, workspace ~/aulo/workspace) and full access. A sandbox denial can be retried unsandboxed only after approval.

Done when:

- tests on macOS and Linux: write outside the workspace is denied

### T5.5. Kill switch

Stage: S5 · Area: safety · Depends on: T4.6 · Blocks: 0 task(s)

Interrupt from any client, a global hotkey and the voice phrase "stop" cancel every running tool and turn within 200 ms.

Done when:

- test measures cancellation latency under a running shell tool

### T5.6. Per-app and per-site grants with a default blocklist

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 2 task(s)

Grants for App and Site subjects (allow once / always / never). Built-in category blocklist that always asks or denies: banking, payments, crypto, password managers, system settings.

Done when:

- test: a banking site always asks even with a broad allow rule

### T5.7. Unattended mode is read-only

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 1 task(s)

When no client is attached or the run is a routine, write/exec/input tools are denied or queued for approval; reads still work.

Done when:

- test: routine run cannot write a file

### T5.8. Sentinel reviewer for consequential actions

Stage: S5 · Area: safety · Depends on: T5.3, T4.5 · Blocks: 0 task(s)

A separate model call on the sentinel tier reviews consequential actions (send, buy, delete, publish, credential use) with the action and a context summary and returns allow or escalate. It can only make a decision stricter, never allow what policy denies. Rationale goes to the audit log.

Done when:

- tests: sentinel escalate forces Ask; sentinel allow cannot override Deny

### T5.9. Tamper-evident audit log

Stage: S5 · Area: safety · Depends on: T2.9, T5.3 · Blocks: 2 task(s)

Append-only, hash-chained rows for every tool call, approval, egress and plugin load. aulo audit verify and aulo audit export.

Done when:

- test: editing a row breaks verification

### T5.10. Secrets vault the model cannot read

Stage: S5 · Area: safety · Depends on: T4.2, T5.1 · Blocks: 0 task(s)

Credentials live in the keychain behind handles. Tools take a handle (type_secret(handle), http auth by handle) and inject the value; the model, transcripts and logs only see the handle.

Done when:

- test: transcript and log contain the handle, never the value

### T5.11. Human takeover

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 0 task(s)

request_takeover(reason) pauses the turn, notifies every client (desktop notification, spoken prompt) and resumes on "done". Used for passwords, 2FA, CAPTCHA, payments.

Done when:

- test: turn resumes only after the takeover-done submission

### T5.12. Untrusted-content guards

Stage: S5 · Area: safety · Depends on: T5.1, T1.11 · Blocks: 0 task(s)

Web pages, tool output, screenshot text and plugin output are wrapped as untrusted data, sanitized with text-sanitize, and instruction-like content in them forces Ask for the next consequential action.

Done when:

- fixture: an injected "ignore previous instructions" page forces Ask

### T5.13. Threat model

Stage: S5 · Area: safety · Depends on: T5.1 · Blocks: 0 task(s)

docs/threat-model.md: trust boundaries (model, tools, MCP servers, plugins, web content, remote clients), STRIDE per boundary, and the guard that covers each threat.

Done when:

- every guard in the doc points to a test or task

## S6. Audio I/O

### T6.1. aulo-audio: capture

Stage: S6 · Area: audio · Depends on: T1.13, T2.4 · Blocks: 3 task(s)

speech-capture behind an aulo trait: device by name or default, 16 kHz mono f32 frames of 20 ms into a lock-free ring buffer (ringbuf), overflow counted, never blocks the audio thread.

Done when:

- test with a fake device; manual check on macOS

### T6.2. aulo-audio: playback

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 4 task(s)

Streaming playback (rodio on cpal) with volume, immediate stop for barge-in and a played-position counter for echo reference.

Done when:

- stop() silences within 50 ms (measured with a fake sink)

### T6.3. Device list, selection and hot-plug

Stage: S6 · Area: audio · Depends on: T6.2, T3.3 · Blocks: 1 task(s)

ConfigService lists input/output devices; selection persists; unplugging falls back to the default device with a notice.

Done when:

- fake-device test for fallback

### T6.4. Echo cancellation on macOS (Voice Processing I/O)

Stage: S6 · Area: audio · Depends on: T6.2, T0.6 · Blocks: 0 task(s)

Per the T0.6 spike: capture and playback through AVAudioEngine in voice-processing mode on macOS, so barge-in works with open speakers.

Done when:

- manual test from the spike passes; config switch to disable

### T6.5. Echo cancellation elsewhere (webrtc-audio-processing)

Stage: S6 · Area: audio · Depends on: T6.2, T0.6 · Blocks: 0 task(s)

AEC3, noise suppression and AGC for Windows and Linux, with the playback stream as the far-end reference. Feature-gated.

Done when:

- fixture test: far-end tone is attenuated by the target dB

### T6.6. Microphone permission handling

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Detect a denied microphone (macOS TCC, Windows privacy settings) and report a clear notice with the fix instead of silence.

Done when:

- notice text covered by a test with a fake permission probe

### T6.7. Opus for remote audio streams

Stage: S6 · Area: audio · Depends on: T6.1 · Blocks: 0 task(s)

Optional Opus encoding of VoiceService frames for low-bandwidth remote clients (PCM16 16 kHz stays the default).

Done when:

- round-trip test within the SNR target

## S7. Speech engines

### T7.1. aulo-speech: engine contracts

Stage: S7 · Area: speech · Depends on: T2.4 · Blocks: 11 task(s)

Traits: SttEngine (push frames, partial and final transcripts), TtsEngine (stream text in, audio chunks out, voices, rate), Vad, KeywordSpotter, TurnDetector. Capabilities: streaming, languages, offline, needs network.

Done when:

- contract tests with fake engines

### T7.2. Engine registry and runtime switching

Stage: S7 · Area: speech · Depends on: T7.1, T2.5 · Blocks: 5 task(s)

Engines register by id (built-in and plugin-provided). [voice.stt] and [voice.tts] select engine, model and voice; a chat or bot can override; switching through gRPC, CLI (aulo voice use) or voice command takes effect on the next utterance without restart. A failing engine is skipped for the next one in the fallback list (fail open).

Done when:

- test switches TTS mid-chat; a failing engine falls back with a notice

### T7.3. Model manager

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 4 task(s)

Download speech models from Hugging Face with pinned SHA-256 (cox whisper-models.json pattern) into ~/.aulo/models, progress events, aulo models pull/ls/rm. Refuse a hash mismatch.

Done when:

- test: corrupted download is rejected

### T7.4. STT: sherpa-onnx (Parakeet TDT v3, Moonshine)

Stage: S7 · Area: speech · Depends on: T7.3, T0.5 · Blocks: 1 task(s)

Default local STT. Parakeet TDT v3 (25 languages incl. ru, uk) on VAD segments; Moonshine streaming for low-latency English partials.

Done when:

- WER test on fixture WAVs under the threshold from the spike

### T7.5. STT: whisper.cpp via whisper-rs

Stage: S7 · Area: speech · Depends on: T7.3, T1.13 · Blocks: 0 task(s)

Whisper backend reusing the speech-capture whisper feature (cox-voice Transcriber).

Done when:

- fixture transcription test

### T7.6. STT: OpenAI-compatible transcription endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

/v1/audio/transcriptions client: works with OpenAI (GPT Transcribe), runa (local server) and other compatible servers. Streaming variant where the server supports it.

Done when:

- wiremock tests for both shapes

### T7.7. STT: Deepgram streaming

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

Official deepgram crate, streaming over WebSocket with interim results.

Done when:

- recorded-session test

### T7.8. STT: Apple SpeechAnalyzer (macOS 26+)

Stage: S7 · Area: speech · Depends on: T7.1, T0.3 · Blocks: 0 task(s)

On-device system recognizer via objc2-speech; only offered when available.

Done when:

- capability probe test; manual check

### T7.9. TTS: sherpa-onnx (Kokoro, Matcha, VITS)

Stage: S7 · Area: speech · Depends on: T7.3, T0.5 · Blocks: 1 task(s)

Default local TTS. Kokoro (Apache-2.0 weights) with voice list; sentence-chunked streaming synthesis so the first sentence plays while the rest is generated.

Done when:

- time-to-first-audio test within the budget

### T7.10. TTS: macOS system voices

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

AVSpeechSynthesizer via objc2-avf-audio writing buffers into aulo-audio (not straight to the speaker, so barge-in and AEC still work). Lists installed voices.

Done when:

- manual check; voice-list test behind a macOS cfg

### T7.11. TTS: Windows system voices

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

WinRT SpeechSynthesizer via the windows crate, streamed into aulo-audio.

Done when:

- test on the Windows runner

### T7.12. TTS: Linux speech-dispatcher / espeak-ng

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

Speech-dispatcher (SSIP) or espeak-ng subprocess as the zero-download fallback voice.

Done when:

- test on the Linux runner with espeak-ng installed

### T7.13. TTS: OpenAI speech endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

OpenAI-compatible /v1/audio/speech with streaming PCM; voice list from config.

Done when:

- wiremock test

### T7.14. TTS: ElevenLabs streaming

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

WebSocket streaming TTS with voice id and model from config; key from the keychain.

Done when:

- recorded-session test

### T7.15. Speakable-text normalizer

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 1 task(s)

Turns model text into speech input: strips markdown, replaces code blocks and long output with "it is on the screen", shortens URLs, expands units and numbers per language, splits sentences for streaming TTS, picks the voice by detected language.

Done when:

- insta snapshot tests for en and ru

### T7.16. Speech benchmarks

Stage: S7 · Area: speech · Depends on: T7.4, T7.9 · Blocks: 0 task(s)

WER (runa-media word_error_rate), real-time factor and time-to-first-audio per engine on fixtures; divan benches and a report command.

Done when:

- aulo bench speech prints a table

## S8. Voice conversation

### T8.1. aulo-voice: conversation pipeline

Stage: S8 · Area: voice · Depends on: T6.2, T7.2, T7.15, T4.8 · Blocks: 10 task(s)

frames, VAD, STT, end of turn, Submission, streamed reply, normalizer, TTS, playback as tokio tasks with bounded channels. States: Idle, Listening, Thinking, Speaking. One audio owner at a time.

Done when:

- end-to-end test with fake engines drives a full turn

### T8.2. VAD backends

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 3 task(s)

earshot (pure Rust) by default, Silero through sherpa-onnx as an alternative; thresholds and hangover in config.

Done when:

- fixture test: speech/no-speech boundaries within 60 ms

### T8.3. Push-to-talk

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 0 task(s)

Global hotkey (handy-keys), gRPC StartListening/StopListening, and the desktop mic button all drive the same state change.

Done when:

- test: start/stop over gRPC produces one utterance

### T8.4. VoiceService over gRPC

Stage: S8 · Area: voice · Depends on: T8.1, T3.4 · Blocks: 4 task(s)

Bidi Talk stream: clients send PCM (or Opus) frames and control; the server runs the pipeline and streams state, transcripts and TTS audio back. Lets the desktop app and remote clients use the server-side pipeline; local mode uses the daemon's own devices.

Done when:

- integration test streams a WAV in and receives transcript and audio out

### T8.5. Barge-in

Stage: S8 · Area: voice · Depends on: T8.2 · Blocks: 0 task(s)

User speech while Speaking stops playback within 150 ms and cancels pending TTS; the agent turn is interrupted or kept running (config). Works with headphones from the start; open speakers need echo cancellation (T6.4, T6.5).

Done when:

- timing test with fake engines; manual test with speakers

### T8.6. Wake word

Stage: S8 · Area: voice · Depends on: T8.2, T7.3 · Blocks: 0 task(s)

sherpa-onnx keyword spotting with open-vocabulary keywords; the wake word defaults to the bot's name; up to 32 triggers, each routed to a bot or chat. Runs fully on device.

Done when:

- fixture test: wake word detected, near-miss rejected

### T8.7. End-of-turn detection

Stage: S8 · Area: voice · Depends on: T8.2 · Blocks: 0 task(s)

Silence timeout (700 ms default) plus optional Smart Turn v3 semantic end-of-turn model (BSD-2, ONNX) to avoid cutting the user off mid-thought.

Done when:

- fixture test: a mid-sentence pause does not end the turn

### T8.8. Progress fillers and earcons

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 0 task(s)

Short spoken fillers while tools run ("checking the browser"), built from tool descriptions and rate-limited; earcons for start/stop listening and for approval requests.

Done when:

- test: at most one filler per N seconds

### T8.9. Voice approvals with limits

Stage: S8 · Area: voice · Depends on: T8.1, T5.3 · Blocks: 1 task(s)

Approvals can be answered by voice ("yes" / "no") for low and medium risk. Destructive, payment and credential actions need a click or key on a screen: voice never widens access.

Done when:

- test: destructive action ignores a spoken yes

### T8.10. aulo-realtime: OpenAI realtime voice client

Stage: S8 · Area: voice · Depends on: T1.3 · Blocks: 1 task(s)

WebSocket client (tokio-tungstenite) for OpenAI realtime voice models (GPT-Live / Realtime), mapping their events to aulo-types; ephemeral client tokens for remote clients.

Done when:

- recorded-session test

### T8.11. Two-brain mode: realtime front consults the agent

Stage: S8 · Area: voice · Depends on: T8.10, T8.1 · Blocks: 2 task(s)

A realtime speech-to-speech model talks with the user; its only tool is consult_agent, which submits to the main agent where policy applies. The front never calls computer tools itself (OpenClaw agent-consult pattern).

Done when:

- test: a realtime tool call other than consult_agent is refused

### T8.12. Realtime: Gemini Live adapter

Stage: S8 · Area: voice · Depends on: T8.11 · Blocks: 0 task(s)

Gemini Live with asynchronous non-blocking function calls mapped to consult_agent.

Done when:

- recorded-session test

### T8.13. Realtime: xAI Voice Agent adapter

Stage: S8 · Area: voice · Depends on: T8.11 · Blocks: 0 task(s)

wss://api.x.ai/v1/realtime adapter (OpenAI-Realtime-shaped events, server VAD).

Done when:

- recorded-session test

### T8.14. Local voice commands

Stage: S8 · Area: voice · Depends on: T8.1, T7.2 · Blocks: 0 task(s)

A small grammar handled before the model: stop, repeat, louder/quieter, "use voice X", "use local model", "new chat". Unmatched speech goes to the model.

Done when:

- table tests for en and ru phrases

### T8.15. Keep talking while the agent works

Stage: S8 · Area: voice · Depends on: T8.1 · Blocks: 0 task(s)

New utterances during a running turn are queued as steering messages instead of being dropped or forcing an interrupt.

Done when:

- test: steering text reaches the running turn

### T8.16. End-to-end voice test with fixtures

Stage: S8 · Area: voice · Depends on: T8.4, T4.9 · Blocks: 2 task(s)

CI test: WAV fixture in, real local STT, scripted provider, real local TTS, audio out; asserts transcript and non-empty audio. Runs headless.

Done when:

- runs in CI on Linux and macOS

### T8.17. Latency budget and measurement

Stage: S8 · Area: voice · Depends on: T8.16 · Blocks: 0 task(s)

Budgets: wake to listening under 300 ms; end of speech to first audio under 1.2 s with local engines and a remote LLM; barge-in under 150 ms. Spans in telemetry and a report command.

Done when:

- aulo bench voice prints the budget table

## S9. Computer control

### T9.1. Shell tool

Stage: S9 · Area: control · Depends on: T5.4, T5.2 · Blocks: 2 task(s)

Run a command in the sandbox with timeout, output cap, streamed output events, cwd and env allowlist. Long output is archived losslessly and the model sees a truncated view with an id (cox expand pattern).

Done when:

- tests: timeout kills the process tree; truncated output is recoverable

### T9.2. Interactive PTY sessions

Stage: S9 · Area: control · Depends on: T9.1 · Blocks: 0 task(s)

portable-pty sessions for interactive programs: open, send, read, close; buffers capped.

Done when:

- test drives python -i or a shell through a PTY

### T9.3. App control on macOS

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

Launch, quit, focus apps and list running apps and windows (NSWorkspace via objc2). Subject App(bundle id) for policy.

Done when:

- manual check; unit tests for the bundle-id resolver

### T9.4. App control on Windows

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

Launch (ShellExecute), focus and list windows and processes; App(exe path) subject.

Done when:

- test on the Windows runner

### T9.5. App control on Linux

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

Launch via .desktop entries (gio), list and focus windows (X11/Wayland where possible).

Done when:

- test on the Linux runner

### T9.6. Browser: managed Chromium over CDP

Stage: S9 · Area: control · Depends on: T5.6 · Blocks: 4 task(s)

chromiumoxide drives a dedicated aulo browser profile (never the user's own profile by default). Tools: open URL, read page as an accessibility snapshot with element refs, page text, screenshot. Site(host) subject for policy.

Done when:

- headless Chromium test reads a fixture page snapshot

### T9.7. Browser: act on elements

Stage: S9 · Area: control · Depends on: T9.6 · Blocks: 2 task(s)

Click, type, select, scroll, press keys by element ref from the snapshot; wait for navigation; batched actions stop on the first failure.

Done when:

- fixture form is filled and submitted in headless test

### T9.8. Browser: tabs, downloads, uploads

Stage: S9 · Area: control · Depends on: T9.7 · Blocks: 0 task(s)

List/switch/close tabs; downloads always ask and land in the workspace; file upload only from allowed roots.

Done when:

- tests for download approval and upload confinement

### T9.9. Browser via Playwright MCP (alternative)

Stage: S9 · Area: control · Depends on: T9.6 · Blocks: 0 task(s)

Config switch to use Playwright MCP as the browser backend (spawned through the MCP client) where Node is available.

Done when:

- documented and covered by an MCP fixture test

### T9.10. Attach to the user's own browser

Stage: S9 · Area: control · Depends on: T9.6, T5.6 · Blocks: 0 task(s)

Opt-in CDP attach to a running Chrome the user started with remote debugging, with an explicit consent prompt and per-site grants.

Done when:

- test: attach without consent is refused

### T9.11. Desktop accessibility tree on macOS

Stage: S9 · Area: control · Depends on: T5.1, T0.3 · Blocks: 2 task(s)

Read the AX tree of an app or the focused window (objc2-application-services) as a compact tree with refs, roles, names and values; capped size.

Done when:

- manual check against Finder; unit tests for the tree compactor

### T9.12. Desktop accessibility actions on macOS

Stage: S9 · Area: control · Depends on: T9.11 · Blocks: 2 task(s)

AXPress, set value, focus, open menu items by ref; policy subject App(bundle).

Done when:

- manual check: rename a file in Finder through AX

### T9.13. Desktop accessibility on Windows (UIA)

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

uiautomation tree and actions with the same tool shape as macOS.

Done when:

- test on the Windows runner against Notepad

### T9.14. Desktop accessibility on Linux (AT-SPI)

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

atspi tree and actions with the same tool shape.

Done when:

- test on the Linux runner against a GTK fixture app

### T9.15. Screenshots with normalized coordinates

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 4 task(s)

xcap screenshots per screen or window, downscaled for the model, coordinates normalized to 0-999 so they do not depend on resolution or Retina scale.

Done when:

- coordinate round-trip tests

### T9.16. Synthetic input

Stage: S9 · Area: control · Depends on: T9.15 · Blocks: 1 task(s)

enigo click, type, key, scroll, drag with normalized coordinates; batched actions stop on the first failure; Input(kind) policy subject.

Done when:

- unit tests for coordinate mapping; manual check

### T9.17. macOS permission onboarding

Stage: S9 · Area: control · Depends on: T9.11, T9.15 · Blocks: 0 task(s)

Check Accessibility and Screen Recording grants, explain why, open the right System Settings pane, re-check.

Done when:

- notice texts covered by tests with fake probes

### T9.18. Control ladder and computer tool facade

Stage: S9 · Area: control · Depends on: T9.1, T9.6, T9.12, T9.16 · Blocks: 2 task(s)

System prompt and tool descriptions order the options: MCP or API first, then shell/app, then browser snapshot, then desktop AX, and screenshots with input last.

Done when:

- eval fixture: the agent picks the browser snapshot over pixel clicks

### T9.19. Clipboard tools

Stage: S9 · Area: control · Depends on: T5.1 · Blocks: 0 task(s)

arboard read/write; reading the clipboard asks by default.

Done when:

- policy test for clipboard read

### T9.20. Isolated agent session (separate OS user or VM)

Stage: S9 · Area: control · Depends on: T9.18 · Blocks: 0 task(s)

Research and prototype running GUI work in a separate OS user session (macOS second user with screen sharing, Windows agent workspace) or a VM, like the Grok Bot and Muse agent computers.

Done when:

- docs/spikes/isolation.md with a working prototype or a no-go

## S10. MCP

### T10.1. MCP client

Stage: S10 · Area: mcp · Depends on: T1.12, T4.7, T5.1 · Blocks: 5 task(s)

mcp-host in aulod: [mcp.servers] in config plus .mcp.json import; stdio and streamable HTTP; tools exposed as mcp__server__tool (deferred by default) and checked by policy; a broken server is a notice.

Done when:

- fixture MCP server test: tool call goes through policy

### T10.2. MCP client: OAuth for remote servers

Stage: S10 · Area: mcp · Depends on: T10.1, T4.2 · Blocks: 0 task(s)

OAuth flow for remote MCP servers; tokens in the keychain.

Done when:

- test with an in-memory token store

### T10.3. MCP client: resources, prompts, elicitation

Stage: S10 · Area: mcp · Depends on: T10.1, T8.9 · Blocks: 0 task(s)

Resources and prompts available as tools; elicitation forms asked through the desktop app or by voice.

Done when:

- fixture tests for each

### T10.4. MCP server management

Stage: S10 · Area: mcp · Depends on: T10.1, T3.5 · Blocks: 2 task(s)

McpService: list servers, status, tools, enable/disable, restart, recent stderr.

Done when:

- grpc tests

### T10.5. aulo as an MCP server (stdio)

Stage: S10 · Area: mcp · Depends on: T8.1, T5.3 · Blocks: 2 task(s)

aulo mcp serves aulo's abilities to other agents (Claude Code, Cursor, Codex): speak(text, voice), listen(timeout), ask_user(question) by voice, notify, plus computer tools. Every call goes through aulo policy and approvals, the caller cannot bypass them. Built on rmcp, following rtok's MCP server.

Done when:

- rmcp client test calls speak and gets a policy decision for shell

### T10.6. MCP server over streamable HTTP

Stage: S10 · Area: mcp · Depends on: T10.5, T3.4 · Blocks: 0 task(s)

The same server inside aulod on HTTP with bearer auth, for remote agents.

Done when:

- test: no token is refused

### T10.7. Register aulo in agent hosts

Stage: S10 · Area: mcp · Depends on: T10.5, T1.18 · Blocks: 0 task(s)

aulo mcp install/uninstall --host claude-code|cursor|codex through agent-host-config; touches only aulo's entry, backs up, writes atomically.

Done when:

- round-trip test keeps other entries byte-for-byte

### T10.8. cox tools through cox mcp

Stage: S10 · Area: mcp · Depends on: T10.1 · Blocks: 0 task(s)

Optional preset that mounts `cox mcp` (cox's built-in read/edit/grep/glob tools) as an MCP server for file and code work instead of reimplementing them. Process boundary, no linking.

Done when:

- preset documented; fixture test lists cox tools

## S11. Plugins, skills and hooks

### T11.1. Plugin manifest

Stage: S11 · Area: plugins · Depends on: T2.5 · Blocks: 1 task(s)

aulo-plugin.toml with schemars schema: id, version, kinds (tools, stt, tts, llm-provider, skills, hooks), requested capabilities (net hosts, fs roots, exec, audio), entry (mcp-process, grpc-process, wasm).

Done when:

- schema test; invalid manifest names the file

### T11.2. Plugin host: discovery and grants

Stage: S11 · Area: plugins · Depends on: T11.1, T5.3 · Blocks: 4 task(s)

Discovery in ~/.aulo/plugins and project .aulo/plugins; grants bound to the package digest; a changed digest or new capability needs approval; a broken plugin is skipped with a warning.

Done when:

- tests: tampered plugin needs re-approval; broken plugin only warns

### T11.3. Process plugins over MCP stdio

Stage: S11 · Area: plugins · Depends on: T11.2, T10.1 · Blocks: 1 task(s)

The default plugin ABI: a plugin is an MCP server process; its tools appear as plugin__id__tool under policy.

Done when:

- example plugin test

### T11.4. Streaming plugins over gRPC (speech engines, providers)

Stage: S11 · Area: plugins · Depends on: T11.2, T7.2, T3.2 · Blocks: 2 task(s)

Streaming kinds (STT, TTS, LLM provider) use aulo.plugin.v1 gRPC services over a Unix socket: the host spawns the plugin, health-checks and restarts it, and registers the engine in the registry.

Done when:

- example TTS plugin is switchable like a built-in engine

### T11.5. aulo-plugin-sdk for Rust

Stage: S11 · Area: plugins · Depends on: T11.4 · Blocks: 0 task(s)

SDK crate with manifest types, gRPC service traits and helpers; examples: echo TTS, hello tool.

Done when:

- examples build and pass the host tests

### T11.6. Piper TTS as an external plugin

Stage: S11 · Area: plugins · Depends on: T11.4 · Blocks: 0 task(s)

Piper is GPL-3.0 now; ship it as a separate plugin process so the GPL code stays out of the aulo binary.

Done when:

- plugin speaks a fixture sentence

### T11.7. WASM plugins

Stage: S11 · Area: plugins · Depends on: T11.2 · Blocks: 0 task(s)

In-process sandboxed plugins (extism, as cox-plugin, or wasmtime components) for tools and hooks; deadline and memory cap per call; network only through an allowlisted host function.

Done when:

- tests: deadline kills a looping guest; denied host is blocked

### T11.8. Skills

Stage: S11 · Area: plugins · Depends on: T1.17, T4.7 · Blocks: 1 task(s)

SKILL.md discovery (AgentSkills layout) from agent-ext; only an index line enters the prompt; the skill tool returns the body.

Done when:

- fixture skill is loaded on demand

### T11.9. Hooks

Stage: S11 · Area: plugins · Depends on: T1.17, T8.1 · Blocks: 0 task(s)

Shell hooks with the Claude Code JSON protocol for UserPromptSubmit, PreToolUse, PostToolUse, Heard (final transcript), BeforeSpeak, SessionStart/End. Exit 2 blocks; a failing hook warns.

Done when:

- tests: blocking hook stops a tool; failing hook only warns

### T11.10. Plugin management

Stage: S11 · Area: plugins · Depends on: T11.2, T3.5 · Blocks: 2 task(s)

aulo plugin install <path|git>, ls, enable, disable, rm; PluginService in gRPC.

Done when:

- trycmd fixtures

### T11.11. Signed plugin index

Stage: S11 · Area: plugins · Depends on: T11.10 · Blocks: 0 task(s)

A plugin index with minisign signatures (ketch trust.rs pattern) for discovery and install by name.

Done when:

- bad signature is refused

## S12. Desktop apps (SwiftUI and WinUI)

### T12.1. aulo-app: UI-agnostic client core

Stage: S12 · Area: desktop · Depends on: T0.2, T3.7 · Blocks: 2 task(s)

Rust core the native apps drive, modelled on cox-app: a tonic client of aulod (local socket or remote URL with token and pin, starts the local daemon when absent), a Timeline fold (events in, keyed block patches out), a Controller that coalesces patches to at most one batch per 16 ms frame, chat list and inbox (approvals, takeovers, finished turns), one Intent enum dispatched to submissions. No UI toolkit in its dependency tree.

Done when:

- scenario tests: a replayed event log folds to the same patches as the live run
- deps test: no UI or CLI crate in aulo-app's tree

### T12.2. aulo-ffi: UniFFI exports and bindings

Stage: S12 · Area: desktop · Depends on: T12.1 · Blocks: 2 task(s)

Forward-only UniFFI exports over aulo-app (cox-ffi rule: every exported body is one expression, logic stays in aulo-app, enforced by a syn test). Swift bindings via uniffi-bindgen, C# bindings via uniffi-bindgen-cs. Host callbacks for notifications, URLs and secrets. Records fixtures for the app tests.

Done when:

- forward-only test passes; Swift and C# bindings generate in CI

### T12.3. macOS app scaffold (SwiftUI)

Stage: S12 · Area: desktop · Depends on: T12.2 · Blocks: 7 task(s)

desktop/macos Xcode project on the aulo-ffi package: window plus menu bar extra, connection to the local daemon or a remote server, login item for aulod.

Done when:

- app builds in CI; launches and connects to a test daemon

### T12.4. macOS: chat list

Stage: S12 · Area: desktop · Depends on: T12.3 · Blocks: 1 task(s)

Sidebar with chats grouped by bot: create, rename, delete, search.

Done when:

- UI test on recorded fixtures

### T12.5. macOS: chat view

Stage: S12 · Area: desktop · Depends on: T12.4, T4.8 · Blocks: 3 task(s)

Streamed markdown messages from Timeline patches, collapsible tool-call cards with output, inline screenshots, copy, stop.

Done when:

- UI test on recorded fixtures

### T12.6. macOS: composer

Stage: S12 · Area: desktop · Depends on: T12.5 · Blocks: 1 task(s)

Text input, file and image attachments, push-to-talk mic button, model picker for the chat.

Done when:

- UI test

### T12.7. macOS: approvals and takeover

Stage: S12 · Area: desktop · Depends on: T12.5, T5.3 · Blocks: 0 task(s)

Inline approval cards (action, risk, why, once/always/never), takeover banner, user notifications when the window is hidden.

Done when:

- UI test; notification callback covered by a fixture

### T12.8. macOS: voice UI

Stage: S12 · Area: desktop · Depends on: T12.6, T8.4 · Blocks: 1 task(s)

Listening/thinking/speaking indicator, live partial transcript, wake-word toggle, barge-in feedback.

Done when:

- UI test with a fake voice stream

### T12.9. macOS: settings — providers and models

Stage: S12 · Area: desktop · Depends on: T12.3, T4.4 · Blocks: 0 task(s)

Add providers, store keys (sent to the daemon, kept in the keychain), default models per tier.

Done when:

- UI test

### T12.10. macOS: settings — voice and audio

Stage: S12 · Area: desktop · Depends on: T12.3, T7.2, T6.3 · Blocks: 0 task(s)

STT and TTS engine and voice pickers with preview, model downloads with progress, audio devices, hotkeys, wake words.

Done when:

- UI test

### T12.11. macOS: settings — MCP servers and plugins

Stage: S12 · Area: desktop · Depends on: T12.3, T10.4, T11.10 · Blocks: 0 task(s)

List, add, enable and disable MCP servers and plugins with status and errors.

Done when:

- UI test

### T12.12. macOS: settings — permissions and audit

Stage: S12 · Area: desktop · Depends on: T12.3, T5.9 · Blocks: 0 task(s)

Rule editor, grants with revoke, blocklist, audit viewer with verify.

Done when:

- UI test

### T12.13. macOS: quick-talk panel

Stage: S12 · Area: desktop · Depends on: T12.8 · Blocks: 0 task(s)

Global hotkey opens a small floating panel for one voice or text request.

Done when:

- UI test for the panel state

### T12.14. macOS: agent screen viewer

Stage: S12 · Area: desktop · Depends on: T12.5, T9.15 · Blocks: 0 task(s)

Live view of the agent's browser or desktop actions (screenshot stream) with Take over and Stop.

Done when:

- UI test with fake frames

### T12.15. Windows app scaffold (WinUI 3)

Stage: S12 · Area: desktop · Depends on: T12.2 · Blocks: 4 task(s)

desktop/windows WinUI 3 (C#) project on the aulo-ffi C# bindings: window, tray icon, connection, startup entry for aulod.

Done when:

- app builds on the Windows runner; connects to a test daemon

### T12.16. Windows: chat list, chat view and composer

Stage: S12 · Area: desktop · Depends on: T12.15, T4.8 · Blocks: 1 task(s)

Parity with T12.4–T12.6 on WinUI.

Done when:

- UI tests on recorded fixtures

### T12.17. Windows: approvals, takeover and voice UI

Stage: S12 · Area: desktop · Depends on: T12.16, T5.3, T8.4 · Blocks: 1 task(s)

Parity with T12.7 and T12.8; toast notifications for approvals.

Done when:

- UI tests

### T12.18. Windows: settings

Stage: S12 · Area: desktop · Depends on: T12.15, T4.4, T7.2, T10.4, T5.9 · Blocks: 0 task(s)

Parity with T12.9–T12.12.

Done when:

- UI tests

### T12.19. Windows: quick-talk and agent screen viewer

Stage: S12 · Area: desktop · Depends on: T12.17, T9.15 · Blocks: 0 task(s)

Parity with T12.13 and T12.14.

Done when:

- UI tests

### T12.20. Multiple servers

Stage: S12 · Area: desktop · Depends on: T12.1 · Blocks: 0 task(s)

aulo-app keeps a list of connections (local daemon, remote servers) and switches between them; both apps expose it.

Done when:

- aulo-app test switches servers

### T12.21. Interface languages

Stage: S12 · Area: desktop · Depends on: T12.3, T12.15 · Blocks: 0 task(s)

English and Russian strings in both apps (String Catalogs on macOS, .resw on Windows).

Done when:

- missing-string lint passes

## S13. Server mode

### T13.1. Headless server profile

Stage: S13 · Area: server · Depends on: T3.8, T8.4 · Blocks: 3 task(s)

aulod run --headless --listen 0.0.0.0:7443: TLS required, no local audio devices, voice only through VoiceService streams.

Done when:

- test: headless start refuses plain TCP

### T13.2. Several clients per chat

Stage: S13 · Area: server · Depends on: T13.1, T4.8 · Blocks: 0 task(s)

Clients attach to the same chat; events fan out; presence per client feeds unattended detection.

Done when:

- test with two clients

### T13.3. API tokens with scopes

Stage: S13 · Area: server · Depends on: T13.1 · Blocks: 1 task(s)

aulo token create/ls/revoke with scopes (read, chat, voice, control, admin); stored hashed; checked per RPC.

Done when:

- tests: a read token cannot submit a turn

### T13.4. Rate limits and quotas

Stage: S13 · Area: server · Depends on: T13.3 · Blocks: 0 task(s)

Per-token request rate, concurrent streams, audio minutes and token spend limits.

Done when:

- tests for each limit

### T13.5. Container image (agent computer)

Stage: S13 · Area: server · Depends on: T13.1, T9.7 · Blocks: 0 task(s)

OCI image with aulod, headless Chromium and a virtual display, so computer control runs inside a container like the Grok Bot agent computer; compose file with a volume for the home.

Done when:

- image builds in CI; smoke test runs a browser tool

### T13.6. Client examples from the protos

Stage: S13 · Area: server · Depends on: T3.2 · Blocks: 0 task(s)

buf generate examples for Python and TypeScript clients (text chat and voice stream).

Done when:

- examples run against a test server in CI

## S14. Bots, routines and memory

### T14.1. Bots (named teammates)

Stage: S14 · Area: bots · Depends on: T4.8, T5.3 · Blocks: 0 task(s)

Several named bots, each with its own prompt, tiers, voice, wake word, grants and workspace; chats belong to a bot.

Done when:

- test: two bots keep separate grants

### T14.2. Background turns

Stage: S14 · Area: bots · Depends on: T4.8 · Blocks: 1 task(s)

A turn keeps running when every client disconnects and reports when one reattaches.

Done when:

- test: disconnect and reattach sees the finished turn

### T14.3. Routines

Stage: S14 · Area: bots · Depends on: T14.2, T5.7 · Blocks: 0 task(s)

Scheduled or recurring runs (cron syntax) that run unattended (read-only mode), post a summary to the chat and notify.

Done when:

- test with a fake clock

### T14.4. Long-term memory

Stage: S14 · Area: bots · Depends on: T2.10, T4.6 · Blocks: 0 task(s)

Facts and preferences with explicit remember and forget, retrieval into the prompt, list and delete from the desktop app.

Done when:

- test: forgotten fact never reaches the prompt

### T14.5. Teach a task

Stage: S14 · Area: bots · Depends on: T9.12, T11.8 · Blocks: 0 task(s)

Record a demonstration (screenshots, AX events, voice narration) and turn it into a draft SKILL.md for the user to review.

Done when:

- a recorded fixture produces a reviewable skill

### T14.6. Channel plugin example: Telegram

Stage: S14 · Area: bots · Depends on: T11.3 · Blocks: 0 task(s)

A plugin that bridges a Telegram bot to a chat, with sender pairing and approval.

Done when:

- unknown sender is refused until paired

## S15. Release and docs

### T15.1. Release of aulod and aulo CLI

Stage: S15 · Area: release · Depends on: T3.7 · Blocks: 0 task(s)

cargo-dist for macOS aarch64, Linux x86_64/aarch64, Windows x86_64; Homebrew tap; git-cliff changelog; bump workflow from pyrlyn/infra.

Done when:

- dry-run release builds all targets

### T15.2. macOS desktop bundle

Stage: S15 · Area: release · Depends on: T12.3 · Blocks: 0 task(s)

App bundle with mic, Accessibility and Apple Events usage strings, signing, notarization and DMG.

Done when:

- notarized DMG from CI

### T15.3. Windows app and Linux packages

Stage: S15 · Area: release · Depends on: T12.15 · Blocks: 0 task(s)

MSIX for the Windows app; deb and AppImage for aulod and the CLI on Linux.

Done when:

- packages build in CI

### T15.4. User guide

Stage: S15 · Area: release · Depends on: T8.16, T9.18 · Blocks: 0 task(s)

docs/: install, first run, voice setup, providers and keys, permissions, MCP, plugins, server mode.

Done when:

- docs build; every config key is documented (lint)

### T15.5. API reference

Stage: S15 · Area: release · Depends on: T3.1 · Blocks: 0 task(s)

Generated reference for the gRPC API from the protos (buf / protoc-gen-doc).

Done when:

- reference regenerates in CI
