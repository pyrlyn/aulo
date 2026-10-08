> [!NOTE]
> This project is under active development. We need "testing volunteers": try it and report what breaks.

# aulo

A local-first voice agent that controls your computer.

Talk to aulo and it talks back. It runs shell commands, opens and drives apps, works in a browser, and reads the screen when it has to. It speaks MCP both ways, loads plugins, and runs on local models (runa, Ollama, LM Studio) or remote ones (OpenAI, Anthropic, Gemini, xAI, OpenRouter, any OpenAI-compatible endpoint). Every speech-to-text and text-to-speech engine is swappable at runtime.

aulo runs as a daemon, `aulod`, with a gRPC API. Use it from the native desktop apps with chats (macOS, Windows), from the `aulo` CLI, or run it headless on a server and connect remotely.

**Status:** specification and plan only. No code yet.

- [Specification](spec.md)
- [Research and sources](research.md)
- [Plan](plan.md)
- [gRPC API reference](docs/api/aulo.v1.md)

## How to test

Rust is pinned with mise. From the repository root:

```bash
mise exec -- just test
```

`just test` runs `cargo test --workspace`.

## License

You can use this project under **any** of the following licenses, at your choice:

1. [GNU GPLv3](LICENSE): free for open source applications on any platform, including embedded systems.
2. [Royalty-free License](LICENSE-ROYALTY-FREE.md): free for proprietary desktop, mobile, and web applications, as long as you disclose that your application uses this project. Embedded systems are not covered.
3. [Commercial license](PRICING.md): for proprietary applications, including embedded systems, without the attribution requirement.

<!-- license-sync:start -->
Commercial use not covered by the GPLv3 or the Royalty-free License requires a separate paid
license — see [PRICING.md](PRICING.md).
<!-- license-sync:end -->
