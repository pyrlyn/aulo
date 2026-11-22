---
status: todo
priority: P0
complexity: 3
readiness: 60%
agent: Claude Code / claude-sonnet-5-5
---

# T1.9. Extract proc-sandbox: Seatbelt, bwrap, Landlock+seccomp

Stage: S1 · Area: shared · Depends on: T0.1 · Blocks: 1 task(s)

Execution plan: in a shared Kotlin modules worktree branched from s1-setup, create proc-sandbox (publish = false, dual licence) from cox-sandbox with its tests (macOS and Linux paths cfg-gated); module-level project files like change-preview; verify ./gradlew ktlintCheck test on macOS. cox adoption waits for publication.

Move cox-sandbox (SandboxMode, command wrapping, argv for PTYs, path::confine) into the shared Kotlin modules/proc-sandbox. The coding agent adopts it.

Done when:

- sandbox tests pass on macOS and Linux CI runners
