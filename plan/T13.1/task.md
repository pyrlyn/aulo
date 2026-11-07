---
status: todo
priority: P0
complexity: 2
readiness: 0%
agent:
---

# T13.1. Headless server profile

Stage: S13 · Area: server · Depends on: T3.8, T8.4 · Blocks: 3 task(s)

aulod run --headless --listen 0.0.0.0:7443: TLS required, no local audio devices, voice only through VoiceService streams.

Done when:

- test: headless start refuses plain TCP
