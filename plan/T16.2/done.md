Model: ZCode / GLM-5.3 · Status: done 2026-10-07 · Priority: P1 · Complexity: 2 · Files: `crates/aulo-speech-system/src/resample.rs`
Check: `cargo test -p aulo-speech-system resample` — 5 passed (new: `a_non_positive_input_rate_passes_through`, covering 0 Hz and −16 kHz); `cargo fmt --check` clean.
