# aulo — done

### T0.1. Decide the licence and how cox code is reused

Stage: S0 · Area: decision · Depends on: none · Blocks: 8 task(s)

cox is GPL-3.0-or-later OR LicenseRef-cox-Royalty-Free, runa is MIT OR Apache-2.0. aulo reuses the provider stack, permissions, sandbox, MCP host and voice capture from cox. Options: (a) license aulo like cox and depend on extracted crates as they are; (b) license aulo MIT OR Apache-2.0 and relicense the extracted crates when they move into packages/. The creator decides; record the answer in spec.md (Decisions) and add LICENSE. Outcome (creator, 2026-10-07): licensed like cox — GPL-3.0-or-later OR LicenseRef-aulo-Royalty-Free, plus a commercial licence (PRICING.md). LICENSE and PRICING.md are byte copies of the canonical files in pyrlyn/ci licenses/; LICENSE-ROYALTY-FREE.md is the cox text; the README carries the license-sync block and license-check.yml guards drift. The extracted shared crates stay GPL-compatible, so aulo may depend on cox code directly.

Done when:

- spec.md Decisions lists the licence and the reason
- LICENSE file(s) committed

### T0.2. Decide the desktop UI toolkit

Stage: S0 · Area: decision · Depends on: none · Blocks: 1 task(s)

