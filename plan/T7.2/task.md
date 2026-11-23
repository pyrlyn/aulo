---
status: in progress
priority: P0
complexity: 3
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T7.2. Engine registry and runtime switching

Stage: S7 · Area: speech · Depends on: T7.1, T2.5 · Blocks: 5 task(s)

Engines register by id (built-in and plugin-provided). [voice.stt] and [voice.tts] select engine, model and voice; a chat or bot can override; switching through gRPC, CLI (aulo voice use) or voice command takes effect on the next utterance without restart. A failing engine is skipped for the next one in the fallback list (fail open).

Done when:

- test switches TTS mid-chat; a failing engine falls back with a notice

## Execution plan

Port the design of the retired Rust T7.2 (commit 0508ff9). The gRPC `SetVoice`, `aulo voice use` and voice commands are their own tasks; they all call `switch`.

1. `:aulo-speech`: `EngineSpec`, the resolved choice a factory builds from, and `EngineChoice`, one layer of choice with unset fields deferring to the layer below. Model and voice must be bounded labels; the fallback list is capped.
2. `:aulo-testkit` (spec §4.2): the fake engines move here from the T7.1 contract tests, so `:aulo-speech` and `:aulo-voice` tests share them. Only test source sets depend on it.
3. `:aulo-voice` (domain library):
   - `EngineRegistry`: factories by id; a duplicate id is refused, so a plugin cannot take over a built-in engine; an unknown id is Unavailable, so it falls back.
   - `candidates(config, bot, chat)`: the ordered engines to try, chat first, then bot, then config, then the fallback list.
   - `ActiveStt` / `ActiveTts`: build lazily; `switch` takes effect at the next begin; an engine fault or a failed build is skipped with a WARN `AuloEvent.Notice`; caller bugs keep the engine. Dropped engines are closed, and a throwing `close` is a notice too.
4. `:aulo-config`: `EngineConfig.choice(key, provenance)` maps contract validation errors to `ConfigException` with the key and the layer.
5. Tests: resolution order, switching TTS mid-chat, fallback with notices, a voice the engine lacks, a fault mid-reply, caller bugs, starting over, duplicate ids, the factory's spec, config mapping.
6. Verify: ktlint, module graph, macOS tests, Linux x64/arm64 on Colima, mingwX64 link; push, PR stacked on #108, CI.
