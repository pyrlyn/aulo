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
| sherpa-onnx native lib (1.13.8, static) | downloaded by the `sherpa-onnx-sys` build script from the k2-fsa GitHub release into `<target>/sherpa-onnx-prebuilt/`, once per target directory; offline: `SHERPA_ONNX_ARCHIVE_DIR` (directory holding the release archive) or `SHERPA_ONNX_LIB_DIR` (unpacked `lib/`) | Prebuilt sherpa-onnx and onnxruntime linked into `aulo-speech-sherpa`; contains GPL-3.0 espeak-ng (spec §17 D9) | https://github.com/k2-fsa/sherpa-onnx/releases/tag/v1.13.8 |
| mise | brew | Pins the Rust toolchain and just | https://github.com/jdx/mise |
| espeak-ng | apt (`espeak-ng`) on Linux; optional elsewhere | Zero-download fallback voice, run as a separate process (GPL-3.0) by the Linux `system` engine in `aulo-speech-system`; the Linux CI runner needs it for the real-binary test | https://github.com/espeak-ng/espeak-ng |
| protoc-gen-doc (v1.5.1) | buf remote plugin `buf.build/community/pseudomuto-doc` (`buf.gen.yaml`), nothing to install | Generates the Markdown gRPC API reference in `docs/api/` (`just api-docs`) | https://github.com/pseudomuto/protoc-gen-doc |

