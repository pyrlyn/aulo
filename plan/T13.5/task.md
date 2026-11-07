---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T13.5. Container image (agent computer)

Stage: S13 · Area: server · Depends on: T13.1, T9.7 · Blocks: 0 task(s)

OCI image with aulod, headless Chromium and a virtual display, so computer control runs inside a container like the Grok Bot agent computer; compose file with a volume for the home.

Done when:

- image builds in CI; smoke test runs a browser tool
