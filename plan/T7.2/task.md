# T7.2. Engine registry and runtime switching

Stage: S7 · Area: speech · Depends on: T7.1, T2.5 · Blocks: 5 task(s)

Engines register by id (built-in and plugin-provided). [voice.stt] and [voice.tts] select engine, model and voice; a chat or bot can override; switching through gRPC, CLI (aulo voice use) or voice command takes effect on the next utterance without restart. A failing engine is skipped for the next one in the fallback list (fail open).


Done when:

- test switches TTS mid-chat; a failing engine falls back with a notice
