---
status: in progress
priority: P0
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T2.7. aulo-telemetry: logs and traces

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 1 task(s)

tracing-subscriber JSON logs in ~/.aulo/logs with rotation, level from config/env, optional OTLP behind a feature. Secrets are never logged (redaction layer).

Done when:

- a test proves a key-like value is redacted

Execution plan (Kotlin/Native; the creator chose a small own logger, no new dependency, OTLP later):

1. `:aulo-telemetry` (LIBRARY):
   - `Redact`: the masking rules of the earlier Rust `telemetry-setup`, plus the aulod token shape. It fails closed: if the rules do not compile, the whole value is masked.
   - `LogLevel`.
   - `Logger`: writes one JSON object per line, masked structurally before it is written.
   - `RotatingFileSink`: `<home>/logs/aulo.log`, rotated by size through kit:fs and keeping a fixed number of files.
2. `:aulo-config` gains `[log] level`. Env works through the existing layers (`AULO_LOG__LEVEL`). The schema is regenerated.
3. Tests:
   - masking rules: key-like values, secret-named fields, idempotence;
   - logger: level filter; a key-like value and a secret field are redacted in the written line;
   - rotation: size cap and number of files kept.
4. Verify: `./gradlew test ktlintCheck` locally on macOS, linuxArm64 test binaries on Colima, CI build and link.

T2.7 runs before T2.6 because `aulo config show` needs these masking rules.
