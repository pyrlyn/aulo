# aulo — specification

Status: draft for creator review. Sources for every external fact are in `research.md` (checked 2026-10-07).

The name comes from the Greek *aulos*, a reed pipe: breath turned into voice.

## 1. What aulo is

aulo is a local-first voice agent that controls the computer it runs on. You talk to it, and it talks back. It runs shell commands, launches and drives programs, and works in a browser. It can use MCP servers and act as one for other agents. It loads plugins, and it runs on local or remote language models that you can switch at any time.

It ships as one daemon, `aulod`, that owns the agent, the voice pipeline, the tools and the data. Three kinds of client reach it over gRPC:

- **Desktop apps.** Native SwiftUI (macOS) and WinUI 3 (Windows) apps with chats, voice, approvals and settings.
- **CLI (`aulo`).** Text chat, administration and the MCP server entry point.
- **Remote clients.** Any gRPC client, when aulod runs as a server on another machine.

### 1.1 Goals

1. **Voice first.**
   - Listening is hands-free (wake word) or push-to-talk.
   - Turn-taking is natural: end-of-turn detection and barge-in.
   - Replies stream as speech.
   - Every STT and TTS engine can be swapped at runtime.
2. **Computer control.**
   - Shell, apps, browser and desktop accessibility.
   - Screenshots plus synthetic input as the last resort.
3. **MCP both ways.** A built-in MCP client, and aulo as an MCP server.
4. **Plugins.** Tools, speech engines, LLM providers, skills and hooks.
5. **Any model.** Local models (runa, Ollama, LM Studio, llama.cpp) and remote providers (OpenAI, Anthropic, Gemini, xAI, OpenRouter, DeepSeek, any OpenAI-compatible endpoint), set per tier and per chat.
6. **Two deployment shapes.** A desktop app with chats, and a headless server with a gRPC API.
7. **Safety by construction.**
   - Policy is deterministic code outside the model and fails closed.
   - Voice never widens access.
   - Secrets never reach the model.

### 1.2 Non-goals for v1

- A hosted cloud service. aulo runs on the user's machine or the user's server.
- Mobile apps. The gRPC API makes them possible later; see `ideas.md`.
- Training models.
- Native in-process plugins. Plugins are processes or WASM, never dylibs.

## 2. Research summary

The full notes with sources are in `research.md`.

### 2.1 The three named products

| | Grok Bot (SpaceXAI, 2026-08) | Meta Muse (2026-09) | OpenAI dots (2026-09) |
| --- | --- | --- | --- |
| Shape | Always-on "teammates", several Bots per account | Personal agent for long consumer tasks | Always-on agents inside ChatGPT |
| Where it acts | One persistent cloud computer per account (browser, terminal, `/workspace`, shared logins); each Bot has its own screen. The local computer is opt-in, under a policy | Muse Secure VM per user with its own browser; Muse for Mac drives local apps with permission | Its own cloud computer and browser; the owner's laptop with permission |
| Voice | Voice chat on desktop and mobile; voice memos. The developer Voice Agent API is speech-to-speech over WebSocket with server VAD and MCP/function tools | Realtime Voice that "works while you talk"; wake word = the agent's name on glasses | Call or message a dot; GPT-Live 1 full-duplex front that delegates to a backend agent; spoken progress fillers |
| Tools | Connector plugins from a marketplace; computer use as the fallback | Per-app connectors with access levels | 4,000+ app plugins |
| Skills | "Teach a task": a screen recording of up to 10 minutes becomes a skill | — | — |
| Safety | Approvals under user-editable Auto-review rules; human takeover for passwords, 2FA and CAPTCHA | Separate **Sentinel** agent approves actions and egress; credential vault the agent cannot read; audit trail | Read-only apps during unattended work; auto-review check for consequential actions |
| Source | Closed | Closed | Closed |

### 2.2 Others studied

- **Claude computer use.**
  - Tool order: connectors → browser → screen.
  - Control in background windows.
  - Per-app grants and category blocklists.
  - Batched actions that stop on the first failure.
- **Gemini computer use and Live.**
  - Normalized 0–999 coordinates.
  - A model-emitted `require_confirmation` signal.
  - Asynchronous function calls during live voice.
- **Windows agent workspace.**
  - A separate OS account per agent.
  - An OS registry of MCP servers.
  - A local wake word.
  - A tamper-evident audit log.
- **OpenClaw** (open source).
  - Rule: "trusted gateway, untrusted execution, deterministic policy".
  - A realtime voice model *consults* the main agent instead of calling tools.
  - A central wake-word list routed to agents.
  - Weak spots: sandbox off by default, in-process plugins.
