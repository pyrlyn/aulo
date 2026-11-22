---
status: in progress
priority: P1
complexity: 3
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T2.2. Dependency-graph test

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

A test reads the Gradle project metadata and enforces module roles: contracts have no I/O deps, only surfaces depend on the CLI and error libraries, one owner module per heavy dependency (SQLite, the gRPC stack, sherpa-onnx, a WASM runtime).

Done when:

- the test fails when a contract module pulls I/O or HTTP client libraries

Execution plan: 1. `build-logic`: every module declares a role (`contract`, `library`, `surface`) through the `aulo` extension of the `aulo.kmp` plugin and writes a manifest of its declared dependencies and cinterop libraries. 2. A root `checkModuleGraph` task (part of `./gradlew test`) reads the manifests and applies the rules from one pure Kotlin function: contracts have no I/O, HTTP, storage or native dependency and depend only on contracts; only surfaces use the CLI libraries and nothing depends on a surface; each heavy dependency (SQLite, the gRPC stack, sherpa-onnx, a WASM runtime) has one owner module. 3. Unit tests in `build-logic` for each rule, including a contract that pulls an HTTP client. 4. Verify with `./gradlew test ktlintCheck`, then break a rule on purpose and check the build fails.
