---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T13.3. API tokens with scopes

Stage: S13 · Area: server · Depends on: T13.1 · Blocks: 1 task(s)

aulo token create/ls/revoke with scopes (read, chat, voice, control, admin); stored hashed; checked per RPC.

Done when:

- tests: a read token cannot submit a turn
