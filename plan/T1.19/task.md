---
status: in progress
priority: P1
complexity: 3
readiness: 80%
agent: Claude Code / claude-sonnet-5-5
---

# T1.19. The local model runner: stream tokens as they are generated

Stage: S1 · Area: shared · Depends on: none · Blocks: 1 task(s)

Execution plan: tracked in the local model runner as P16.1 (branch runa-p16-aulo-tasks, done there and waiting for a local model runner PR to merge); stream GenEvents from the generation task to the SSE body through a bounded channel, keep the non-streaming path, add a timed test. T1.20 is the local model runner P16.2.

runa serve builds the whole reply before replaying it as SSE (serve.rs generate_events), so time to first token equals generation time. Change it to stream GenEvents as they arrive. Work happens in the local model runner repository; aulo only tracks it.

Done when:

- The local model runner streams the first token before generation ends (test with a timer)