Candidates: Slint (in rust.md, Rust-only, one codebase, gRPC client in-process; recommended), native SwiftUI/WinUI over UniFFI (the ketch/cox pattern, best platform feel, two codebases), Tauri (web UI, heavier). Record the choice and the reason. Outcome (creator, 2026-10-07): native apps as in cox — SwiftUI on macOS and WinUI 3 on Windows, over a UI-agnostic Rust core (aulo-app) exported with UniFFI (aulo-ffi; Swift bindings, C# via uniffi-bindgen-cs).

Done when:

- spec.md Decisions names the toolkit

### T0.3. Decide supported platforms and minimum OS versions

Stage: S0 · Area: decision · Depends on: none · Blocks: 4 task(s)

Proposal: macOS 15+ on arm64 only (background-window control, VP-IO), Windows 11 x86_64, Linux x86_64/aarch64 (X11 and Wayland). The server mode runs on Linux first. Outcome (creator, 2026-10-07): macOS (arm64), Windows 11 (x86_64) and Linux (x86_64, aarch64). Minimum macOS stays 15 as proposed (background-window control); a native Linux desktop app is not decided yet.

Done when:

- spec.md Decisions lists targets and minimum versions

### T0.4. Approve the new crates for rust.md

Stage: S0 · Area: decision · Depends on: none · Blocks: 0 task(s)

New crates the plan needs that rust.md does not list yet: tonic, tonic-build, prost, tonic-health, tonic-reflection, sherpa-onnx, earshot, rodio, ringbuf, webrtc-audio-processing, chromiumoxide, xcap, enigo, objc2-application-services, objc2-avf-audio, atspi, uiautomation, handy-keys, tokio-tungstenite (already listed as dev). The creator approves the list; each crate gets its rust.md and toolchain.md rows in the task that wires it in. Outcome (creator, 2026-10-07): the list above is approved.

Done when:

- approved list recorded in spec.md Decisions

### T0.7. Decide how the agent loop is reused from cox

Stage: S0 · Area: decision · Depends on: none · Blocks: 2 task(s)

cox-core is a coding-oriented state machine (Submission in, Event out, tools, approvals, interrupts, compaction). Options: (a) extract a neutral agent-loop crate from cox-core into packages/ and build aulo on it (recommended, satisfies the no-duplication rule); (b) depend on cox-core as a whole; (c) write a new loop. Record the decision. Outcome (creator, 2026-10-07): option (a), the recommendation — extract a neutral agent-loop crate from cox-core (T1.15) and build aulo-agent on it.

Done when:

- spec.md Decisions records the agent-loop choice

### T2.1. Cargo workspace skeleton

Stage: S2 · Area: infra · Depends on: T0.1 · Blocks: 6 task(s)

Virtual manifest (members = crates/*, resolver 3, edition 2024), workspace.package, workspace.dependencies with one-line reasons, workspace.lints (no unwrap/expect/panic outside tests), dev and dist profiles, mise.toml pinning Rust, justfile, .gitignore.

Execution plan: 1. Root Cargo.toml: virtual workspace, members crates/*, resolver 3, edition 2024, rust-version 1.99, license expression from D1, workspace.lints (deny unwrap_used, expect_used, panic, todo outside tests), dev profile line-tables-only, dist profile (fat LTO, codegen-units 1, strip). 2. crates/aulo: the surface crate with `aulo` and `aulod` bin targets printing their version, so the workspace has a default member. 3. mise.toml pinning Rust 1.99.0 with rustfmt and clippy; justfile (check, test, lint, fmt); rustfmt.toml. 4. Verify: `mise exec -- cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, run both binaries. 5. toolchain.md rows for rustc, cargo, mise, just.

Done when:

- cargo check, clippy and fmt --check pass on an empty workspace

Outcome: root `Cargo.toml` (virtual workspace, resolver 3, edition 2024, rust-version 1.99, D1 licence, shared dependencies with reasons, lints denying unwrap/expect/panic/todo outside tests, `dist` profile), `clippy.toml`, `rustfmt.toml`, `mise.toml` (Rust 1.99.0, just), `justfile`, and `crates/aulo` with the `aulo` and `aulod` binaries printing their version. `cargo check`, `clippy -D warnings` and `fmt --check` pass.

### T2.8. aulo-store: Diesel SQLite with chats and messages

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 3 task(s)

~/.aulo/aulo.db, bundled SQLite, WAL, filename-keyed migrations. Tables: bots, chats, messages, tool_calls, usage. Typed Diesel DSL only.


Done when:

- migration up/down tests; CRUD tests for chats and messages

Outcome: crates/aulo-store: Diesel over bundled SQLite (libsqlite3-sys 0.38 bundled), WAL, foreign keys and busy_timeout on open (the only raw SQL, PRAGMA, which the DSL cannot express), embedded migration 2026-10-07-000000_core_tables (bots, chats, messages, tool_calls, usage). Typed Store API with ULID ids, keyset pagination and cascade delete. 8 tests (migrations up/down/up, CRUD, pagination, foreign keys, cascade, WAL). Follow-ups: role/status strings become aulo-types enums; open() takes &str until aulo-config gives the path; single connection, not Sync.

### T2.4. aulo-types: domain contract

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 4 task(s)

Ids (ULID): BotId, ChatId, TurnId, CallId, ClientId. aulo events beyond agent-loop: VoiceState, Transcript (partial/final), SpeechStarted/Ended, TtsChunk, ApprovalRequired, TakeoverRequested, Notice. No I/O dependencies.


Done when:

- types serialize round-trip; deps test passes

Outcome: crates/aulo-types: BotId, ChatId, TurnId, CallId, ClientId from one ulid_id! macro (plain 26-char ULID in Display, FromStr and serde; IdParseError names the kind and caps the echoed input), and the serde-tagged, non-exhaustive AuloEvent (VoiceState, SpeechStarted/Ended, Transcript partial/final, TtsChunk metadata, ApprovalRequired with needs_click, TakeoverRequested, Notice). No I/O dependencies. 10 tests. Follow-ups: no prefixed id form (the spec defines none); event payload fields are a first reading and may change with T3.1 protos.

### T2.5. aulo-config: typed config with schema and layers

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 5 task(s)

Config types with schemars; committed schemas/aulo.schema.json with a stale test; figment layers default < ~/.aulo/config.toml < project .aulo.toml < AULO_* env < flags, with per-key provenance. The only crate that touches toml/toml_edit/figment. Use config-schema when T1.14 lands.


Done when:

- schema test fails when types change without regenerating
- unknown keys are rejected with the file name in the error

Outcome: crates/aulo-config: Config { daemon, providers, voice, mcp, plugins, policy } with serde(default, deny_unknown_fields) and schemars; Config::load layers defaults < <home>/config.toml (AULO_HOME, else ~/.aulo) < <project>/.aulo.toml < AULO_* env (nested with __) < caller overrides, with per-leaf Provenance. Errors name the file or env var and the key. Committed schemas/aulo.schema.json with a stale test (AULO_UPDATE_SCHEMA=1 regenerates); nulls stripped because TOML has none. 13 tests under figment::Jail. Follow-ups: approval timeout default 60 s; cross-field checks (tier provider exists, TLS files with listen) belong to T3/T4; no upward search for .aulo.toml; move to config-schema when T1.14 lands.

### T2.9. aulo-store: grants and audit tables

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 2 task(s)

Tables for permission grants (subject, scope, decision, expiry) and the audit log (seq, prev_hash, hash, kind, payload).


Done when:

- migrations and CRUD tests pass

Outcome: Migration 2026-10-07-000100_grants_audit: grants (subject, scope, decision, nullable expires_at; index on subject+scope) and append-only audit (seq AUTOINCREMENT, UNIQUE prev_hash so two writers cannot fork the chain, hash, kind, payload). Store gains put/list/revoke grants, list_active_grants (expiry filtered in the Diesel query), append_audit (RETURNING via diesel returning_clauses_for_sqlite_3_35), last_audit, list_audit_after. Hashing and chain verification stay in aulo-policy (T5.9). 12 tests. Follow-ups: duplicate subject+scope grants are kept (T5.x decides precedence); no DB triggers block audit UPDATE/DELETE.

### T3.1. aulo-proto: v1 service definitions

Stage: S3 · Area: api · Depends on: T2.4 · Blocks: 2 task(s)

proto/aulo/v1/*.proto: ChatService (Create/List/Get/Rename/Delete chats, ListMessages), SessionService (bidi Converse: client submissions, server events), VoiceService (bidi Talk: PCM frames and control in; transcripts, state, TTS audio out), ApprovalService, ConfigService (providers, models, voices, devices), McpService, PluginService, AuditService. buf.yaml with lint rules.


Done when:

- buf lint passes; spec.md API section matches the protos

Outcome: proto/buf.yaml (v2, STANDARD lint, FILE breaking) and proto/aulo/v1/{common,event,chat,session,voice,approval,config,mcp,plugin,audit}.proto: ChatService, SessionService.Converse (bidi), VoiceService.Talk (bidi, 16 KiB audio frames, 32 KiB TTS chunks), ApprovalService, ConfigService (GetConfig JSON, SetConfig RFC 7386 merge patch), McpService, PluginService, AuditService (Export streams JSONL). Event oneof mirrors AuloEvent 1:1 plus agent-loop events. Size caps, untrusted fields and per-service scopes documented in comments. buf lint, build and format clean (buf 1.73.0). spec §5.2/§11 aligned (SetVoice on VoiceService; Converse/Talk Request/Response names). Follow-ups: ApprovalDecision adds DENY_ONCE; Chat.model is ModelRef while the store has one string (T3.5); daemon must enforce needs_click on Decide (T5.3).

### T0.5. Spike: sherpa-onnx on macOS arm64

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Build the official sherpa-onnx Rust crate (1.13.x) and run its examples for Parakeet TDT v3 STT, Silero VAD, keyword spotting and Kokoro TTS. Measure build time, binary size, model sizes, real-time factor and time to first audio. Write docs/spikes/sherpa-onnx.md with numbers and sources.


Done when:

- docs/spikes/sherpa-onnx.md with measured numbers
- go/no-go for sherpa-onnx as the default local speech backend

Outcome: docs/spikes/sherpa-onnx.md: sherpa-onnx 1.13.8 on M3 Max, GO. Prebuilt static lib (16 s cold build, 19.6 MB stripped binary). Parakeet TDT v3 int8 RTF 0.07 at 4 threads (640 MiB, 1.2-1.4 GB RSS); Kokoro fp32 RTF 0.23, first audio 0.24-0.64 s (int8 is 3x slower); KWS RTF 0.015, English only; Silero VAD RTF 0.003. Risks: the prebuilt lib links espeak-ng (GPL-3.0), which conflicts with the plugin-process rule (open question for the creator); build.rs downloads without checksum (CI needs SHERPA_ONNX_ARCHIVE_DIR); Kokoro has no Russian; a bad KWS model aborts the process, so engines belong behind a process boundary.

### T2.7. aulo-telemetry: logs and traces

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 1 task(s)

tracing-subscriber JSON logs in ~/.aulo/logs with rotation, level from config/env, optional OTLP behind a feature. Secrets are never logged (redaction layer).


Done when:

- a test proves a key-like value is redacted

Outcome: crates/aulo-telemetry: init(&Settings) -> Guard installs an EnvFilter (AULO_LOG over the level), a daily-rotating JSON file layer (tracing-appender, 14 files kept, non-blocking) and an ANSI-free stderr layer. Redaction runs on the finished line: secret-named JSON keys and name=value pairs, bearer tokens and common key shapes (sk-, ghp_, github_pat_, xox*, AKIA, AIza, JWT) are masked; if the patterns fail to compile everything is masked. Optional otlp feature (opentelemetry 0.33, plain HTTP) with a span processor that masks attributes before export. 16 tests. Follow-ups: mask secret-handle shapes after T5.10; aulo-config has no log-level key yet; invalid AULO_LOG is an error, not a warning.

### T7.1. aulo-speech: engine contracts

Stage: S7 · Area: speech · Depends on: T2.4 · Blocks: 11 task(s)

Traits: SttEngine (push frames, partial and final transcripts), TtsEngine (stream text in, audio chunks out, voices, rate), Vad, KeywordSpotter, TurnDetector. Capabilities: streaming, languages, offline, needs network.


Done when:

- contract tests with fake engines

Outcome: crates/aulo-speech: object-safe, Send, synchronous push-then-poll traits SttEngine (begin/push/finish/poll/cancel; exactly one Final per utterance), TtsEngine (voices, begin -> AudioFormat, push_text, poll into a caller-owned slice, cancel for barge-in), Vad, KeywordSpotter, TurnDetector. Sync because async_trait boxes a future per call on the audio path; cloud and plugin engines use bounded queues and return Overflow. AudioFrame borrows samples (16 kHz mono PIPELINE format), EngineInfo with validated EngineId and Capabilities (streaming, languages, offline, needs_network), SpeechRate 0.5-2.0. One shared SpeechError instead of associated types (a dyn registry needs one type anyway); should_fall_back() drives fail-open; engine error text capped at 512 bytes. 24 tests with fake engines. Follow-ups: T7.2 owns engine factories and maps out-of-range config rate to an error.

### T3.2. aulo-proto: code generation and breaking-change check

Stage: S3 · Area: api · Depends on: T3.1 · Blocks: 3 task(s)

tonic-build/prost code generation; CI runs buf breaking against main.


Done when:

- generated crate builds; CI fails on a breaking field change

Outcome: crates/aulo-proto: build.rs (tonic-prost-build 0.14.6, returns Result) compiles all ten proto/aulo/v1 files, server and client; lib.rs exposes aulo::v1 via include_proto!; generated code passes workspace lints without allows. protoc from PROTOC or PATH. .github/workflows/proto.yml runs buf lint, format and breaking against the PR base (bufbuild/buf-action v1.6.0 and actions/checkout v7.0.1 pinned by SHA). Proven locally: renumbering UserTurn.text makes buf breaking exit 100. 2 round-trip tests. Follow-ups: the Rust CI workflow (T2.3) must install protoc; vendoring protoc is open; the workflow has not run on GitHub yet.

### T2.2. Dependency-graph test

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

crates/aulo/tests/deps.rs reads cargo metadata and enforces crate roles: contracts have no I/O deps, only surfaces depend on clap/anyhow, one owner crate per heavy dependency (diesel, tonic, sherpa-onnx, wasmtime).


Done when:

- the test fails when a contract crate pulls tokio::fs or reqwest

Outcome: crates/aulo/tests/deps.rs parses `cargo metadata --no-deps` (as cox does) against a ROLES table for all 27 spec §4.2 crates: every crate must have a role; dependencies point down only (contract < domain/adapter < assembly < surface, nothing depends on a surface, only a testkit on the testkit); contracts have no I/O deps (tokio, reqwest, hyper, cpal, diesel, wasmtime, uniffi, sherpa-onnx, chromiumoxide, ...); only aulo and aulo-ffi use clap/anyhow; one owner per heavy dependency (diesel -> aulo-store, prost -> aulo-proto, tonic -> aulo-proto/aulo-server/aulo-app, tonic-health/reflection -> aulo-server, sherpa-onnx, whisper, wasmtime, chromiumoxide, uniffi). A synthetic bad graph test asserts each message. Proven by temporarily adding tokio, clap and diesel to aulo-types.

### T11.1. Plugin manifest

Stage: S11 · Area: plugins · Depends on: T2.5 · Blocks: 1 task(s)

aulo-plugin.toml with schemars schema: id, version, kinds (tools, stt, tts, llm-provider, skills, hooks), requested capabilities (net hosts, fs roots, exec, audio), entry (mcp-process, grpc-process, wasm).


Done when:

- schema test; invalid manifest names the file

Outcome: crates/aulo-plugin (manifest module): PluginManifest::load/from_toml for aulo-plugin.toml (id, semver version, kinds set, optional entry mcp-process/grpc-process/wasm, capabilities net/fs_read/fs_write/exec/audio), deny_unknown_fields everywhere, committed schemas/aulo-plugin.schema.json with a stale test. Untrusted-input guards: 64 KiB read cap, id [a-z][a-z0-9]*(-[a-z0-9]+)* up to 32 chars, entry must match kinds (streaming kinds need grpc-process, tools mcp-process or wasm), relative entry paths without .., host-only net patterns (no bare * or *.tld), absolute fs roots, control characters and lengths capped; errors name the file and escape echoed values. 20 tests. Open: manifest types for aulo-plugin-sdk (T11.5), exec as bool vs allowlist (T11.2), no name/description yet (T11.10).

### T0.6. Spike: echo cancellation from Rust

Stage: S0 · Area: spike · Depends on: T0.3 · Blocks: 2 task(s)

Compare macOS Voice Processing I/O (AVAudioEngine setVoiceProcessingEnabled via objc2-avf-audio) with webrtc-audio-processing 2.x (AEC3). Play TTS through speakers while capturing; measure residual echo and false barge-ins. Write docs/spikes/aec.md.


Done when:

- docs/spikes/aec.md with a recommendation per OS

Outcome: docs/spikes/aec.md (M3 Max, built-in speakers and mic, 7 s TTS echo): raw mic -40 dBFS with a never-ending false barge-in; webrtc-audio-processing 2.1.0 AEC3 27.5 dB reduction (30 dB with NS), 0 false barge-ins, cuts the first overlapping word and detects real speech ~1 s late; macOS VP-IO ~38 dB after a 0.5 s leak at playback start, 0 false barge-ins, but ducks other audio. Recommendation: macOS VP-IO default with AEC3 fallback (TTS must play through the engine; barge-in ignores the start leak); Windows 11 OS AEC on a communications WASAPI stream (needs its own spike; AEC3 bundled MSVC build reportedly fails, unverified); Linux AEC3 bundled (meson, ninja; beware Homebrew abseil linking dynamically). Open: a live human barge-in test on both; AEC3 tuning; a 7.5 s burst seen with cpal on built-in speakers.

### T7.15. Speakable-text normalizer

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 1 task(s)

Turns model text into speech input: strips markdown, replaces code blocks and long output with "it is on the screen", shortens URLs, expands units and numbers per language, splits sentences for streaming TTS, picks the voice by detected language.


Done when:

- insta snapshot tests for en and ru

Outcome: crates/aulo-voice (domain): speakable(markdown, SpeakOptions) strips markdown (pulldown-cmark), turns code blocks and tables into "It's on the screen." / "Это на экране.", URLs into hosts, removes bidi/control/zero-width characters, expands %, decimals and ~24 units with Russian plurals, caps input at 64 KiB and speech at 2000 chars. SentenceSplitter and SpeakableStream emit whole sentences from deltas (abbreviation-aware, max chunk) so TTS starts after the first paragraph. detect_language guesses en/ru. insta golden snapshots for en and ru. Limits: numbers not spelled out; dates, times, versions, compound units left as written; the stream waits for a blank line or ~1 KiB.

### T7.3. Model manager

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 4 task(s)

Download speech models from Hugging Face with pinned SHA-256 (cox whisper-models.json pattern) into ~/.aulo/models, progress events, aulo models pull/ls/rm. Refuse a hash mismatch.


Done when:

- test: corrupted download is rejected

Outcome: crates/aulo-models (adapter; added to spec §4.2 and the deps ROLES): embedded data/models.json catalog with schema and stale test (pinned base_url, files with size and sha256; ids and paths reject .., absolute, \, :, .part). ModelManager list/path/pull/remove under <AULO_HOME>/models/<id>/: streams into .part while hashing, caps at the catalog size, fsyncs and renames atomically, refuses hash or size mismatch and deletes the partial, resumes via Range with Content-Range checks, 5 redirects max, timeouts. reqwest 0.13 (rustls), sha2. 14 wiremock tests including a corrupted download. Catalog seeded with Parakeet TDT v3 int8 pinned to HF commit 2bda32e. Left out: Silero VAD and KWS (GitHub release assets without published digests), Kokoro (378 files; needs a catalog generator), tar.bz2 archives.

### T7.2. Engine registry and runtime switching

Stage: S7 · Area: speech · Depends on: T7.1, T2.5 · Blocks: 5 task(s)

Engines register by id (built-in and plugin-provided). [voice.stt] and [voice.tts] select engine, model and voice; a chat or bot can override; switching through gRPC, CLI (aulo voice use) or voice command takes effect on the next utterance without restart. A failing engine is skipped for the next one in the fallback list (fail open).


Done when:

- test switches TTS mid-chat; a failing engine falls back with a notice

Outcome: aulo-voice (domain) gains EngineRegistry<E> (SttRegistry, TtsRegistry; duplicate ids refused so a plugin cannot replace a built-in; unregistered id is Unavailable and falls back), candidates(config, Overrides{bot, chat}) ordering chat > bot > config > fallback list without duplicates, and ActiveEngine<E> whose switch applies at the next begin (never mid-utterance); build failures and should_fall_back errors skip the engine with a Warn Notice, caller bugs and overflow do not. aulo-speech gains EngineSpec, EngineChoice and a testkit feature with the shared fake engines; aulo-config gains EngineConfig::choice, which turns bad engine ids and out-of-range rates into a ConfigError naming key and layer. 14 tests incl. switching TTS mid-chat and fallback with a notice. Decisions: the configured engine stays second behind an override; fallback engines start with default model and voice; validation runs at startup via choice(), not in Config::load.

### T3.3. aulo-server: tonic server scaffold

Stage: S3 · Area: api · Depends on: T3.2, T2.5, T2.7 · Blocks: 4 task(s)

tonic server with tonic-health and reflection, listeners for a Unix socket / Windows named pipe (local) and TCP (remote), graceful shutdown, request size limits.


Done when:

- integration test connects over UDS and calls health

Outcome: crates/aulo-server: ApiServer::new(Limits) mounts tonic-health and reflection (v1, v1alpha) over aulo_proto::FILE_DESCRIPTOR_SET; add_service requires MessageLimits so no service is mounted without size caps (4 MiB both ways), plus per-connection concurrency 32, 64 streams, 30 s timeout. bind(Listen) fails fast: Unix socket under <home>/run (absolute path, parent 0700 or refused, symlinked parent refused, socket 0600, a live socket is never replaced, a stale one only after connection refused, non-sockets untouched), Windows named pipe (first instance, remote clients rejected; not compiled here), TCP refused on non-loopback unless remote auth is configured. serve() with CancellationToken flips health to NOT_SERVING and drains streams for 10 s. 12 tests over a real UDS incl. health, reflection, oversize, shutdown and socket safety. Follow-ups: limits into config; loopback TCP vs the TLS rule (T3.8); owner-only pipe DACL (T3.4).

### T2.6. Embedded default config

Stage: S2 · Area: infra · Depends on: T2.5 · Blocks: 0 task(s)

config/default.toml with every assignment commented out; a test checks it parses to Config::default(). `aulo config show --origin` prints values with provenance.


Done when:

- test passes; trycmd fixture for config show

Outcome: aulo-config embeds config/default.toml as DEFAULT_TOML (## prose, # commented-out settings; unset keys show an example marked unset by default); tests check it parses to Config::default() commented and uncommented, and a schema walk fails when a Config key has no line. Config::to_toml_redacted and Loaded::render_origins mask every string through aulo_telemetry::redact plus URL userinfo. crates/aulo gains a clap skeleton with `aulo config show [--origin]` and `aulo config default`; errors name the file. trycmd fixtures. Follow-ups: aulo-config now depends on aulo-telemetry for redaction (move to text-sanitize when T1.11 lands); keys print alphabetically.

### T3.6. aulod binary

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 2 task(s)

clap surface: aulod run, --config, --listen; single-instance lock per home; pidfile; SIGINT/SIGTERM shutdown.


Done when:

- second instance exits with a clear message; trycmd fixtures

Outcome: aulod [run] [--config DIR] [--listen ADDR]; std File::try_lock single-instance lock with pidfile under <home>/run (0700/0600), SIGINT/SIGTERM graceful shutdown, UDS always and loopback-only TCP; trycmd fixtures for help, version and second instance. --config takes a directory holding config.toml; log level fixed at info until a logging config section exists.
