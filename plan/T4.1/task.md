---
status: todo
priority: P0
complexity: 3
readiness: 0%
agent:
---

# T4.1. aulo-providers: build providers from config

Stage: S4 · Area: providers · Depends on: T1.6, T2.5 · Blocks: 4 task(s)

Assembly crate that builds llm-wire providers from [providers.*]: OpenAI (chat/responses), Anthropic, and OpenAI-compatible sections without per-vendor code (Ollama, LM Studio, vLLM, OpenRouter, DeepSeek, Gemini compat, xAI, the local model runner, custom). Fail open: a broken provider becomes a warning.

Done when:

- tests build every preset from fixtures; a bad section only warns
