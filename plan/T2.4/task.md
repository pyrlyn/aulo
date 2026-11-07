# T2.4. aulo-types: domain contract

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 4 task(s)

Ids (ULID): BotId, ChatId, TurnId, CallId, ClientId. aulo events beyond agent-loop: VoiceState, Transcript (partial/final), SpeechStarted/Ended, TtsChunk, ApprovalRequired, TakeoverRequested, Notice. No I/O dependencies.


Done when:

- types serialize round-trip; deps test passes
