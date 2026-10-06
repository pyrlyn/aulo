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
