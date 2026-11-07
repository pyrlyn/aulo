---
status: in progress
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T2.3. CI workflows

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

ci.yml calls pyrlyn/ci ci-rust.yml (Linux x86_64 and aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), commit-subject lint, pipeline.yml (CodeQL, Semgrep). Linux job installs libasound2-dev.

Execution plan: 1. .github/workflows/ci.yml calling pyrlyn/ci ci-rust.yml at the same pinned SHA style the coding agent uses (targets Linux x86_64/aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), with protoc and libasound2-dev installed where the reusable workflow allows. 2. Commit-subject lint and pipeline.yml (CodeQL, Semgrep) following the coding agent's workflows. 3. Validate locally with actionlint (and zizmor if rust.md/cox uses it). 4. CI green can only be checked after a push, which needs the creator's permission; until then readiness stays below 100% and the card notes it.
Status note: ci.yml (pyrlyn/ci ci-rust.yml at f880dee, shared matrix without Intel macOS; setup-command installs libasound2-dev and protoc per OS), a bash Conventional Commits check on pull requests, pipeline.yml and .github/infra.yml (CodeQL actions/python/rust, Semgrep) are on main and pass actionlint. Waiting for a push to see CI green; apt protoc is 3.21 while local is 36.2.

Done when:

- CI green on the skeleton
