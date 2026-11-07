Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P3 · Complexity: 1 · Files: `crates/aulo-speech-local/src/model.rs`, `scripts/sync-issues.py`
Check: `cargo test -p aulo-speech-local model` — 2 passed; `python3 -c "import ast; ast.parse(open('scripts/sync-issues.py').read())"`; `cargo fmt --check` clean.
