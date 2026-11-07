# T1.7. llm-router: one provider per tier

Stage: S1 · Area: shared · Depends on: T1.6 · Blocks: 1 task(s)

cox-session builds one provider per session from `tiers.code.provider`. `llm-router` holds one provider per tier and routes a job to that tier, so the voice front, the main agent and the sentinel can use different providers.

Done when:

- a test routes two tiers to two different providers
