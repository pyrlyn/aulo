Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P1 · Complexity: 2 · Files: `crates/aulo/src/daemon/serve.rs`
Check: `cargo check -p aulo` clean; `cargo test -p aulo --bin aulod` — 17 passed; the refusal path itself is pinned by `aulo-server` `auth.rs:109` and `tls.rs:165`.
