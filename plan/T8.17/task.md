---
status: todo
priority: P1
complexity: 2
readiness: 0%
agent:
---

# T8.17. Latency budget and measurement

Stage: S8 · Area: voice · Depends on: T8.16 · Blocks: 0 task(s)

Budgets: wake to listening under 300 ms; end of speech to first audio under 1.2 s with local engines and a remote LLM; barge-in under 150 ms. Spans in telemetry and a report command.

Done when:

- aulo bench voice prints the budget table
