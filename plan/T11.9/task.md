---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T11.9. Hooks

Stage: S11 · Area: plugins · Depends on: T1.17, T8.1 · Blocks: 0 task(s)

Shell hooks with the Claude Code JSON protocol for UserPromptSubmit, PreToolUse, PostToolUse, Heard (final transcript), BeforeSpeak, SessionStart/End. Exit 2 blocks; a failing hook warns.

Done when:

- tests: blocking hook stops a tool; failing hook only warns
