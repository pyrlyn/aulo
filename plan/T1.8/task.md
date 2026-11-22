---
status: todo
priority: P0
complexity: 3
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.8. Extract perm-rules: permission rule engine

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Execution plan: in a shared Kotlin modules worktree branched from s1-setup, create perm-rules (publish = false, dual licence) from cox-permission with a neutral, extensible subject type and its rule-grammar and decision-order tests; module-level project files like change-preview; verify ./gradlew ktlintCheck test. cox adoption waits for publication (a path dependency breaks cox CI).

Move cox-permission (rule grammar such as Bash(git commit:*), Edit(src/**), mcp__srv__*, decision order, risk fallback) into the shared Kotlin modules/perm-rules with neutral subject types so aulo can add App(...) and Site(...) subjects. The coding agent adopts it.

Done when:

- rule-grammar and decision-order tests pass in both repos
