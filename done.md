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
