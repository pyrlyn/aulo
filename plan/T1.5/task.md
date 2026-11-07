# T1.5. Extract llm-catalog: model catalog and prices

Stage: S1 · Area: shared · Depends on: T1.1 · Blocks: 2 task(s)

Execution plan: in a packages/crates worktree branched from t1-1-llm-wire, create llm-catalog (publish = false, dual licence, rust-version 1.98) from cox-models: built-in rows, config overlay, prices.toml, overlay of served models, with Catalog::load, get and effort_for tests; crate-level project files like change-preview; verify cargo clippy/fmt/test. cox adoption (T1.6) waits for publication.

Move cox-models (built-in rows, config overlay, prices.toml, overlay of served models) into packages/crates/llm-catalog.

Done when:

- Catalog::load, get and effort_for tests pass
