---
title: aulo
tagline: A local-first voice agent that controls your computer, with switchable speech engines, MCP both ways and local or remote models.
repo: https://github.com/pyrlyn/aulo
install: "git clone https://github.com/pyrlyn/aulo && cd aulo && mise exec -- just test"
version: "0.0.0"
status: "Early development · no release yet"
accent: "#FF8A5C"
order: 10
---

<!-- Website copy for the pyrlyn project site, copied to pyrlyn/landing as
content/projects/aulo.md; front matter follows CONTENT_CONTRACT.md in that repository.
Sources (checked 2026-10-14): README.md, spec.md and plan.md; version from the workspace Cargo.toml (no release yet, hence status). The accent is a placeholder: the repository has no brand colour yet. -->

## Overview

aulo is a local-first voice agent: talk to it and it talks back, runs shell commands, opens and
drives apps, works in a browser and reads the screen when it has to. It runs as a daemon, `aulod`,
with a gRPC API for desktop apps, the `aulo` CLI and remote clients.

aulo is in early development with no release: the Rust crates of the daemon are being built and
the desktop apps are not in the repository yet. What follows is the planned scope from the
specification.

## Features

- **Switchable speech.** Every speech-to-text and text-to-speech engine is swappable at runtime.
- **Controls the computer.** Shell, apps, browser and screen reading.
- **MCP both ways.** aulo is an MCP client and an MCP server, and loads plugins.
- **Local or remote models.** the local model runner, Ollama, LM Studio, or OpenAI, Anthropic, Gemini, xAI, OpenRouter and any OpenAI-compatible endpoint.
- **gRPC API.** `aulod` serves desktop apps, the CLI, and remote clients of a headless server.

## Install

There is no release yet. Build and test from the repository:

```bash
git clone https://github.com/pyrlyn/aulo && cd aulo && mise exec -- just test
```

## Usage examples

Run the workspace tests:

```bash
mise exec -- just test
```

## Links

- Repository: <https://github.com/pyrlyn/aulo>
- Specification: <https://github.com/pyrlyn/aulo/blob/main/spec.md>
- gRPC API reference: <https://github.com/pyrlyn/aulo/blob/main/docs/api/aulo.v1.md>
- License: your choice of GNU GPLv3, a royalty-free license, or a commercial license (see <https://github.com/pyrlyn/aulo#license>)
