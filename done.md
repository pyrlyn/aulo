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

### T3.5. aulo-server: ChatService

Stage: S3 · Area: api · Depends on: T3.3, T2.8 · Blocks: 5 task(s)

Implement chat CRUD and message listing over aulo-store with pagination.


Done when:

- grpc integration tests for every RPC

Outcome: ChatApi over Arc<Mutex<Store>> on spawn_blocking: Create/List/Get/Rename/Delete chats and ListMessages with keyset page tokens, canonical id parsing, title and ModelRef validation, generic INTERNAL errors; SearchMessages UNIMPLEMENTED until T2.10. Model stored as provider/model; role and status strings user|assistant|tool and running|succeeded|failed|denied|cancelled until aulo-types gets enums.

### T3.4. aulo-server: authentication

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 3 task(s)

Local socket: trust by file owner and 0600 permissions. TCP: bearer token required, compared in constant time, interceptor rejects missing tokens. The first token is generated on first run and stored in the OS keychain.


Done when:

- tests: TCP without token is rejected; UDS from owner is accepted

Outcome: aulo_server::auth: Unix socket peers must match the daemon euid (rustix); TCP needs exactly one Bearer token compared in constant time (subtle) against its SHA-256, health and reflection included; no TCP listener without an installed token. ApiToken is aulo_ + 64 hex from getrandom, redacted Debug, masked by aulo-telemetry. TokenStore with keyring 4 and in-memory stores. Windows pipe clients trust the default DACL (no per-client check without unsafe FFI). remote_auth_configured stays the non-loopback gate until TLS (T3.8); token scopes not modelled yet.

### T7.6. STT: OpenAI-compatible transcription endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

/v1/audio/transcriptions client: works with OpenAI (GPT Transcribe), runa (local server) and other compatible servers. Streaming variant where the server supports it.


Done when:

- wiremock tests for both shapes

Outcome: New adapter crate aulo-speech-cloud: CloudStt over POST /v1/audio/transcriptions (multipart WAV via hound), JSON and SSE (sse-core) shapes chosen by reply Content-Type, preallocated utterance buffer with Overflow, abort on cancel, no retries, no redirects, key never sent over plain http off loopback, error bodies never read, transcript and reply caps; 401/403/429/connect map to Unavailable for registry fallback. Registration under engine ids and [voice.stt] keys for timeouts left to the assembly task.

### T2.10. aulo-store: full-text search over messages

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 1 task(s)


FTS5 virtual table over message text. Raw SQL stays in the storage crate with a comment that Diesel cannot model FTS5.

Done when:

- search test finds a message by a word

Outcome: Migration 000200_message_search: external-content FTS5 table over messages.content (unicode61, remove_diacritics 2) with insert/update/delete triggers and a rebuild of existing rows. Store::search_messages runs one bound-parameter sql_query ordered by bm25 then id with keyset cursor; user text becomes ANDed quoted phrases (1 KiB, 32 words), so FTS syntax is plain text. ChatService.SearchMessages wired with s1.<rank>.<id> tokens. Proto has no bot filter or snippet field; the index keys on the implicit rowid, so a future VACUUM needs a rebuild.

### T7.13. TTS: OpenAI speech endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)


OpenAI-compatible /v1/audio/speech with streaming PCM; voice list from config.

Done when:

- wiremock test

Outcome: CloudTts in aulo-speech-cloud over POST /v1/audio/speech with response_format pcm (24 kHz s16le mono, layout unverified from the OpenAPI spec): one request per pushed sentence in order, 100 ms chunks through a bounded queue of 8 so a slow consumer slows the server, text queue of 64 with Overflow, abort on cancel or begin, total audio cap, idle timeout; voice from request, spec or config default within a configured list. Shared endpoint, auth, client and error mapping moved into a private net module used by STT too. Speed comes from TtsRequest.rate; EngineSpec.rate is ignored.

### T7.10. TTS: macOS system voices

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

AVSpeechSynthesizer via objc2-avf-audio writing buffers into aulo-audio (not straight to the speaker, so barge-in and AEC still work). Lists installed voices.


