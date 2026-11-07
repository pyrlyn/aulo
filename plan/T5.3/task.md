---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T5.3. Approval flow and grants

Stage: S5 · Area: safety · Depends on: T5.1, T4.8, T2.9 · Blocks: 11 task(s)

ApprovalRequired event; any attached client answers once / always for this chat / always / never; grants are persisted with scope and expiry; no answer before the timeout means deny.

Done when:

- tests: timeout denies; "always" grant is reused next turn
