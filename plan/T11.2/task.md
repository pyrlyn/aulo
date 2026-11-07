---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T11.2. Plugin host: discovery and grants

Stage: S11 · Area: plugins · Depends on: T11.1, T5.3 · Blocks: 4 task(s)

Discovery in ~/.aulo/plugins and project .aulo/plugins; grants bound to the package digest; a changed digest or new capability needs approval; a broken plugin is skipped with a warning.

Done when:

- tests: tampered plugin needs re-approval; broken plugin only warns
