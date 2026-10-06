# aulo

A local-first voice agent that controls your computer.

Talk to aulo and it talks back. It runs shell commands, opens and drives apps, works in a browser, and reads the screen when it has to. It speaks MCP both ways, loads plugins, and runs on local models (runa, Ollama, LM Studio) or remote ones (OpenAI, Anthropic, Gemini, xAI, OpenRouter, any OpenAI-compatible endpoint). Every speech-to-text and text-to-speech engine is swappable at runtime.

aulo runs as a daemon, `aulod`, with a gRPC API. Use it from the desktop app with chats, from the `aulo` CLI, or run it headless on a server and connect remotely.

**Status:** specification and plan only. No code yet.

- [Specification](spec.md)
- [Research and sources](research.md)
- [Plan](plan.md)
