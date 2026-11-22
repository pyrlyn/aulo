---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T12.2. aulo-ffi: C ABI exports and bindings

Stage: S12 · Area: desktop · Depends on: T12.1 · Blocks: 2 task(s)

Forward-only C ABI exports over aulo-app (cox-ffi rule: every exported body is one expression, logic stays in aulo-app, enforced by a source test). Swift and C# bindings are generated from that ABI. Host callbacks for notifications, URLs and secrets. Records fixtures for the app tests.

Done when:

- forward-only test passes; Swift and C# bindings generate in CI
