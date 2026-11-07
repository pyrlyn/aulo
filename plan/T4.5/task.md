---
status: todo
priority: P1
complexity: 3
readiness: 0%
agent:
---

# T4.5. Tiers and job routing

Stage: S4 · Area: providers · Depends on: T4.4, T1.7 · Blocks: 2 task(s)

Tiers: voice (fast, short answers), main (tools), sentinel (reviewer), cheap (titles, summaries). Each tier may use a different provider (llm-router).

Done when:

- test routes main and cheap to different providers
