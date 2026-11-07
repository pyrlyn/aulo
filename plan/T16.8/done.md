Recorded as a second `T16.7` in the old `done.md`; renumbered to T16.8 when the tasks moved into `plan/`.

Validation: the subprocess regression failed before isolation with a synthetic OPENAI_API_KEY. All workspace cargo tests and clippy --workspace --all-targets -- -D warnings pass after the fix, as does cargo fmt --check. The existing trycmd env.inherit option isolates the no-credentials fixture without changing production credential resolution.
