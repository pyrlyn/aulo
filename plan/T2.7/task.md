# T2.7. aulo-telemetry: logs and traces

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 1 task(s)

tracing-subscriber JSON logs in ~/.aulo/logs with rotation, level from config/env, optional OTLP behind a feature. Secrets are never logged (redaction layer).


Done when:

- a test proves a key-like value is redacted
