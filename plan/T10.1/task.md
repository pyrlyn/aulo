---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T10.1. MCP client

Stage: S10 · Area: mcp · Depends on: T1.12, T4.7, T5.1 · Blocks: 5 task(s)

mcp-host in aulod: [mcp.servers] in config plus .mcp.json import; stdio and streamable HTTP; tools exposed as mcp__server__tool (deferred by default) and checked by policy; a broken server is a notice.

Done when:

- fixture MCP server test: tool call goes through policy
