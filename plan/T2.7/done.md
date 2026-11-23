# T2.7. Done

The `:aulo-telemetry` module. PR: https://github.com/pyrlyn/aulo/pull/91, stacked on #87.

- `Redact`:
  - the masking rules of the earlier Rust `telemetry-setup`: bearer headers, provider and forge keys, Slack, AWS and Google keys, JWTs, and secret-named `name=value` pairs;
  - plus the aulod token shape;
  - `scrubJson` replaces the whole value of a secret-named key, whatever its type.
  - It fails closed: if the rules do not compile, the whole value is masked.
- `Logger` writes one JSON object per line: `ts`, `level`, `target`, `message`, `fields`.
  - The whole record is masked before the sink sees it.
  - A sink failure is counted in `dropped`, never thrown.
- `RotatingFileSink` writes `<home>/logs/aulo.log` through `pyrlyn.kit:fs`.
  - The default size cap is 10 MiB, with 5 rotated files kept.
  - Each line is flushed. Writers are serialized by a spin lock.
- `[log] level` in `:aulo-config` (default `info`, env `AULO_LOG__LEVEL`). The schema is regenerated.
- Not done, by the creator's choice: OTLP export. There is no new dependency.
- Wiring the logger into `aulod` waits for the daemon (S3).

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph` passes on macOS arm64: 14 telemetry tests and 14 config tests.
  - `LoggerTest.aKeyLikeValueNeverReachesTheSink` passes a provider key in the message, a secret-named field and a URL query. It checks that the key is absent from the written line.
  - Rotation tests cover the size cap, the number of files kept, and a line longer than the cap.
- The linuxArm64 test binaries passed in Ubuntu 24.04 on Colima (14 and 14 tests).
- The mingwX64 test binary cross-links, but has not been run.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45076446743. The org variable `SKIP_CI_TESTS=true` skipped tests there.
