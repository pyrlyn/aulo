> [!NOTE]
> This project is under active development. We need "testing volunteers": try it and report what breaks.

# aulo

A local-first voice agent that controls your computer.

Talk to aulo and it talks back. It runs shell commands, opens and drives apps, works in a browser, and reads the screen when it has to. It speaks MCP both ways, loads plugins, and runs on local models (the local model runner, Ollama, LM Studio) or remote ones (OpenAI, Anthropic, Gemini, xAI, OpenRouter, any OpenAI-compatible endpoint). Every speech-to-text and text-to-speech engine is swappable at runtime.

aulo runs as a daemon, `aulod`, with a gRPC API. Use it from `aulo-chat` (the official Compose Multiplatform client, on Android, iOS and desktop), from the `aulo` CLI, or run it headless on a server and connect remotely.

**Status:** early development, no release yet. aulo is being rewritten in **Kotlin/Native**
(Gradle KMP, native targets). The first modules of the daemon (audio, speech engines, models,
plugins, the gRPC protocol, realtime, store, telemetry) will land in the Gradle project. The
official client is [aulo-chat](aulo-chat/README.md), a Compose Multiplatform app in this
repository; native SwiftUI and WinUI apps are not needed (spec §17 D2, D12). The plan has
been reset to the start: every task in [plan/](plan/README.md) is open again.

- [Specification](spec.md)
- [Research and sources](research.md)
- [Plan](plan/README.md) · [Roadmap](roadmap.md)
- [Documentation](docs/README.md): [gRPC API reference](docs/api/aulo.v1.md), [spike reports](docs/spikes/)
- [aulo-chat](aulo-chat/README.md): the Compose Multiplatform chat client (Android, iOS, desktop) with a development server; it builds and runs today

## Build

The Gradle wrapper brings Gradle and the Kotlin/Native toolchain; mise pins the JDK that runs
it and the lint tools:

```bash
mise install               # JDK 21 and the lint tools from mise.toml
./gradlew test             # host-target tests
./gradlew ktlintCheck      # style
./gradlew assemble         # aulo and aulod executables in aulo/build/bin/
```

`just api-docs` regenerates [docs/api](docs/api/aulo.v1.md) from `proto/`.

## License

You can use this project under **any** of the following licenses, at your choice:

1. [GNU GPLv3](LICENSE): free for open source applications on any platform, including embedded systems.
2. [Royalty-free License](LICENSE-ROYALTY-FREE.md): free for proprietary desktop, mobile, and web applications, as long as you disclose that your application uses this project. Embedded systems are not covered.
3. [Commercial license](PRICING.md): for proprietary applications, including embedded systems, without the attribution requirement.

<!-- license-sync:start -->
Commercial use not covered by the GPLv3 or the Royalty-free License requires a separate paid
license — see [PRICING.md](PRICING.md).
<!-- license-sync:end -->
