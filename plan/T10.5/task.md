---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T10.5. aulo as an MCP server (stdio)

Stage: S10 · Area: mcp · Depends on: T8.1, T5.3 · Blocks: 2 task(s)

aulo mcp serves aulo's abilities to other agents (Claude Code, Cursor, Codex): speak(text, voice), listen(timeout), ask_user(question) by voice, notify, plus computer tools. Every call goes through aulo policy and approvals, the caller cannot bypass them. Built on rmcp, following the token-reduction CLI's MCP server.

Done when:

- rmcp client test calls speak and gets a policy decision for shell
