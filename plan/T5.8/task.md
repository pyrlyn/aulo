---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T5.8. Sentinel reviewer for consequential actions

Stage: S5 · Area: safety · Depends on: T5.3, T4.5 · Blocks: 0 task(s)

A separate model call on the sentinel tier reviews consequential actions (send, buy, delete, publish, credential use) with the action and a context summary and returns allow or escalate. It can only make a decision stricter, never allow what policy denies. Rationale goes to the audit log.

Done when:

- tests: sentinel escalate forces Ask; sentinel allow cannot override Deny