## cargo

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| anyhow | local | https://github.com/dtolnay/anyhow | Error context in the `aulo` and `aulod` binaries only (surface crates) |
| base64 | local | https://github.com/marshallpierce/rust-base64 | Decodes the base64 audio inside the ElevenLabs WebSocket messages in `aulo-speech-cloud` |
| block2 | local (macOS) | https://github.com/madsmtm/objc2 | Objective-C block for the `AVSpeechSynthesizer` buffer callback (`aulo-speech-system`) |
| clap | local | https://github.com/clap-rs/clap | Command-line parsing for `aulo` (`aulo config show`, `aulo config default`) and `aulod` |
| diesel | local | https://github.com/diesel-rs/diesel | Typed SQLite access in `aulo-store` (no raw SQL) |
| diesel_migrations | local | https://github.com/diesel-rs/diesel | Embedded, filename-keyed schema migrations |
| divan | local (dev) | https://github.com/nvzqz/divan | Benchmarks of the deterministic speech-harness parts (word error rate, resampling, measurement against fake engines) in `aulo-speech-bench` |
| figment | local | https://github.com/SergioBenitez/Figment | Layered config with per-key provenance |
| futures-util | local | https://github.com/rust-lang/futures-rs | Splits the ElevenLabs and Deepgram WebSockets into read and write halves (`StreamExt`, `SinkExt`) in `aulo-speech-cloud`, so a slow audio consumer never stalls the text sent and results are read while audio is written |
| getrandom | local | https://github.com/rust-random/getrandom | OS randomness for the 256-bit `aulod` API token (`aulo-server`) |
| hound | local | https://github.com/ruuda/hound | Encodes the buffered utterance as 16-bit PCM WAV for the upload in `aulo-speech-cloud`; decodes the WAV streams of `espeak-ng --stdout` and WinRT `SpeechSynthesizer` in `aulo-speech-system`; reads fixture WAVs in the `aulo-speech-sherpa` and `aulo-speech-system` tests; decodes the embedded fixture clips in `aulo-speech-bench` |
| hyper-util | local (dev) | https://github.com/hyperium/hyper-util | `TokioIo` adapter for the gRPC client over a Unix socket in `aulo-server` tests |
| insta | local (dev) | https://github.com/mitsuhiko/insta | Snapshot tests of the speakable-text normalizer (`aulo-voice`) |
| keyring | local | https://github.com/open-source-cooperative/keyring-rs | OS keychain that holds the `aulod` API token (`aulo-server`) |
| libsqlite3-sys | local | https://github.com/rusqlite/rusqlite | Bundled SQLite for Diesel |
| objc2 | local (macOS) | https://github.com/madsmtm/objc2 | Objective-C runtime (`Retained`, downcasts) for the macOS system voices (`aulo-speech-system`) |
| objc2-avf-audio | local (macOS) | https://github.com/madsmtm/objc2 | `AVSpeechSynthesizer`, `AVSpeechSynthesisVoice` and `AVAudioPCMBuffer` bindings (`aulo-speech-system`) |
| objc2-foundation | local (macOS) | https://github.com/madsmtm/objc2 | `NSString`, `NSArray`, `NSNumber` for AVFoundation calls; `NSLocale`, `NSOperationQueue` and `NSError` for speech recognition; `NSRunLoop` in the synthesis test (`aulo-speech-system`); `NSDate` and `NSRunLoop` for the main run loop that `aulo bench speech` pumps (`aulo-speech-bench`) |
| objc2-speech | local (macOS) | https://github.com/madsmtm/objc2 | `SFSpeechRecognizer` forced on-device for the macOS system STT (`aulo-speech-system`); `SpeechAnalyzer` is Swift-only |
| opentelemetry | local | https://github.com/open-telemetry/opentelemetry-rust | OTel API for the optional OTLP trace export in `aulo-telemetry` (`otlp` feature) |
| opentelemetry-otlp | local | https://github.com/open-telemetry/opentelemetry-rust | OTLP/HTTP span exporter, `otlp` feature only |
| opentelemetry_sdk | local | https://github.com/open-telemetry/opentelemetry-rust | Tracer provider and batch span processor, `otlp` feature only |
| prost | local | https://github.com/tokio-rs/prost | Protobuf messages for the gRPC API, only in `aulo-proto` |
| prost-types | local | https://github.com/tokio-rs/prost | Well-known protobuf types (`Timestamp`) used by the generated code |
| pulldown-cmark | local | https://github.com/pulldown-cmark/pulldown-cmark | Markdown events for the speakable-text normalizer (`aulo-voice`); no HTML renderer |
| rcgen | local | https://github.com/rustls/rcgen | Self-signed certificate for the `aulo-server` TCP listener, generated on first start (aws-lc-rs) |
| regex | local | https://github.com/rust-lang/regex | Credential and secret-pair patterns in the log redaction |
| reqwest | local | https://github.com/seanmonstar/reqwest | Streaming HTTPS downloads (rustls) of speech models in `aulo-models`; multipart transcription uploads and streamed speech replies in `aulo-speech-cloud` |
| ringbuf | local | https://github.com/agerasev/ringbuf | Fixed-size lock-free ring from the TTS producer (the macOS main-queue callback, the espeak-ng and Windows worker threads) to `poll` (`aulo-speech-system`); spec §5.3 |
| rustix | local | https://github.com/bytecodealliance/rustix | Daemon euid for the local-socket peer check in `aulo-server`, without hand-written `unsafe` |
| rustls | local | https://github.com/rustls/rustls | Certificate and key checks for the `aulo-server` TCP listener; the pinned-fingerprint client verifier for `aulo connect --pin` (aws-lc-rs, the provider reqwest already builds) |
| schemars | local | https://github.com/GREsau/schemars | JSON Schema generated from the config types |
| semver | local | https://github.com/dtolnay/semver | Plugin manifest versions (`aulo-plugin`) |
| serde | local | https://github.com/serde-rs/serde | Serialization of ids, events and config; reply parsing in `aulo-speech-cloud` |
| serde_json | local | https://github.com/serde-rs/json | Config schema output and overrides; round-trip tests; `cargo metadata` parsing in the dependency-graph test; reply and SSE event parsing and speech request bodies in `aulo-speech-cloud`; Kokoro per-text `lang` option in `aulo-speech-sherpa`; `aulo bench speech --json` output in `aulo-speech-bench` |
| sha2 | local | https://github.com/RustCrypto/hashes | SHA-256 of model files, hashed while they download; the API token is held only as its SHA-256 |
| sherpa-onnx | local | https://github.com/k2-fsa/sherpa-onnx | Local speech engines, only in `aulo-speech-sherpa` (Parakeet TDT v3 STT, Kokoro TTS); exact pin `=1.13.8` because `sherpa-onnx-sys` downloads the native lib of its own release |
| sse-core | local | https://github.com/PizzasBear/sse-rs | Zero-I/O server-sent-events parser for streaming transcription replies in `aulo-speech-cloud` (`eventsource-stream` has had no release since 2022) |
| subtle | local | https://github.com/dalek-cryptography/subtle | Constant-time comparison of the API token digest (`aulo-server`) |
| tempfile | local (dev) | https://github.com/Stebalien/tempfile | Temporary databases and directories in tests |
| thiserror | local | https://github.com/dtolnay/thiserror | Typed errors in library crates |
| tokio | local | https://github.com/tokio-rs/tokio | Async runtime; Unix socket, named pipe and TCP listeners in `aulo-server`; SIGINT/SIGTERM shutdown in `aulod`; one-worker runtime that runs the cloud engines under `aulo bench speech` (`aulo-speech-bench`) |
| tokio-rustls | local (dev) | https://github.com/rustls/tokio-rustls | TLS stream under the pinned gRPC client in the `aulo-server` tests |
| tokio-stream | local | https://github.com/tokio-rs/tokio | Listener streams for tonic `serve_with_incoming` in `aulo-server` |
| tokio-tungstenite | local | https://github.com/snapview/tokio-tungstenite | WebSocket client (rustls, platform trust roots) for the ElevenLabs `stream-input` speech API and the Deepgram live transcription API in `aulo-speech-cloud`; also the local WebSocket fixture server in its tests (spec D4) |
| tokio-util | local | https://github.com/tokio-rs/tokio | `CancellationToken` that shuts every `aulo-server` listener down together; `AbortOnDropHandle` that cancels an abandoned transcription or speech request in `aulo-speech-cloud` |
| toml | local | https://github.com/toml-rs/toml | Parses `aulo-plugin.toml` in `aulo-plugin` (the manifest module owns that file); renders and parses the config in `aulo-config` (default file tests, `aulo config show`) |
| tonic | local | https://github.com/hyperium/tonic | gRPC server and client runtime in `aulo-proto` (generated code) and `aulo-server` (transport; rustls TLS on the TCP listener via `tls-aws-lc`) |
| tonic-health | local | https://github.com/hyperium/tonic | `grpc.health.v1` service, only in `aulo-server` |
| tonic-prost | local | https://github.com/hyperium/tonic | Prost codec for tonic |
| tonic-prost-build | local (build) | https://github.com/hyperium/tonic | Generates server and client code from the protos |
| tonic-reflection | local | https://github.com/hyperium/tonic | gRPC server reflection for grpcurl and buf curl, only in `aulo-server` |
| tower | local (dev) | https://github.com/tower-rs/tower | `service_fn` connector for the gRPC client over a Unix socket in `aulo-server` tests |
| tracing | local | https://github.com/tokio-rs/tracing | Structured logs and spans |
| tracing-appender | local | https://github.com/tokio-rs/tracing | Daily-rotating, non-blocking log files with a bounded file count |
| tracing-opentelemetry | local | https://github.com/tokio-rs/tracing-opentelemetry | Bridges tracing spans to OTel, `otlp` feature only |
| tracing-subscriber | local | https://github.com/tokio-rs/tracing | EnvFilter, JSON and human log formatters |
| trycmd | local (dev) | https://github.com/assert-rs/snapbox | Full command-output fixtures for the `aulo` CLI and `aulod` (`crates/aulo/tests/cmd/`, `crates/aulo/tests/aulod/`) |
| ulid | local | https://github.com/dylanhart/ulid-rs | Sortable ids for bots, chats, turns, calls and stored rows |
| url | local | https://github.com/servo/rust-url | Host of a URL for the speakable-text normalizer (`aulo-voice`) |
| windows | local (Windows) | https://github.com/microsoft/windows-rs | WinRT `SpeechSynthesizer` and `DataReader` bindings for the Windows system voices (`aulo-speech-system`); only the `Media_SpeechSynthesis` family of features |
| wiremock | local (dev) | https://github.com/LukeMathWalker/wiremock-rs | Local HTTP fixture server for the `aulo-models` downloader and `aulo-speech-cloud` transcription and speech tests |
