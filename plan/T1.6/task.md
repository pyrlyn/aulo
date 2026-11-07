---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T1.6. Switch the coding agent to the extracted llm crates

Stage: S1 · Area: shared · Depends on: T1.3, T1.4, T1.5 · Blocks: 2 task(s)

The coding agent depends on llm-wire, llm-http, llm-openai, llm-anthropic and llm-catalog; the old code is deleted. Update the coding agent tests/deps.rs, the coding agent toolchain.md and rust.md.

Done when:

- The coding agent CI green with no duplicated provider code left
