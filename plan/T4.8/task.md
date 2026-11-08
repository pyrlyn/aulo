---
status: in progress
priority: P0
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T4.8. Converse RPC and chat persistence

Stage: S4 · Area: agent · Depends on: T4.6, T3.5 · Blocks: 11 task(s)

SessionService.Converse drives aulo-agent; every turn, tool call and usage row is persisted; a chat resumes after a daemon restart.

Done when:

- test restarts the daemon and continues the chat with full history

## Execution plan

1. `aulo-store`: a migration adds a nullable `messages.wire` column (the
   `llm_wire::Message` JSON, so tool use and results survive a restart).
   `append_turn` writes a turn's messages and their tool calls in one
   transaction; `chat_history` reads every message of a chat.
2. `aulo-server::session`: `SessionApi` implements `SessionService.Converse`.
   - One live `aulo_agent::Session` per chat, built on first Attach from the
     stored history; every attached client gets every event (broadcast).
   - Model: the chat's stored `provider/model`, else `providers.tiers.main`.
     `SwitchModel` stores the new model and takes effect at the next turn.
   - Approvals: `PolicyGate` over `config.policy`; tools: the T4.7 registry
     (empty until tool backends land), so an unknown call fails closed.
   - Persistence: usage rows on every provider call; the turn's new history
     (messages plus tool calls derived from `ToolUse`/`ToolResult` pairs) is
     written before `TurnFinished` reaches clients.
   - `Interrupt{all}` interrupts every live chat (kill switch); scopes stay
     T13.3. `TakeoverDone` answers with a notice until T5.11.
3. `aulod`: mount `SessionApi` in `serve.rs` on the shared store.
4. `aulo-testkit`: `TestDaemon` mounts the real session with scripted
   endpoints and can restart on the same home.
5. Verify: a testkit test runs a turn, restarts the daemon, continues the
   chat and asserts the provider saw the full history (text and tool call);
   unit tests for the history mapping; `cargo test --workspace`, clippy
   `-D warnings`, `fmt --check`, `buf lint`.