Done when:

- manual check; voice-list test behind a macOS cfg

Outcome: New adapter crate aulo-speech-system (macOS only): AVSpeechSynthesizer write(_:toBufferCallback:) on a dedicated worker thread, buffers pushed by try_lock into a fixed 1M-sample ringbuf with overflow counted, a generation fence for cancel, linear resampling only on rate mismatch, voice choice by tag then primary subtag then quality. The buffer callback fires only on the main dispatch queue, so hosts (aulod, aulo-ffi) must keep the main run loop running and run tokio elsewhere; without it the engine reports Unavailable after 5 s. unsafe is allowed only in src/macos with SAFETY comments. Manual WAV check, no speaker playback.

### T3.8. aulo-server: TLS for TCP listeners

Stage: S3 · Area: api · Depends on: T3.4 · Blocks: 1 task(s)


rustls; user-provided certificate or a generated self-signed one with fingerprint pinning in clients (`aulo connect --pin`).

Done when:

- test: client with wrong pin is refused

Outcome: aulo_server::tls: TlsIdentity from user PEM files or a self-signed rcgen identity kept 0700/0600 under a given directory (never regenerated over a lone certificate), CertFingerprint (SHA-256, openssl form), PinnedServerVerifier and pinned_client_config for clients. TcpListen carries Option<TlsIdentity> instead of remote_auth_configured: off loopback needs TLS and a token. tonic tls-aws-lc with one process-wide aws-lc-rs provider and a 10 s handshake timeout. Not checked: certificate expiry and permissions of user-provided keys; aulod still passes tls: None and the client connector lives only in tests.

### T7.4. STT: sherpa-onnx (Parakeet TDT v3, Moonshine)

Stage: S7 · Area: speech · Depends on: T7.3, T0.5 · Blocks: 1 task(s)


Default local STT. Parakeet TDT v3 (25 languages incl. ru, uk) on VAD segments; Moonshine streaming for low-latency English partials.

Done when:

- WER test on fixture WAVs under the threshold from the spike

Outcome: New adapter crate aulo-speech-sherpa (sherpa-onnx =1.13.8): ParakeetFactory validates the catalog model files (id, sizes, ONNX header, token table, UTF-8 paths) before sherpa sees them and loads the model once per factory; ParakeetStt decodes VAD segments on one worker thread with a bounded job queue, preallocated utterance buffer, cancel, 25-language check and transcript cap; PARAKEET_ATTRIBUTION carries the CC-BY-4.0 notice. WER 0.000 on 5 fixture clips (en, ru) with a gated test and MAX_WER 0.10. Moonshine left out (offline-only in 1.13.8, no verified bundle); offline builds via SHERPA_ONNX_ARCHIVE_DIR or SHERPA_ONNX_LIB_DIR. Follow-ups in ideas.md.

### T3.9. Run aulod as a user service

Stage: S3 · Area: api · Depends on: T3.6 · Blocks: 0 task(s)


aulod service install/uninstall: launchd agent on macOS, systemd user unit on Linux, Task Scheduler entry on Windows.

Done when:

- install and uninstall are idempotent (tested with a fake service manager)

Outcome: aulod service install|uninstall|status over a ServiceManager trait (std fs and Command, no shell) with launchd (dev.aulo.aulod LaunchAgent, bootstrap/bootout), systemd user unit (daemon-reload only on change, enable --now, restart on update) and schtasks ONLOGON backends; Fresh/Updated/Unchanged results, uninstall of nothing runs no command. Per-format quoting (XML, systemd, Windows CRT), control characters and non-UTF-8 paths refused. AULO_HOME goes in the unit environment, or as a hidden --home flag on Windows. 17 unit tests with a stateful fake manager plus trycmd fixtures.

### T7.14. TTS: ElevenLabs streaming

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)


WebSocket streaming TTS with voice id and model from config; key from the keychain.

Done when:

- recorded-session test

