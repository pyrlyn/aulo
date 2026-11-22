---
status: todo
priority: P1
complexity: 2
readiness: 50%
agent: Claude Code / claude-sonnet-5-5
---

# T1.11. Extract text-sanitize: terminal and bidi guard

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Execution plan: in a shared Kotlin modules worktree branched from s1-setup, create text-sanitize (publish = false, dual licence so ketch can adopt it) merging cox-sanitize, rtok src/sanitize.rs and ketch changelog::sanitize, with the union of their tests; module-level project files like change-preview; verify ./gradlew ktlintCheck test. Adoption by cox, rtok and ketch and the module layering docs duplicate note wait for publication and creator permission.

Merge cox-sanitize, the token-reduction CLI src/sanitize.rs and ketch changelog::sanitize (a known duplicate in the module layering docs) into the shared Kotlin modules/text-sanitize. All three projects adopt it.

Done when:

- one module, three adopters, the module layering docs duplicate note removed
