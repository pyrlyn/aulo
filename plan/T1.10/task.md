---
status: todo
priority: P0
complexity: 2
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.10. Extract shell-classify: bash risk classifier

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Execution plan: in a shared Kotlin modules worktree branched from s1-setup, create shell-classify (publish = false, dual licence) from cox-tools/src/bash/classify.rs with its fixture tests; module-level project files like change-preview; verify ./gradlew ktlintCheck test. cox adoption waits for publication.

Move the tree-sitter bash classifier from cox-tools/src/bash/classify.rs into the shared Kotlin modules/shell-classify (ReadOnly, Write, Exec, Destructive).

Done when:

- classifier fixture tests pass; the coding agent adopts it