Outcome: ElevenLabsTts in aulo-speech-cloud over the stream-input WebSocket (tokio-tungstenite with rustls, base64, futures-util): xi-api-key header never in the URL, validated voice id in the path, pcm_<rate> output, init/text/close message sequence with a flush per sentence so the first one plays early, audio through the shared bounded reply queues (new private reply.rs shared with OpenAI TTS), abort on cancel or begin, message, audio and text caps; auth, quota, rate and connect failures map to Unavailable and server text is never echoed. Verified against a synthetic recorded session only; error and close-code layout marked unverified. About 550 net lines including the moved reply code.

### T7.12. TTS: Linux speech-dispatcher / espeak-ng

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)


Speech-dispatcher (SSIP) or espeak-ng subprocess as the zero-download fallback voice.

Done when:

- test on the Linux runner with espeak-ng installed

Outcome: espeak module in aulo-speech-system (compiles and is tested on every OS; exported as the Linux system engine): one espeak-ng process per sentence with text on stdin, no shell, voice ids only from --voices, sample rate probed from the WAV header and checked per utterance, decoding through hound into a fixed 2^18-sample ring with backpressure and counted drops, cancel and Drop kill and reap the child, a generation fence against stale audio. speech-dispatcher left out: it plays audio itself and would bypass aulo-audio and AEC. The binary path is trusted startup config, never EngineSpec.model. Linux CI installs espeak-ng; the real-binary test is unverified until CI runs. About 584 non-test lines, over the 500 cap.

### T7.7. STT: Deepgram streaming

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)


Official deepgram crate, streaming over WebSocket with interim results.

Done when:

- recorded-session test

Outcome: DeepgramStt in aulo-speech-cloud over the live listen WebSocket, reusing the T7.14 tokio-tungstenite stack and net helpers instead of the official deepgram crate (it pulls a second tokio-tungstenite 0.28 with its own rustls roots, tokio full, and has no message-size cap). Token header never in the URL, linear16 16 kHz frames from a bounded 64-chunk queue, KeepAlive after 4 s of quiet, CloseStream on finish with a bounded wait; cumulative Partials and one Final after finish (speech_final does not end the turn; Finalize not sent). language None sends multi. 401/402/403/429 and connect failures map to Unavailable, server text never echoed, message, byte and transcript caps. Tested against a synthetic recorded session only.

### T7.9. TTS: sherpa-onnx (Kokoro, Matcha, VITS)

Stage: S7 · Area: speech · Depends on: T7.3, T0.5 · Blocks: 1 task(s)


Default local TTS. Kokoro (Apache-2.0 weights) with voice list; sentence-chunked streaming synthesis so the first sentence plays while the rest is generated.

Done when:

- time-to-first-audio test within the budget

Outcome: KokoroTts in aulo-speech-sherpa (sherpa-kokoro, kokoro-multi-lang-v1_0, 24 kHz): 54 speakers with names and nine languages from the primary sources, per-text phoneme language, one job per sentence in order through the generic worker shared with Parakeet, 100 ms chunks through a per-reply channel of 8, instant cancel, text caps and cleaning; Russian and unknown voices return Unsupported for fallback. Bundle files validated before sherpa sees them, load checks 24 kHz and 54 speakers, then a warm-up. Catalog entry of 377 files with SHA-256 computed from the official k2-fsa archive (digest matched) and cross-checked against the Hugging Face commit; licence labelled Apache-2.0 AND GPL-3.0-or-later. Time to first audio 0.24 s (en) and 0.16 s (es) on 4 threads, under the 0.3 s budget. About 547 non-test lines, over the 500 cap.

### T7.16. Speech benchmarks

Stage: S7 · Area: speech · Depends on: T7.4, T7.9 · Blocks: 0 task(s)


WER (runa-media word_error_rate), real-time factor and time-to-first-audio per engine on fixtures; divan benches and a report command.

Done when:

- aulo bench speech prints a table

Outcome: Outcome: new crate aulo-speech-bench (WER as a small word-level Levenshtein, since the only crates.io candidate rwer 0.2.2 is new and heavy; embedded fixture clips; push/poll timing harness; engine discovery; table and JSON report) and `aulo bench speech [--engine ID] [--json]`. Unavailable engines get a row with the reason; cloud engines are built only when their key is set. divan benches for WER, decode, resampling and the harness. The sherpa WER test now uses the shared WordErrors. Parakeet and Kokoro rows are untested end to end without local models; ElevenLabs model and voice ids are unverified.

