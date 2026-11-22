---
status: in progress
priority: P0
complexity: 2
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T2.3. CI workflows

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

ci.yml calls the shared Kotlin/Gradle CI workflow (Linux x86_64 and aarch64, macOS aarch64, Windows x86_64; no x86_64 macOS), commit-subject lint, pipeline.yml (CodeQL, Semgrep). Linux job installs libasound2-dev.

Execution plan: pyrlyn/ci has no shared Kotlin/Gradle workflow (checked at 9675ff42), so `ci.yml` follows the other Kotlin/Native project: its own jobs on pyrlyn/ci's `config` (docs-only skip) and `macos-runner` actions at the pinned SHA. 1. `build` matrix `<os> (<target>)`: macOS arm64, Linux x86_64, Windows x86_64 each run `./gradlew test ktlintCheck` on their own runner, link and smoke-run both release executables. The Linux x86_64 job also cross-links the linuxArm64 executables and test binary, and a `ubuntu-24.04-arm (linuxArm64)` job runs them natively. No x86_64 macOS. 2. Keep the commit-subject job and pipeline.yml (CodeQL, Semgrep); refresh the comments that still say the build lands later. 3. libasound2-dev and protoc are left to the first tasks that link ALSA or generate gRPC code: nothing builds against them yet. 4. Validate with jactionlint; CI green is checked on the pull request.

Done when:

- CI green on the skeleton
