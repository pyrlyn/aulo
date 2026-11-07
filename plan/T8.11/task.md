---
status: todo
priority: P1
complexity: 4
readiness: 0%
agent:
---

# T8.11. Two-brain mode: realtime front consults the agent

Stage: S8 · Area: voice · Depends on: T8.10, T8.1 · Blocks: 2 task(s)

A realtime speech-to-speech model talks with the user; its only tool is consult_agent, which submits to the main agent where policy applies. The front never calls computer tools itself (OpenClaw agent-consult pattern).

Done when:

- test: a realtime tool call other than consult_agent is refused
