---
status: todo
priority: P1
complexity: 4
readiness: 0%
agent:
---

# T5.10. Secrets vault the model cannot read

Stage: S5 · Area: safety · Depends on: T4.2, T5.1 · Blocks: 0 task(s)

Credentials live in the keychain behind handles. Tools take a handle (type_secret(handle), http auth by handle) and inject the value; the model, transcripts and logs only see the handle.

Done when:

- test: transcript and log contain the handle, never the value