- **Goose** (Rust). An MCP-native host on `rmcp`, with local Whisper dictation.
- **Handy** (Rust). A push-to-talk STT reference:
  - `cpal`, Silero VAD, Whisper/Parakeet, `enigo`.
- **Voice pipeline frameworks.**
  - Pipecat: Smart Turn v3 end-of-turn model.
  - LiveKit Agents: semantic turn detection.
  - Kyutai Unmute: streaming STT/TTS around any LLM.
- **Browser control.**
  - Playwright MCP: accessibility snapshots, and the token-cheaper CLI-plus-skill form.
  - Chrome DevTools MCP.
  - `chromiumoxide` (CDP).

### 2.3 What aulo takes, and what it does better

| Idea | From | aulo's version |
| --- | --- | --- |
| Persistent agent host with several bots | Grok Bot | `aulod` hosts several named bots, each with its own prompt, voice, wake word, grants and workspace (T14.1). The host is the user's own machine or server, not a vendor cloud |
| Agent computer the user can watch and take over | Grok Bot, Muse, dots | Agent screen viewer with Take over and Stop (T12.14, T12.19); `request_takeover` for secrets, 2FA and CAPTCHA (T5.11); a container image as an isolated agent computer (T13.5) |
| Two-brain voice: a cheap full-duplex front, a slow backend agent | dots/GPT-Live, Gemini Live, OpenClaw | The realtime front has exactly one tool, `consult_agent`. Policy runs in the backend (T8.11). The local cascade is the default and needs no cloud |
| Work while the user keeps talking | Muse | Utterances during a running turn become steering messages (T8.15) |
| Wake word = the agent's name, routed to an agent | Muse glasses, OpenClaw | On-device open-vocabulary keyword spotting; each trigger is routed to a bot or chat (T8.6) |
| Separate reviewer for consequential actions | Muse Sentinel, auto-review | The sentinel tier can only make decisions stricter, never allow what policy denies (T5.8) |
| Read-only when unattended | dots | Routines and runs with no client attached cannot write, run or type (T5.7) |
| Vault the agent can use but not read | Muse | Secret handles; values are injected by tools and never reach the model, transcripts or logs (T5.10) |
| Connectors → browser → screen ladder | Claude | Control ladder: MCP/API → shell/app → browser snapshot → desktop AX → screenshot + input (T9.18) |
| Normalized coordinates | Gemini | 0–999 coordinates for screenshots and input (T9.15, T9.16) |
| Per-app and per-site grants, category blocklist | Claude, Windows | App/Site subjects plus a built-in blocklist (T5.6) |
| Tamper-evident audit | Windows | Hash-chained audit log with `aulo audit verify` (T5.9) |
| Teach a task | Grok Bot | Demonstration → draft SKILL.md for review (T14.5) |
| Voice never widens access | Grok Bot | Destructive, payment and credential actions need a click or key, never just a spoken yes (T8.9) |
| Sandbox on by default (OpenClaw's gap) | — | Commands run in the workspace-write sandbox by default (T5.4) |
| No in-process native plugins (OpenClaw's gap) | — | Plugins are MCP or gRPC processes, or WASM with deadlines (T11.3, T11.4, T11.7) |
| Local-first (Grok Bot/Muse/dots are cloud-only) | — | Fully local voice, models and control are the default; the cloud is opt-in per tier |

## 3. Requirements

### 3.1 Functional

| Id | Requirement | Tasks |
| --- | --- | --- |
| R1 | Speech recognition: local and remote STT engines, streaming partials, at least en and ru | S7 |
| R2 | Speech synthesis through several TTS engines, switchable at runtime per bot or chat, with a fallback chain | T7.2, T7.9–T7.14, T11.6 |
| R3 | Voice conversation: push-to-talk and wake word, end-of-turn detection, barge-in, progress fillers | S8 |
| R4 | Computer control: shell (sandboxed), PTY, apps, browser, desktop accessibility, screenshots, input | S9 |
| R5 | MCP client (stdio, streamable HTTP, OAuth) and aulo as an MCP server (stdio and HTTP) | S10 |
| R6 | Plugins: tools, STT/TTS engines, LLM providers, skills, hooks; grants by digest; fail open | S11 |
| R7 | Models and providers: local and remote, per-tier routing, per-chat switching, keys in the OS keychain | S4 |
| R8 | Desktop apps (macOS, Windows) with chats, voice, approvals and settings | S12 |
| R9 | Server mode with a gRPC API, TLS, scoped tokens and remote voice streams | S3, S13 |
| R10 | Safety: policy, approvals, sandbox, sentinel, audit, vault, takeover, kill switch | S5 |

### 3.2 Quality targets

- **Latency** (T8.17):
  - wake word to listening: under 300 ms;
  - end of speech to first audio: under 1.2 s with local STT/TTS and a remote LLM;
  - barge-in to silence: under 150 ms;
  - kill switch: under 200 ms.
- **Privacy.**
  - The default configuration sends nothing off the machine until the user adds a remote provider.
  - No telemetry is sent anywhere.
- **Resilience.** A broken engine, MCP server, plugin or hook is warned about and skipped, never fatal.
- **Platforms (D3).**
  - macOS arm64: macOS 15+, and macOS 26+ for SpeechAnalyzer. Daemon, CLI and SwiftUI app.
  - Windows 11 x86_64. Daemon, CLI and WinUI 3 app.
  - Linux x86_64 and aarch64. Daemon and CLI (desktop and server); no native desktop app yet.
  - Intel macOS is not supported.

## 4. Architecture

```
            ┌───────────────────────── clients (gRPC) ─────────────────────────┐
            │  macOS / Windows apps (chats)  aulo CLI   remote gRPC clients     │
            └───────────────┬──────────────────┬──────────────────┬────────────┘
                 Unix socket / named pipe      │         TCP + TLS + token
            ┌───────────────▼──────────────────▼──────────────────▼────────────┐
            │ aulod                                                           │
            │  aulo-server ── Chat / Session / Voice / Approval / Config / Mcp │
            │       │                                                          │
            │  ┌────▼─────┐   ┌──────────────┐   ┌───────────────────────────┐ │
            │  │aulo-voice│──▶│  aulo-agent  │──▶│ aulo-policy (rules, grants,│ │
            │  │ pipeline │◀──│ (agent-loop) │   │ sentinel, audit, vault)   │ │
            │  └─┬────┬───┘   └──┬───────┬───┘   └────────────┬──────────────┘ │
            │    │    │          │       │                    │ allowed calls  │
            │ aulo-  aulo-   aulo-      tool registry ────────▼──────────────┐ │
            │ audio  speech  providers  shell · pty · apps · browser · AX ·  │ │
            │ (cpal) (STT,   (llm-*)    screen · input · mcp__* · plugin__*  │ │
            │        TTS,                                                    │ │
            │        VAD,KWS)  aulo-realtime (S2S front, consult only)       │ │
            │  aulo-store (SQLite) · aulo-config · aulo-telemetry             │ │
            └──────┬───────────────┬────────────────────┬────────────────────┘
                   │               │                    │
          local models       remote providers     MCP servers / plugins
     (runa, Ollama, LM Studio) (OpenAI, Anthropic,  (processes: MCP stdio, gRPC UDS;
                               Gemini, xAI, …)        WASM in-process)
```

### 4.1 Processes

- **`aulod`.**
  - The only process that holds state, credentials and policy. This is the "trusted gateway".
  - It runs per user as a login service, or headless on a server.
- **`aulo`.** A thin CLI client. `aulo mcp` runs the stdio MCP server; it is a client of aulod, so every call still goes through aulod's policy.
- **Desktop apps** (`desktop/macos`, SwiftUI; `desktop/windows`, WinUI 3).
  - Native UI over `aulo-app`, a Rust core exported with UniFFI (`aulo-ffi`), as in cox.
  - `aulo-app` is a gRPC client of aulod; it starts the local daemon when it is not running.
  - It can also connect to remote servers.
- **Plugins and MCP servers.**
  - Child processes of aulod with the capabilities they were granted.
  - Untrusted. Their output is data.

### 4.2 Crates

Crate roles follow rust.md: contracts, domain, adapters, assembly, surfaces, testkit. The dependency-graph test (T2.2) enforces them.

| Crate | Role | Purpose |
| --- | --- | --- |
| `aulo-types` | contract | Ids, aulo events, voice state |
| `aulo-proto` | contract | gRPC protos and generated code (tonic, prost) |
| `aulo-speech` | contract | `SttEngine`, `TtsEngine`, `Vad`, `KeywordSpotter`, `TurnDetector` traits |
| `aulo-config` | adapter (config owner) | Typed config, JSON Schema, layers, provenance |
| `aulo-store` | adapter (diesel owner) | SQLite: bots, chats, messages, tool calls, usage, grants, audit, memory |
| `aulo-telemetry` | adapter | Logs, traces, redaction |
| `aulo-audio` | adapter | Capture, playback, AEC, devices |
| `aulo-speech-sherpa` | adapter (sherpa-onnx owner) | VAD, KWS, Parakeet/Moonshine STT, Kokoro/Matcha/VITS TTS |
| `aulo-speech-whisper` | adapter | whisper.cpp STT |
| `aulo-speech-system` | adapter | macOS, Windows and Linux system voices; Apple SpeechAnalyzer |
| `aulo-speech-cloud` | adapter | OpenAI-compatible STT/TTS, Deepgram, ElevenLabs |
| `aulo-realtime` | adapter | Realtime speech-to-speech fronts (OpenAI, Gemini, xAI) |
| `aulo-voice` | domain | Conversation pipeline, turn-taking, barge-in, fillers, voice commands |
| `aulo-providers` | assembly | LLM providers from config (llm-* crates), tiers, local presets |
| `aulo-agent` | domain | Sessions on agent-loop, tool registry, persona, compaction |
| `aulo-policy` | domain | perm-rules subjects, grants, blocklist, sentinel, audit, vault, takeover |
| `aulo-tools` | adapter | Shell, PTY, files, clipboard |
| `aulo-apps` | adapter | App launch, focus and listing per OS |
| `aulo-browser` | adapter (chromiumoxide owner) | Managed Chromium over CDP |
| `aulo-desktop-ax` | adapter | AX (macOS), UIA (Windows), AT-SPI (Linux), screenshots, input |
| `aulo-mcp` | adapter | MCP client (mcp-host) and aulo's MCP server (rmcp) |
| `aulo-plugin` | adapter | Manifest, discovery, grants, process and WASM hosts |
| `aulo-plugin-sdk` | contract | Plugin-side SDK (gRPC plugin services, manifest types) |
| `aulo-server` | assembly | gRPC services, auth, TLS |
| `aulo` | surface | CLI binary (`aulo`) and daemon binary (`aulod`) |
| `aulo-app` | domain | UI-agnostic client core for the native apps: gRPC client, Timeline fold, coalescing Controller, inbox, intents (cox-app pattern) |
| `aulo-ffi` | surface | Forward-only UniFFI exports over `aulo-app`; Swift and C# (uniffi-bindgen-cs) bindings; the only crate that depends on `uniffi` |
| `aulo-testkit` | testkit | Scripted provider, test daemon, fake audio and engines |

Heavy subsystems are off-by-default features of the binaries: `sherpa`, `whisper`, `browser`, `desktop-control`, `wasm-plugins`, `realtime`, `otel`.

### 4.3 One voice turn (local cascade)

1. `aulo-audio` delivers 20 ms frames at 16 kHz mono into a ring buffer. With AEC on, the playback stream is the far-end reference.
2. The keyword spotter waits for the wake word, unless push-to-talk is used. A trigger selects the bot or chat.
3. VAD marks speech, and the STT engine streams partial transcripts to clients.
4. End of turn is decided by a silence timeout, plus the semantic end-of-turn model when enabled.
5. The final transcript becomes a `UserTurn` submission to the bot's chat session.
6. The agent streams its reply. Tool calls pass policy (§7); approvals can block, and spoken fillers cover tool latency.
7. The speakable-text normalizer cuts the reply into sentences, without markdown and with code replaced by "it is on the screen". The TTS engine synthesizes it sentence by sentence.
8. Playback starts with the first sentence. If the user speaks during playback, barge-in stops it within 150 ms and either interrupts the turn or keeps it running (config).

### 4.4 Two-brain mode (realtime front)

When a realtime provider is selected for the voice tier:

- The speech-to-speech model holds the conversation.
- Its only tool is `consult_agent(request)`, which submits to the main agent; the main agent applies policy and tools.
- The front's other tool calls are refused.
- This keeps the latency of native voice models without letting them act unchecked.

## 5. Voice subsystem

### 5.1 Engines (all switchable)

| Kind | Default (local) | Alternatives |
| --- | --- | --- |
| VAD | earshot (pure Rust) | Silero via sherpa-onnx |
| Wake word | sherpa-onnx keyword spotting (open vocabulary) | plugin |
| End of turn | silence 700 ms | + Smart Turn v3 (BSD-2) |
| STT | sherpa-onnx Parakeet TDT v3 (25 languages incl. ru, uk) | Moonshine streaming (en), whisper.cpp, Apple SpeechAnalyzer, OpenAI-compatible endpoint (OpenAI, runa), Deepgram, plugins |
| TTS | sherpa-onnx Kokoro (Apache-2.0 weights) | Matcha, VITS, macOS/Windows/Linux system voices, OpenAI-compatible speech, ElevenLabs, Piper (GPL, plugin only), plugins |
| Realtime front | none (cascade) | OpenAI GPT-Live / Realtime, Gemini Live, xAI Voice Agent |

### 5.2 Switching and fallback

- **Where an engine is chosen.** `[voice.stt]` and `[voice.tts]` hold `engine`, `model`, `voice`, `rate` and `fallback = [...]`.
- **Overrides.**
  - A bot can override the choice, and a chat can override the bot.
- **How to switch.**
  - `VoiceService.SetVoice`;
  - `aulo voice use <engine> [voice]`;
  - the desktop picker with a preview;
  - a local voice command ("use voice Anna").
  - The new engine is used from the next utterance; a sentence already being spoken finishes first.
- **Plugin engines.** They register in the same registry (T11.4) and are chosen the same way.
- **Fallback.** A failing engine raises a notice and the next engine in `fallback` takes over. This is the fail-open rule; voice keeps working.
- **Models.** They are downloaded on demand with pinned SHA-256 hashes (T7.3).

### 5.3 Audio

- **I/O.** `cpal` capture, `rubato` resampling, `ringbuf` frames, and `rodio` streaming playback with an immediate stop.
- **Echo cancellation.**
  - macOS: Voice Processing I/O.
  - Windows and Linux: webrtc-audio-processing (AEC3, noise suppression, AGC).
  - The T0.6 spike makes the final choice.
- **One audio owner at a time.** Wake word, push-to-talk and the realtime front arbitrate for the microphone.
- **Remote clients.** They stream PCM16 16 kHz frames, or Opus (T6.7), over `VoiceService.Talk`.

## 6. Computer control

### 6.1 Tools

| Tool group | Backend | Policy subject |
| --- | --- | --- |
| `shell` | proc-sandbox (Seatbelt, bwrap, Landlock+seccomp), output archive | `Shell(cmd)` + risk class |
| `pty_*` | portable-pty | `Shell(cmd)` |
| `files_*` | confined to allowed roots; optional `cox mcp` preset for code work | `File(path)` |
| `app_*` | NSWorkspace / ShellExecute / gio | `App(id)` |
| `browser_*` | chromiumoxide on a dedicated profile; Playwright MCP alternative | `Site(host)` |
| `ax_*` | AXUIElement / UIA / AT-SPI | `App(id)` |
| `screen_*`, `input_*` | xcap, enigo, normalized 0–999 coordinates | `Input(kind)` |
| `clipboard_*` | arboard | `Clipboard(read/write)` |
| `request_takeover`, `type_secret` | policy | — |

### 6.2 Control ladder

The system prompt and tool descriptions steer the model in this order (T9.18):

1. MCP server or API connector.
2. Shell or app command.
3. Browser accessibility snapshot with element refs.
4. Desktop accessibility tree.
5. Screenshot plus synthetic input, last.

Batched UI actions stop at the first failure.

### 6.3 Isolation levels

1. **User's desktop** (default). Commands run in the workspace-write sandbox. The browser uses a dedicated profile. GUI work happens in the user's session.
2. **Container agent computer** (T13.5). Headless Chromium and a virtual display inside an OCI image, like the Grok Bot cloud computer.
3. **Separate OS user or VM** (T9.20, research).

## 7. Safety model

Every tool call, whichever surface or plugin asks for it, passes the same pipeline:

1. **Rules** (perm-rules grammar): deny, then allow, then ask, then session grants.
2. **Risk fallback.** Decides when no rule matched: the shell classifier, plus the tool's declared risk.
3. **Blocklist.** Banking, payments, crypto, password managers and system settings always ask or deny.
4. **Mode.** Unattended runs are read-only.
5. **Sentinel.** Reviews consequential actions and may escalate to Ask. It cannot allow what the steps above denied.
6. **Approval.**
   - Asked through any attached client, or by voice for low and medium risk.
   - No answer before the timeout means deny.
7. **Execution** in the sandbox.
8. **Audit.** A hash-chained row is written.

Also:

- **Secrets.** Secrets are handles. Tools inject the values, and the model only ever sees the handle.
- **Untrusted content.** Web pages, tool output, screenshot text and plugin output are untrusted data. They are sanitized, and instructions found in them force Ask for the next consequential action.
- **Kill switch.** A hotkey, the voice word "stop" or a gRPC Interrupt cancels everything within 200 ms.
- **Threat model.** Kept in `docs/threat-model.md` (T5.13).

## 8. MCP

- **Client** (T10.1–T10.4).
  - Built on the shared `mcp-host` crate (rmcp 3.x), extracted from cox.
  - Servers come from `[mcp.servers]` and `.mcp.json`, over stdio or streamable HTTP, with OAuth for remote servers.
  - Their tools appear as `mcp__server__tool`, deferred by default, and every call goes through policy.
  - Resources, prompts and elicitation are supported; elicitation is answered by voice or in the desktop app.
  - A broken server is a notice.
- **Server** (T10.5–T10.7).
  - `aulo mcp` (stdio) and aulod's streamable HTTP endpoint (bearer token) expose `speak`, `listen`, `ask_user`, `notify` and the computer tools.
  - Callers are subject to aulo's policy and approvals; an external agent can never bypass them.
  - `aulo mcp install --host claude-code|cursor|codex` registers the server, editing only aulo's entry in the host's config.

## 9. Plugins, skills, hooks

**Manifest.** `aulo-plugin.toml` (schema-checked) declares:

- `id` and `version`;
- `kinds`: tools, stt, tts, llm-provider, skills, hooks;
- requested capabilities: network hosts, filesystem roots, exec, audio;
- the entry: `mcp-process`, `grpc-process` or `wasm`.

**ABIs.**

| ABI | Use | Isolation |
| --- | --- | --- |
| MCP over stdio (default) | Tools | Process; capabilities via sandbox profile |
| gRPC over a Unix socket (`aulo.plugin.v1`) | Streaming kinds: STT, TTS, LLM providers | Process; health-checked and restarted |
| WASM (extism or wasmtime components) | Small tools and hooks in-process | Deadline and memory cap per call; network only through an allowlisted host function |

**Grants.**

- A grant is bound to the package digest.
- A new digest or a new capability needs approval again.
- A broken plugin is skipped with a warning.

**Skills and hooks.**

- **Skills** use the `SKILL.md` folder layout. Only an index line enters the prompt, and the `skill` tool returns the body.
- **Hooks** use the Claude Code JSON protocol, on stdin and stdout; exit code 2 blocks. They fire on these events:
  - UserPromptSubmit
  - PreToolUse
  - PostToolUse
  - Heard
  - BeforeSpeak
  - SessionStart and SessionEnd
- Both come from the shared `agent-ext` crate.

## 10. Models and providers

- **Wires.** The provider stack is the shared `llm-*` crates extracted from cox:
  - OpenAI Chat and Responses, which also cover every OpenAI-compatible endpoint with no per-vendor code;
  - Anthropic Messages;
  - the model catalog;
  - credentials (env var, then OS keychain; inline keys are rejected).
- **Local.**
  - runa, through its OpenAI-compatible server. T1.19 makes it stream tokens; T1.20 adds in-process embedding.
  - Ollama and LM Studio.
  - Running servers are auto-detected.
- **Remote.** OpenAI, Anthropic, Gemini (compat), xAI, OpenRouter, DeepSeek, any custom endpoint.
- **Tiers.** Each tier may use a different provider through `llm-router`:

  | Tier | Used for |
  | --- | --- |
  | `voice` | Fast, short answers, or the realtime front |
  | `main` | Tools |
  | `sentinel` | The reviewer |
  | `cheap` | Titles and summaries |

- **Switching.** A chat stores its model and can switch mid-chat. Each provider call writes a usage row.

## 11. gRPC API (`aulo.v1`)

| Service | Key RPCs |
| --- | --- |
| `ChatService` | CreateChat, ListChats, GetChat, RenameChat, DeleteChat, ListMessages, SearchMessages |
| `SessionService` | `Converse(stream ConverseRequest) returns (stream ConverseResponse)`: attach, user turns, approvals, interrupts, model switches, takeover-done in; `Event`s (text deltas, tool calls, approvals, notices) out |
| `VoiceService` | `Talk(stream TalkRequest) returns (stream TalkResponse)`: start, audio frames and control in; state, partial/final transcripts and TTS audio out. StartListening, StopListening, SetVoice, ListVoices |
| `ApprovalService` | ListPending, Decide, ListGrants, RevokeGrant |
| `ConfigService` | ListProviders, SetProviderKey (write-only), ListModels, ListAudioDevices, SetDevice, GetConfig, SetConfig |
| `McpService` | ListServers, ServerStatus, Enable, Disable, Restart |
| `PluginService` | List, Install, Enable, Disable, Remove |
| `AuditService` | Query, Verify, Export |
| `grpc.health.v1` | Health (tonic-health); reflection for tooling |

**Transport.**

- Locally: a Unix socket or Windows named pipe, owner-only.
- Remotely: TCP with TLS (a given certificate, or self-signed with fingerprint pinning).
- Authentication: bearer tokens with scopes (read, chat, voice, control, admin), stored hashed.

**Compatibility.** The protos are linted, and `buf breaking` against main runs in CI.

## 12. Desktop app

Native apps, as in cox (D2):

- **`aulo-app`** (Rust) holds all client logic: the gRPC connection, folding events into keyed block patches, coalescing patches to one batch per frame, the chat list, the inbox and intents. Replaying a recorded event log yields the same patches as the live run, so UI tests run on fixtures.
- **`aulo-ffi`** exports it with UniFFI. Every exported body is a single expression, so logic cannot leak into the bindings.
- **macOS:** SwiftUI app in `desktop/macos` with a window and a menu bar extra.
- **Windows:** WinUI 3 (C#) app in `desktop/windows` with a window and a tray icon.
- **Linux:** no native app yet; use the CLI or connect a desktop app on another machine to the Linux server.

Both apps offer:

- Chat list grouped by bot, with search.
- Chat view: streamed markdown, tool-call cards and screenshots.
- Composer: text, attachments, push-to-talk, model picker.
- Voice indicator with a live transcript.
- Approvals and takeover cards, and system notifications.
- Settings:
  - providers and keys;
  - models per tier;
  - STT/TTS engine and voice pickers with preview;
  - devices, hotkeys and wake words;
  - MCP servers and plugins;
  - rules, grants and audit.
- A quick-talk window on a global hotkey.
- An agent screen viewer with Take over and Stop.
- Connections to the local daemon and to remote servers.
- English and Russian UI.

## 13. Server mode

- **Start.** `aulod run --headless --listen 0.0.0.0:7443`.
  - TLS is required.
  - The server opens no local audio devices; voice comes only through `VoiceService` streams.
- **Clients.** Several clients can attach to one chat. Their presence decides whether a run counts as unattended.
- **Limits.** Scoped tokens, rate limits and quotas.
- **Container image** with headless Chromium for computer control.
- **Clients in other languages.** Generated from the protos, with Python and TypeScript examples.

## 14. Storage and config

- **Database.** SQLite through Diesel, at `~/.aulo/aulo.db`, in WAL mode. Raw SQL only in migrations and for FTS5.
- **Config.**
  - Layers: `~/.aulo/config.toml`, then project `.aulo.toml`, then `AULO_*` env, then flags, with per-key provenance.
  - A JSON Schema is generated from the Rust types and committed, with a stale-schema test.
  - One crate owns config I/O.
- **Foreign configs** (agent hosts' MCP entries) are edited only at aulo's own key: atomically, byte-preserving, with a `file-backup` copy.
- **Models** live in `~/.aulo/models`, and logs in `~/.aulo/logs`.

## 15. Reuse from listepo

The full map with `path:line` citations is in `research.md` Part 6. Summary:

| Need | Source | Plan |
| --- | --- | --- |
| Provider trait, wires, credentials, catalog | cox-protocol, cox-provider*, cox-models | Extract into `llm-wire`, `llm-http`, `llm-openai`, `llm-anthropic`, `llm-catalog` (T1.1–T1.6) |
| Provider per tier | cox-session `provider_for` (one provider per session) | `llm-router` (T1.7) |
| Agent loop | cox-core | `agent-loop` (T1.15, decision T0.7) |
| Permissions, sandbox, bash risk | cox-permission, cox-sandbox, cox-tools bash classifier | `perm-rules`, `proc-sandbox`, `shell-classify` (T1.8–T1.10) |
| Sanitizing untrusted text | cox-sanitize, rtok sanitize, ketch changelog::sanitize | `text-sanitize` (T1.11) |
| MCP client | cox-mcp | `mcp-host` (T1.12) |
| MCP server pattern | rtok `src/mcp.rs`, cox-mcp server | Followed in `aulo-mcp` (T10.5) |
| Registering in agent hosts | rtok-agent-sdk `register_mcp` | `agent-host-config` (T1.18) |
| Mic capture, resample, Whisper, energy VAD | cox-voice, runa-media | `speech-capture` (T1.13) |
| Config with schema and provenance | cox-config, rtok config/layers.rs, ketch toml_file.rs | `config-schema` (T1.14) |
| Skills and hooks | cox-ext | `agent-ext` (T1.17) |
| WASM plugin host and grants | cox-plugin, rtok plugins/wasm.rs | Pattern for T11.7 |
| Local inference | runa serve and engine | OpenAI-compatible endpoint now; streaming fix (T1.19); library target (T1.20) |
| File backups | packages/crates/file-backup (crates.io 0.1.1) | Direct dependency |
| CI and release | pyrlyn/ci ci-rust.yml, pyrlyn/infra bump.yml, cargo-dist | Direct reuse (T2.3, T15.1) |
| File and code tools | `cox mcp` stdio server | Optional MCP preset (T10.8) |

Licence (D1): aulo is licensed like cox, so cox code can be reused directly. runa is MIT OR Apache-2.0, which can be combined into a GPL work.

## 16. Stages

| Stage | Name | Outcome |
| --- | --- | --- |
| S0 | Decisions and spikes | Decisions D1–D5 settled; sherpa-onnx and AEC still to measure (T0.5, T0.6) |
| S1 | Shared crates in packages/ | Provider stack, permissions, sandbox, MCP host, speech capture, agent loop shared with cox/runa |
| S2 | Workspace foundations | Workspace, CI, types, config, telemetry, store |
| S3 | gRPC API and daemon | Protos, server, auth, TLS, aulod, text CLI |
| S4 | Models, providers and agent loop | Text chat end to end on local and remote models |
| S5 | Safety core | Policy, approvals, sandbox, kill switch, sentinel, audit, vault, takeover |
| S6 | Audio I/O | Capture, playback, devices, AEC |
| S7 | Speech engines | STT and TTS engines, registry and switching, models, normalizer |
| S8 | Voice conversation | Pipeline, VAD, PTT, wake word, barge-in, VoiceService, realtime front |
| S9 | Computer control | Shell, PTY, apps, browser, AX, screenshots, input, ladder |
| S10 | MCP | Client, server (stdio and HTTP), host registration |
| S11 | Plugins, skills and hooks | Manifest, host, MCP/gRPC/WASM plugins, SDK, skills, hooks |
| S12 | Desktop apps (SwiftUI and WinUI) | aulo-app core, UniFFI bindings, macOS app, Windows app |
| S13 | Server mode | Headless, tokens, multi-client, container image |
| S14 | Bots, routines and memory | Named bots, background turns, routines, memory, teach-a-task |
| S15 | Release and docs | Packages, signing, guides, API reference |

**Milestones.**

- **M1, text agent.**
  - The P0 tasks of S0–S5.
  - Result: talk to aulod by text from the CLI, on local and remote models, with a policy-checked shell.
- **M2, voice agent.**
  - The P0 tasks of S6–S8.
  - Result: push-to-talk voice turns with switchable local STT and TTS, plus barge-in.
- **M3, computer agent.**
  - The P0 tasks of S9–S11.
  - Result: browser and apps, MCP both ways, plugins.
- **M4, desktop and server.**
  - The P0 tasks of S12–S13.
- **v1.0.** All P0 and P1 tasks.

Priorities:

- **P0**: the path to M1–M4;
- **P1**: needed for v1.0;
- **P2**: after v1.0;
- **P3**: research or nice to have.

Complexity runs from 1 (an hour or two of focused work) to 5 (a multi-part task that pushes the 500-line cap).

## 17. Decisions

Settled by the creator on 2026-10-07 (details in `done.md`):

- **D1 Licence** (T0.1). Like cox: `GPL-3.0-or-later OR LicenseRef-aulo-Royalty-Free`, plus a commercial licence.
  - `LICENSE` and `PRICING.md` are byte copies of the canonical files in `pyrlyn/ci` `licenses/`.
  - `LICENSE-ROYALTY-FREE.md` is the cox text.
  - The README carries the license-sync block.
  - `license-check.yml` fails on drift.
  - The Cargo manifests use the expression above.
- **D2 Desktop** (T0.2). Native SwiftUI (macOS) and WinUI 3 (Windows) over `aulo-app` + `aulo-ffi` (UniFFI), as in cox.
- **D3 Platforms** (T0.3). macOS arm64 (15+), Windows 11 x86_64, Linux x86_64 and aarch64.
- **D4 New crates** (T0.4). Approved:
  - API: tonic, tonic-build, prost, tonic-health, tonic-reflection;
  - speech and audio: sherpa-onnx, earshot, rodio, ringbuf, webrtc-audio-processing;
  - browser, screen and input: chromiumoxide, xcap, enigo;
  - platform accessibility and audio: objc2-application-services, objc2-avf-audio, atspi, uiautomation;
  - other: handy-keys, tokio-tungstenite.

  Each crate gets its `rust.md` and `toolchain.md` rows in the task that wires it in.
- **D5 Agent loop** (T0.7). Extract a neutral `agent-loop` crate from cox-core (T1.15) and build `aulo-agent` on it.

- **D6** The API is gRPC (tonic and prost), not HTTP/JSON. Creator, 2026-10-07.
- **D7** There are two deployment shapes: a desktop app with chats, and a server with an API. Creator, 2026-10-07.
- **D8** The repository is the public `pyrlyn/aulo`. Creator, 2026-10-07.

Open:

- **Native desktop app for Linux.** Linux runs the daemon and the CLI; whether it gets a desktop app (and with which toolkit) is not decided.

## 18. Risks

| Risk | Mitigation |
| --- | --- |
| sherpa-onnx build or size on all targets | T0.5 spike; whisper.cpp and remote engines as fallbacks |
| Echo makes barge-in fire on aulo's own voice | T0.6 spike, VP-IO / AEC3, headphones mode, barge-in sensitivity |
| Prompt injection through web pages and screenshots | Untrusted-content guards, sentinel, blocklist, read-only unattended mode |
| The model acts on the wrong window or element | Accessibility refs before pixels, batched actions stop on failure, screen viewer and kill switch |
| Licence mix (Piper GPL, model licences such as CC BY-NC) | D1 keeps aulo GPL-compatible; Piper only as a separate plugin; model licences checked in the model manager table |
| Extraction work in cox delays aulo | S1 tasks run in parallel with S2–S3, which do not depend on them |
| runa streaming latency | T1.19, or Ollama/LM Studio until it lands |