### T8.10. aulo-realtime: OpenAI realtime voice client

Stage: S8 · Area: voice · Depends on: T1.3 · Blocks: 1 task(s)

WebSocket client (tokio-tungstenite) for OpenAI realtime voice models (GPT-Live / Realtime), mapping their events to aulo-types; ephemeral client tokens for remote clients.


Done when:

- recorded-session test

Outcome: New crate aulo-realtime: tokio-tungstenite client for the OpenAI Realtime and GPT-Live voice sockets, events mapped to aulo-types, client-secret minting for remote clients; ApiKey and the loopback endpoint check shared from aulo-types. Recorded-session tests replay fixtures shaped from the docs (no live key). About 840 non-test lines, kept as one task by the creator's decision. Checks: cargo test --workspace 475 passed; clippy -D warnings and fmt clean. Note for T8.11: a remote client must not talk to the model directly, because a client connection can override a minted session's instructions and tools.

### T16.1. `daemon.listen` can never work: the daemon never installs an API token

`crates/aulo/src/daemon/serve.rs` built `ApiServer::new(Limits::default())` without `.with_token(...)`, so `ApiServer::serve` always refused the TCP listener (`ServerError::TcpWithoutToken`, covered by `aulo-server` tests) — a user setting the documented `daemon.listen` key or `aulod --listen` got a daemon that bound the socket and then exited. The `KeychainTokenStore::load_or_create` machinery was exported but never called. Found by the 2026-10-07 audit. Fix: when `daemon.listen` asks for TCP, the daemon loads-or-creates the token (`dev.aulo.daemon` service, one keychain account per home, so a test daemon never touches the real one's) and serves with it; a unix-socket daemon still never touches the keychain, and a token failure at startup is a startup error rather than a post-bind exit.
Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P1 · Complexity: 2 · Files: `crates/aulo/src/daemon/serve.rs`
Check: `cargo check -p aulo` clean; `cargo test -p aulo --bin aulod` — 17 passed; the refusal path itself is pinned by `aulo-server` `auth.rs:109` and `tls.rs:165`.

### T16.2. `Resampler` hangs on a zero/negative input rate

`crates/aulo-speech-system/src/resample.rs` computed `step = input_hz / output_hz` whenever the rates differed and only guarded `output_hz > 0`: a malformed buffer reporting 0 Hz (or negative — the macOS callback path feeds `format.sampleRate()` straight in, `src/macos/worker.rs:200-218`) produced a `step <= 0` that never advanced `pos`, and `push`'s emit loop spun forever on the main dispatch queue, hanging the whole process. Found by the 2026-10-07 audit. Fix: non-positive input rates pass samples through, like equal rates — the same degradation the wav path's 8000..=192000 guard reaches by refusing the input.
Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P1 · Complexity: 2 · Files: `crates/aulo-speech-system/src/resample.rs`
Check: `cargo test -p aulo-speech-system resample` — 5 passed (new: `a_non_positive_input_rate_passes_through`, covering 0 Hz and −16 kHz); `cargo fmt --check` clean.
### T16.3. `OfflineStt::begin` validates the language before cancelling

`crates/aulo-speech-local/src/offline.rs` returned `unsupported` before `self.cancel()`, so an utterance left in `Recording` or `Waiting` survived a failed `begin` — violating the trait contract "`begin` … implies `cancel`" (`aulo-speech/src/stt.rs`) that deepgram and the macOS engine honour. Found by the 2026-10-07 audit. Fix: the cancel runs first; a rejected language still ends the utterance in progress.
Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P2 · Complexity: 1 · Files: `crates/aulo-speech-local/src/offline.rs`
Check: `cargo test -p aulo-speech-local offline` — 9 passed (new: `a_rejected_language_still_cancels_the_utterance_in_progress`); `cargo fmt --check` clean.
