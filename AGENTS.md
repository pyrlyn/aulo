# aulo — instructions for agents

**What this is.** A local-first voice agent that controls the computer: speech in and out with switchable engines, shell/app/browser/desktop control, MCP client and server, plugins, local and remote models. One daemon (`aulod`) serves a gRPC API to the native desktop apps (SwiftUI, WinUI), the CLI and remote clients.

The implementation stack is **Kotlin/Native** (Gradle KMP, native targets). The project has
been reset: every task in `plan/` is open.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `spec.md` — the specification. Section numbers are stable; tasks cite them.
- `research.md` — sources for every external fact and the reuse map of the workspace (Part 6).
- `plan/` — one folder per task (`plan/<id>/task.md`, closed ones also `done.md`); `plan/README.md` says how to read it. Settled decisions D1–D11 are recorded in `spec.md` §17. Never decide an open question in `spec.md` §17 yourself; ask the creator.

## Hard rules

- **Policy runs outside the model.** Every tool call — built-in, MCP, plugin or a call made through aulo's own MCP server — passes `aulo-policy`. No tool runs before policy returns Allow. Policy fails closed.
- **Voice never widens access.** Destructive, payment and credential actions need a click or key, never only a spoken yes.
- **Secrets never reach the model.** Tools take secret handles; values never appear in prompts, transcripts or logs.
- **Everything from a model, a tool, a web page, a screenshot, an MCP server, a plugin or a remote client is untrusted input.** Wrap it, sanitize it, cap it, and test the cap.
- **Fail open on extensions.** A broken STT/TTS engine, MCP server, plugin, skill or hook is warned about and skipped; voice falls back to the next engine.
- **No unbounded buffers on the audio path.** Audio callbacks never block or allocate; ring buffers are fixed size and overflow is counted.
- **Reuse before writing.** Provider stack, permissions, sandbox, MCP host, speech capture, config helpers and the agent loop come from the shared Kotlin modules planned in S1. Do not copy code from the coding agent, the local model runner or the token-reduction CLI into aulo.
- **gRPC is the API.** Protos in `proto/aulo/v1`; `buf lint` and `buf breaking` must pass.
- **Licences.** Check the licence of every engine and model before wiring it. GPL code (for example Piper) only runs as a separate plugin process. The one exception is espeak-ng, linked by sherpa-onnx (spec §17 D9).

## Layout

- Gradle KMP modules (`aulo-<role>`), created by S2+ tasks, hold the Kotlin/Native daemon and CLI. Every module applies the `aulo.kmp` convention plugin from `build-logic/` (targets per spec §17 D3, warnings as errors, ktlint). `:aulo` is the surface module that links the `aulo` and `aulod` executables.
- Shared Kotlin/Native libraries come from `kotlin-kit` in the workspace's `packages/` (pyrlyn/packages), included as a Gradle build: `pyrlyn.kit:<module>` (TOML, files, SQLite, processes, HTTP). Gradle finds it in the first ancestor `packages/` or through `KOTLIN_KIT_DIR` and `SHARED_SETTINGS_DIR`; CI checks it out at a pinned commit. Check it before writing a helper.
- `:aulo-config` is the only module that reads or writes aulo's TOML config. `schemas/aulo.schema.json` is generated from its types; after changing them run `just schema` (the stale-schema test fails otherwise).
- Logs go through `:aulo-telemetry`'s `Logger`, which masks every record with `Redact` before writing. Anything else that shows configured or untrusted text to a person masks it with `Redact` too; do not add a second rule set.
- `:aulo-store` owns SQLite (`aulo.db`). Callers use `Store`'s typed methods; SQL lives only in its `.sq` files (SQLDelight) and in `aulo-store/migrations/<key>/{up,down}.sql`. A schema change is a new migration directory (key `YYYY-MM-DD-HHMMSS_name`) plus the same change in the `.sq` files; `MigrationsTest` fails if the two disagree. Never edit a migration that has reached `main`.
- `aulo` commands are tested in-process from fixtures in `aulo/tests/cmd` (see its README); after an intended output change run `AULO_UPDATE_FIXTURES=1 ./gradlew :aulo:allTests` and review the diff.
- Every module declares its role in its build file, `aulo { role = ModuleRole.X }`: `CONTRACT` (types only: no I/O, HTTP, storage or native library; depends only on contracts), `LIBRARY`, or `SURFACE` (links an executable; the only place for the CLI libraries; nothing depends on it). SQLite, the gRPC stack, sherpa-onnx and a WASM runtime each have one owner module. `./gradlew test` runs `checkModuleGraph`, which enforces this (T2.2); the library lists live in `build-logic/src/main/kotlin/aulo/buildlogic/ModuleGraph.kt` — extend them when a task wires a new library of one of those kinds.
- `desktop/macos` (SwiftUI) and `desktop/windows` (WinUI 3) — native apps over `aulo-ffi` (T12.3, T12.15).
- `proto/aulo/v1/` — gRPC service definitions (T3.1).
- `:aulo-proto` is that API as Kotlin: Wire generates the messages and one `<Service>Server` interface per service into `build/generated` on every build (spec §17 D10). Nothing generated is committed; change the `.proto` files, never the output. It holds no transport.
- `:aulo-grpc` owns the gRPC stack: gRPC over HTTP/2 on nghttp2 for every target (spec §17 D11). It knows no aulo service.
- `:aulo-server` binds services to it. Kotlin/Native cannot reflect, so `GrpcTables` in `build-logic` (a Wire schema handler) writes `grpcHandlers()` for every `<Service>Server` interface and the file descriptors that server reflection serves; pass `service.grpcHandlers()` to `AuloServer`. Health and reflection are built in. `grpc.health.v1` and `grpc.reflection.v1` are vendored in `aulo-server/src/proto/`; never edit them. With `AULO_GRPCURL` naming a grpcurl binary, the server tests also check the server with grpcurl.
- C libraries come only through Conan. A module applies the `aulo.conan` plugin, lists them in its `conanfile.txt`, pins their recipe revisions in its `conan.lock`, and binds each one with `conan { cinterop(...) }` and a `.def` file. The profiles in `conan/` take Conan Center's binaries on macOS and build Linux and Windows from source with Kotlin/Native's own clang and sysroots. After changing a `conanfile.txt`, recreate its lock: `conan lock create <module>/conanfile.txt --lockfile-out=<module>/conan.lock -pr:h=conan/macosArm64.profile -pr:b=conan/build.profile`. A package whose requirements differ per platform makes the Linux or Windows install fail on the lock; then extend the lock with those profiles too.
- `plugins/` — plugin SDK examples (T11.5).
- `docs/` — guides, threat model, spike reports (`docs/spikes/`). Historical spike reports may describe the retired Rust workspace; new work follows this file and `spec.md`.
- `scripts/sync-issues.py` — creates GitHub issues from the active tasks in `plan/` (dry run by default).

## Commands

jactionlint and hk are pinned in `mise.toml`. `.github/jactionlint.yaml` applies the default profile and the
baseline `.github/jactionlint-baseline.json`. `hk.pkl` runs jactionlint as a pre-push hook:

```bash
mise install                               # JDK 21 and the lint tools
mise x -- ./gradlew test                   # host-target tests (just test)
mise x -- ./gradlew ktlintCheck            # style (just lint)
mise x -- jactionlint                      # lint workflows, composite actions, dependabot.yml
mise x -- jactionlint --baseline-write     # refresh the baseline after fixing findings
mise x -- hk install --mise                # install the pre-push hook once per clone
just api-docs                              # regenerate docs/api from proto/
```
