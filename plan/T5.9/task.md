---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T5.9. Tamper-evident audit log

Stage: S5 · Area: safety · Depends on: T2.9, T5.3 · Blocks: 2 task(s)

Append-only, hash-chained rows for every tool call, approval, egress and plugin load. aulo audit verify and aulo audit export.

Done when:

- test: editing a row breaks verification
