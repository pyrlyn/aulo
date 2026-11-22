---
status: in progress
priority: P0
complexity: 2
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T2.4. aulo-types: domain contract

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 4 task(s)

Ids (ULID): BotId, ChatId, TurnId, CallId, ClientId. aulo events beyond agent-loop: VoiceState, Transcript (partial/final), SpeechStarted/Ended, TtsChunk, ApprovalRequired, TakeoverRequested, Notice. No I/O dependencies.

Done when:

- types serialize round-trip; deps test passes

Execution plan: 1. New `:aulo-types` module, role `CONTRACT`, with the kotlinx-serialization plugin and `kotlinx-serialization-core` (the obvious equivalent of serde, spec §17 D4). 2. `Ulid` (Crockford base32, 48-bit ms timestamp, 80 random bits from the stdlib's secure `Uuid.random()`) and value-class ids `BotId`, `ChatId`, `TurnId`, `CallId`, `ClientId` that reject anything but a canonical ULID. 3. A sealed `AuloEvent` mirroring the voice and safety events of `proto/aulo/v1/event.proto`: `VoiceStateChanged`, `SpeechStarted`, `SpeechEnded`, `Transcript`, `TtsChunk`, `ApprovalRequired`, `TakeoverRequested`, `Notice` (untrusted text capped at 2 KiB). The agent-loop events (turns, text, tool calls) come from `agent-loop` (T1.15) and are not duplicated here. 4. Tests: JSON round trip of every event and id, ULID format and ordering, invalid ids rejected, the Notice cap; `checkModuleGraph` passes with the new contract. Verify with `./gradlew test ktlintCheck`.
