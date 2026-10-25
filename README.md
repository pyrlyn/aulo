> [!NOTE]
> This project is under active development. We need "testing volunteers": try it and report what breaks.

# aulo

A local-first voice agent that controls your computer.

Talk to aulo and it talks back. It runs shell commands, opens and drives apps, works in a browser, and reads the screen when it has to. It speaks MCP both ways, loads plugins, and runs on local models (runa, Ollama, LM Studio) or remote ones (OpenAI, Anthropic, Gemini, xAI, OpenRouter, any OpenAI-compatible endpoint). Every speech-to-text and text-to-speech engine is swappable at runtime.

aulo runs as a daemon, `aulod`, with a gRPC API. Use it from the native desktop apps with chats (macOS, Windows), from the `aulo` CLI, or run it headless on a server and connect remotely.

**Status:** early development, no release yet. The Rust workspace in [`crates/`](crates/)
holds the first crates of the daemon (audio, speech engines, models, plugins, the gRPC
protocol, realtime, store, telemetry); the desktop apps are not in the repository yet.
Progress is tracked in [plan.md](plan.md).

- [Specification](spec.md)
- [Research and sources](research.md)
- [Plan](plan.md) · [Roadmap](roadmap.md)
- [Documentation](docs/README.md): [gRPC API reference](docs/api/aulo.v1.md), [spike reports](docs/spikes/)

## How to test

Rust is pinned with mise. From the repository root:

```bash
mise exec -- just test
```

`just test` runs `cargo test --workspace`.

## License

You can use this project under **either** of the following licenses, at your choice:

1. [GNU GPLv3](LICENSE): free for open source applications on any platform, including embedded systems.
2. [Commercial license](PRICING.md): for proprietary applications, including embedded systems.

<!-- license-sync:start -->
Commercial use not covered by the GPLv3 requires a separate paid
license — see [PRICING.md](PRICING.md).
<!-- license-sync:end -->
