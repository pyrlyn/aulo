# T3.6 done

PR: https://github.com/pyrlyn/aulo/pull/105 (stacked on #104), branch `t3.6-aulod`.

## What was done

- `aulod run [--config FILE] [--listen ADDR]` (`aulo/app/daemon`). `--config` replaces
  `<home>/config.toml` and must exist (`LoadOptions.configFile`); the home itself still comes
  from `AULO_HOME` or the user directory. `--listen` overrides `daemon.listen` as the highest
  layer, so TLS is still required on TCP. Every operator error is `error: …` with exit 1.
- `runDaemon` (aulo-server): listeners, the owner-only run directory, a kotlin-kit `FileLock`
  on `run/aulod.lock`, the pidfile `run/aulod.pid`, the store, the first token, `AuloServer`
  with `ChatService`; graceful shutdown, pidfile removed, lock released. A second aulod for the
  same home fails with `aulod is already running for <home> (pid N)` before touching anything.
- Shutdown: on Unix SIGINT and SIGTERM go through a self-pipe in a cinterop C shim
  (`aulo/src/nativeInterop/cinterop/signals.def`). `pthread_sigmask` + `sigwait` was tried
  first and failed in the end-to-end test: the Kotlin/Native runtime starts threads before
  `main`, so SIGTERM killed the process through one of them. Windows uses
  `SetConsoleCtrlHandler`.

## How it was checked

- Command fixtures (`aulo/tests/cmd/aulod_*`): no arguments, `run --help`, `run` start and
  stop, a second instance, a missing `--config`, `--listen` without TLS.
- `DaemonTest`: `CreateChat` over the socket through `runDaemon`, pidfile contents, second
  instance refused, cleanup after stop, pidfile parsing caps, listener errors before the home
  is touched.
- `AulodProcessTest` (Unix): the linked `aulod` refuses a second process, and SIGTERM and
  SIGINT each stop it with exit 0 and a removed pidfile.
- macOS: aulo 7, aulo-config 23, aulo-server 31 tests pass; the same suites pass on linuxX64
  and linuxArm64 on Colima; mingwX64 `aulo` and `aulod` link; `ktlintCheck` and
  `checkModuleGraph` pass.
- CI run 45168241229: green on every target.
