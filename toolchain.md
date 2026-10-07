# Toolchain

Only what the project uses today. Crates are added by the task that wires each of them in; the planned stack is in `spec.md` and `research.md` Part 4.

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| python3 | system / mise | Runs `scripts/sync-issues.py` (stdlib only) | https://github.com/python/cpython |
| gh | brew / mise | Creates labels, milestones and issues from `plan.md` | https://github.com/cli/cli |
| rustc, cargo, rustfmt, clippy | mise (`mise.toml`, 1.99.0) | Build, format and lint the workspace | https://github.com/rust-lang/rust |
| just | mise (`mise.toml`) | Task runner (`justfile`) | https://github.com/casey/just |
| buf (1.73.0) | brew | Lints and builds the protos, `buf format`, breaking-change check (`proto/buf.yaml`) | https://github.com/bufbuild/buf |
| mise | brew | Pins the Rust toolchain and just | https://github.com/jdx/mise |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| diesel | local | https://github.com/diesel-rs/diesel | Typed SQLite access in `aulo-store` (no raw SQL) |
| diesel_migrations | local | https://github.com/diesel-rs/diesel | Embedded, filename-keyed schema migrations |
| figment | local | https://github.com/SergioBenitez/Figment | Layered config with per-key provenance |
| libsqlite3-sys | local | https://github.com/rusqlite/rusqlite | Bundled SQLite for Diesel |
| schemars | local | https://github.com/GREsau/schemars | JSON Schema generated from the config types |
| serde | local | https://github.com/serde-rs/serde | Serialization of ids, events and config |
| serde_json | local | https://github.com/serde-rs/json | Config schema output and overrides; round-trip tests |
| thiserror | local | https://github.com/dtolnay/thiserror | Typed errors in library crates |
| ulid | local | https://github.com/dylanhart/ulid-rs | Sortable ids for bots, chats, turns, calls and stored rows |
| tempfile | local (dev) | https://github.com/Stebalien/tempfile | Temporary databases and directories in tests |
