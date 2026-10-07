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
