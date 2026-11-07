---
status: todo
priority: P0
complexity: 2
readiness: 0%
agent:
---

# T5.6. Per-app and per-site grants with a default blocklist

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 2 task(s)

Grants for App and Site subjects (allow once / always / never). Built-in category blocklist that always asks or denies: banking, payments, crypto, password managers, system settings.

Done when:

- test: a banking site always asks even with a broad allow rule
