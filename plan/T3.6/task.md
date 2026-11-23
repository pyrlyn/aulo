---
status: in progress
priority: P0
complexity: 3
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T3.6. aulod binary

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 2 task(s)

CLI surface: aulod run, --config, --listen; single-instance lock per home; pidfile; SIGINT/SIGTERM shutdown.

Done when:

- second instance exits with a clear message; trycmd fixtures

## Execution plan

Reading of the flags: `--config <FILE>` replaces `<home>/config.toml` and must exist; the home
(lock, socket, database, secrets) still comes from `AULO_HOME` or the user directory.
`--listen <ADDR>` overrides `daemon.listen` as the highest configuration layer.

1. `aulo-config`: `LoadOptions.configFile`; a missing named file is a `ConfigException`.
2. `aulo-server/Daemon.kt`: `runDaemon(home, config, secrets, log, stop)` — listeners first, then
   the owner-only run directory, `kotlin-kit` `FileLock` on `run/aulod.lock` (a second instance
   fails with `AlreadyRunningException`, naming the pid from `run/aulod.pid`), the pidfile, the
   store, the first token, `AuloServer` with `ChatService`; graceful shutdown, pidfile removed,
   lock released.
3. `aulo/app/daemon`: the `aulod` clap tree (`run`, `--config`, `--listen`), `runDaemonCli` that
   maps every operator error to `error: …` and exit 1; `ShutdownSignals` — Unix blocks
   SIGINT/SIGTERM and takes them with `sigwait` on a thread of its own, Windows uses
   `SetConsoleCtrlHandler`; `main` wires them with the system secret store.
4. Tests: `aulod` command fixtures in `aulo/tests/cmd` (help, version, a missing `--config`,
   `--listen` on TCP without TLS, a second instance while the lock is held); an in-process
   lifecycle test (health and `CreateChat` over the socket, pidfile written and removed, lock
   released); a Unix test that sends SIGTERM to the process.
5. Verify: macOS tests and ktlint, linuxX64 and linuxArm64 test binaries on Colima, mingwX64
   link, `checkModuleGraph`; CI on the PR.
