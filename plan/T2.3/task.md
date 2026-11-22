---
status: todo
priority: P0
complexity: 2
readiness: 0%
agent: Claude Code / claude-sonnet-5-5
---

# T2.3. CI workflows

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

ci.yml calls the shared Kotlin/Gradle CI workflow (Linux x86_64 and aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), commit-subject lint, pipeline.yml (CodeQL, Semgrep). Linux job installs libasound2-dev.

Execution plan: 1. .github/workflows/ci.yml calling the shared Kotlin/Gradle CI workflow at the same pinned SHA style the coding agent uses (targets Linux x86_64/aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), with protoc and libasound2-dev installed where the reusable workflow allows. 2. Commit-subject lint and pipeline.yml (CodeQL, Semgrep) following the coding agent's workflows. 3. Validate locally with jactionlint. 4. CI green can only be checked after a push, which needs the creator's permission; until then readiness stays below 100% and the card notes it.

Done when:

- CI green on the skeleton
