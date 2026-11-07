# T1.2. Extract llm-http: credentials, retries, SSE framing

Stage: S1 · Area: shared · Depends on: T1.1 · Blocks: 2 task(s)

Move resolve_key (env var, then keyring), retry policy and SSE framing from cox-provider-http into packages/crates/llm-http.

Done when:

- llm-http tests cover env-first, keyring fallback and retry backoff
