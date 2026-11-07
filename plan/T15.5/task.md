---
status: in progress
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-sonnet-5-5
---

# T15.5. API reference

Stage: S15 · Area: release · Depends on: T3.1 · Blocks: 0 task(s)

Execution plan: 1. Generate a Markdown reference for proto/aulo/v1 with buf and protoc-gen-doc into docs/api/. 2. A justfile recipe to regenerate it. 3. The proto CI workflow regenerates it and fails on drift. 4. Verify locally with buf.

Generated reference for the gRPC API from the protos (buf / protoc-gen-doc).

Done when:

- reference regenerates in CI
