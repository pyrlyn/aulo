# T1.3. Extract llm-openai: Chat and Responses wires

Stage: S1 · Area: shared · Depends on: T1.2 · Blocks: 2 task(s)

Move the OpenAI Chat and Responses wires from cox-provider-openai into packages/crates/llm-openai. They also serve every OpenAI-compatible endpoint (Ollama, LM Studio, vLLM, OpenRouter, DeepSeek, Gemini compat, xAI, the local model runner).

Done when:

- streaming text and tool-call tests pass against recorded fixtures
