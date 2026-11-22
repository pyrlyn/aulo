---
status: todo
priority: P0
complexity: 4
readiness: 80%
agent: Claude Code / claude-opus-5-5
---

# T1.1. Extract llm-wire: provider contract and neutral types

Stage: S1 · Area: shared · Depends on: T0.1, T0.7 · Blocks: 3 task(s)

Execution plan: in a shared Kotlin modules worktree branched from s1-setup, create llm-wire (publish = false, dual licence) with the provider trait and neutral types copied from cox-protocol plus their tests; Replay/Scripted fakes behind `test-util`; module-level AGENTS.md, README.md, plan.md, todo.md, done.md, roadmap.md, ideas.md, toolchain.md like change-preview; verify ./gradlew ktlintCheck test in the shared Kotlin modules. cox re-export waits until the module is published (modules.io or a git dependency), because a path dependency breaks cox CI.

Move the provider trait and the neutral types (Request, ProviderEvent, Caps, ToolSpec, Usage, Risk) out of cox-protocol into the shared Kotlin modules/llm-wire. No behaviour change. Keep the Replay/Scripted fakes behind a test-util feature.

Done when:

- llm-wire builds and its tests pass in the shared Kotlin modules
- cox-protocol re-exports the moved types (no API break for the coding agent)

