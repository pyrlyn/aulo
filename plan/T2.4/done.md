# T2.4. Done

The `:aulo-types` contract module. PR: https://github.com/pyrlyn/aulo/pull/86 (stacked on #85).

- `Ulid` makes ULIDs: a 48-bit millisecond timestamp, then 80 random bits from the stdlib's secure `Uuid.random()`, as 26 Crockford base32 characters. Only the canonical upper-case form is valid.
- `BotId`, `ChatId`, `TurnId`, `CallId` and `ClientId` are value classes. Each rejects anything but a canonical ULID, including when decoded, and serializes as a bare string.
- The sealed `AuloEvent` mirrors the first block of `Event` in `proto/aulo/v1/event.proto` field for field: `VoiceStateChanged`, `SpeechStarted`, `SpeechEnded`, `Transcript`, `TtsChunk`, `ApprovalRequired`, `TakeoverRequested`, `Notice`.
- The JSON discriminator is the proto field name. Agent-loop events stay with T1.15.
- Untrusted text carries the proto's caps:
  - transcript text 64 KiB;
  - tool name 128 B;
  - summary and notice 2 KiB;
  - language 35 B and notice source 128 B, added here.
- Constructors reject longer values, so decoding fails closed. `capped(...)` factories truncate on a UTF-8 character boundary.
- The only dependency is `kotlinx-serialization-core` 1.13.0, the serde equivalent under spec §17 D4. `checkModuleGraph` accepts the module as `CONTRACT`.

How it was checked:

- `./gradlew test ktlintCheck` passes locally on macOS arm64, with 10 aulo-types tests (`UlidTest`, `AuloEventTest`) and 0 failures.
- CI was dispatched on the branch: https://github.com/pyrlyn/aulo/actions/runs/45073239082. It is green on macosArm64, linuxX64, mingwX64 and linuxArm64.
