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
| protoc (libprotoc 36.2) | brew (`PROTOC` or `PATH`) | `aulo-proto` build script compiles `proto/aulo/v1`; not vendored so the version follows the platform package | https://github.com/protocolbuffers/protobuf |
| mise | brew | Pins the Rust toolchain and just | https://github.com/jdx/mise |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| diesel | local | https://github.com/diesel-rs/diesel | Typed SQLite access in `aulo-store` (no raw SQL) |
| diesel_migrations | local | https://github.com/diesel-rs/diesel | Embedded, filename-keyed schema migrations |
| figment | local | https://github.com/SergioBenitez/Figment | Layered config with per-key provenance |
| libsqlite3-sys | local | https://github.com/rusqlite/rusqlite | Bundled SQLite for Diesel |
| opentelemetry | local | https://github.com/open-telemetry/opentelemetry-rust | OTel API for the optional OTLP trace export in `aulo-telemetry` (`otlp` feature) |
| opentelemetry-otlp | local | https://github.com/open-telemetry/opentelemetry-rust | OTLP/HTTP span exporter, `otlp` feature only |
| opentelemetry_sdk | local | https://github.com/open-telemetry/opentelemetry-rust | Tracer provider and batch span processor, `otlp` feature only |
| prost | local | https://github.com/tokio-rs/prost | Protobuf messages for the gRPC API, only in `aulo-proto` |
| prost-types | local | https://github.com/tokio-rs/prost | Well-known protobuf types (`Timestamp`) used by the generated code |
| regex | local | https://github.com/rust-lang/regex | Credential and secret-pair patterns in the log redaction |
| schemars | local | https://github.com/GREsau/schemars | JSON Schema generated from the config types |
| serde | local | https://github.com/serde-rs/serde | Serialization of ids, events and config |
| serde_json | local | https://github.com/serde-rs/json | Config schema output and overrides; round-trip tests; `cargo metadata` parsing in the dependency-graph test |
| tempfile | local (dev) | https://github.com/Stebalien/tempfile | Temporary databases and directories in tests |
| thiserror | local | https://github.com/dtolnay/thiserror | Typed errors in library crates |
| tonic | local | https://github.com/hyperium/tonic | gRPC server and client runtime, only in `aulo-proto` |
| tonic-prost | local | https://github.com/hyperium/tonic | Prost codec for tonic |
| tonic-prost-build | local (build) | https://github.com/hyperium/tonic | Generates server and client code from the protos |
| tracing | local | https://github.com/tokio-rs/tracing | Structured logs and spans |
| tracing-appender | local | https://github.com/tokio-rs/tracing | Daily-rotating, non-blocking log files with a bounded file count |
| tracing-opentelemetry | local | https://github.com/tokio-rs/tracing-opentelemetry | Bridges tracing spans to OTel, `otlp` feature only |
| tracing-subscriber | local | https://github.com/tokio-rs/tracing | EnvFilter, JSON and human log formatters |
| ulid | local | https://github.com/dylanhart/ulid-rs | Sortable ids for bots, chats, turns, calls and stored rows |
