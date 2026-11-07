---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T4.8. Converse RPC and chat persistence

Stage: S4 · Area: agent · Depends on: T4.6, T3.5 · Blocks: 11 task(s)

SessionService.Converse drives aulo-agent; every turn, tool call and usage row is persisted; a chat resumes after a daemon restart.

Done when:

- test restarts the daemon and continues the chat with full history
