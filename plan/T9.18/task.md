---
status: todo
priority: P1
complexity: 2
readiness: 0%
agent:
---

# T9.18. Control ladder and computer tool facade

Stage: S9 · Area: control · Depends on: T9.1, T9.6, T9.12, T9.16 · Blocks: 2 task(s)

System prompt and tool descriptions order the options: MCP or API first, then shell/app, then browser snapshot, then desktop AX, and screenshots with input last.

Done when:

- eval fixture: the agent picks the browser snapshot over pixel clicks
