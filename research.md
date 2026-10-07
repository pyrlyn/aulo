# aulo — research

All sources checked on **2026-10-07** unless a different date is given. "cd" means "checked date".
Facts backed only by press, blogs or forums are marked **unverified**. Star counts and versions come from
the GitHub REST API (`api.github.com/repos/...`) and the crates.io / Hugging Face registry APIs, queried on 2026-10-07.

Notes on access: `openai.com` and `help.openai.com` return HTTP 403 to automated fetches, and so does `x.ai/news`.
For those pages the facts below come from search-engine extracts of the vendor page itself (still the vendor's
words) plus vendor docs that did load (`docs.x.ai`, `developers.openai.com`). Where only press coverage confirmed a
detail, it is marked **unverified**.

---

## Part 1. The three products the creator named

### 1.1 "Grok Bot" — SpaceXAI (formerly xAI) Grok Bot

**What it is.** Always-on "AI teammates" that each work on a persistent cloud computer (browser, filesystem,
terminal), sign into your tools, run multi-step jobs, and come back only when they need approval.
- Announced **2026-08-11** — https://x.ai/news/introducing-grok-bot (cd 2026-10-07).
- Release notes list "Grok Bot availability … durable AI teammates that work on a persistent cloud computer" under
  August 2026 — https://docs.x.ai/developers/release-notes.
- Voice mode added **2026-09-17** (desktop + mobile, rolled out over several days) — **unverified** (only
  https://runtimewire.com/article/grok-bot-adds-voice-desktop-mobile); vendor FAQ confirms voice chat exists (below).
- Status: generally available to paid subscribers; enterprise via waitlist. Closed source.
- Access: SuperGrok / SuperGrok Plus / SuperGrok Heavy, Cursor Pro / Pro+ / Ultra, Cursor Teams Standard / Premium —
  https://x.ai/news/introducing-grok-bot. The FAQ defers security and data retention to "the applicable Cursor terms"
  — https://docs.x.ai/grok-bot/faq. (So Grok Bot ships jointly with Cursor; the corporate story behind that is
  **unverified** from primary sources.)
- Platforms (client apps): macOS (Apple silicon and Intel), Windows x64/Arm64, Linux x64/Arm64, iOS 18+, Android 9+ —
  https://docs.x.ai/grok-bot/faq, https://docs.x.ai/grok-bot/overview.

**Architecture (from docs.x.ai/grok-bot/*):**
- *Execution*: one persistent cloud computer per account, shared by all of the user's Bots: shared files
  (`/workspace`), shared browser cookies and logged-in sessions; each Bot gets its own screen on that computer and
  can run one computer-use task at a time — https://docs.x.ai/grok-bot/computer-and-apps. The OS/VM technology is
  not documented.
- *Control*: browser + CLI on the cloud machine; user can open "Agent Computer" to watch clicks/typing/navigation live
  and **take over** for passwords, 2FA, CAPTCHAs, payment verification —
  https://docs.x.ai/grok-bot/computer-and-apps, https://docs.x.ai/grok-bot/faq.
- *Local machine*: the cloud computer is separate from your Mac/PC; running commands on the local computer requires
  that capability to be enabled **and** approved under a "local-computer policy" — https://docs.x.ai/grok-bot/computer-and-apps.
- *Tools*: "Connectors" installed as plugins from a Marketplace, attached with `@` in chat, account-wide; API
  connectors where available, computer use as fallback for sites "with no clean API or MCP" —
  https://docs.x.ai/grok-bot/computer-and-apps, https://x.ai/news/introducing-grok-bot. MCP support in Bot itself is
  not documented; the developer Voice Agent API does accept `mcp` tools (below).
- *Skills*: "A skill describes how to perform a task"; created via **"Teach a task"** — a screen recording of up to
  10 minutes that the Bot turns into a reusable skill — https://docs.x.ai/grok-bot/faq.
- *Routines*: scheduled runs; closing the app/laptop/phone does not stop a background turn or routine —
  https://docs.x.ai/grok-bot/faq.
- *Memory*: stable preferences, role context, summaries of prior work — https://docs.x.ai/grok-bot/overview.
- *Safety*: sensitive/consequential actions stop for approval; user-defined **Auto-review rules** decide which actions
  need approval; human-only steps via takeover — https://docs.x.ai/grok-bot/faq.
- *Voice*: "Start voice chat starts a live conversation on desktop, iPhone, and Android. Bots can also send a voice
  memo" — https://docs.x.ai/grok-bot/faq. Approval requests can arrive inside a call (**unverified**, runtimewire).
- *Model*: not named in Bot docs. Grok 4.6 (Aug 2026) and Grok 4.7 (Sep 2026, 500k context) are the current frontier
  models — https://docs.x.ai/developers/release-notes.

**xAI developer voice stack (what a Grok voice front-end most likely sits on):**
- Speech-to-speech Voice Agent API over WebSocket `wss://api.x.ai/v1/realtime?model=grok-voice-latest`, JSON events
  (OpenAI-Realtime-style); `turn_detection: {type: "server_vad"}` emits `input_audio_buffer.speech_started` and
  auto-creates responses; tools: `file_search`, `web_search`, `x_search`, `mcp`, `function`; custom/cloned voices;
  idle timeout that re-engages the user; BCP-47 language hint updatable mid-session; ephemeral tokens for clients —
  https://docs.x.ai/developers/model-capabilities/audio/voice-agent,
  https://docs.x.ai/developers/model-capabilities/audio/ephemeral-tokens.
- STT "Grok Voice Transcribe 2.0" default since Sep 2026; `grok-voice-transcribe-1.0` EOL 2026-10-02 —
  https://docs.x.ai/developers/release-notes.

**Worth borrowing:** shared-machine/multiple-screens model (many agents, one logged-in browser profile); "Teach a task"
by demonstration → skill; human takeover as a first-class step instead of the agent typing secrets; user-editable
Auto-review rules; voice as a *delegation channel* on top of the same permission model (voice never widens access).
**Improve on:** no local-first option, the shared cookie jar means one compromised Bot reaches every session.

### 1.2 "Meta Muse" — Meta Muse personal agent (model: Muse Spark)

**What it is.** Meta's consumer personal agent that runs long tasks (email, travel, bills, forms, purchases,
plans) in a dedicated cloud VM.
- Announced **2026-09-08** — https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/.
- Platforms at launch: iOS, Android, web (muse.ai), US only; free tier plus paid tiers —
  same URL. $20 "Power" / $100 "Maximum" tiers and WhatsApp access — **unverified**
  (https://techcrunch.com/2026/09/08/meta-debuts-its-muse-ai-agent-will-consumers-trust-it/).
- **Muse for Mac with computer use** ("with your permission, Muse can now drive any app on your Mac"; keeps working
  after you walk away) announced at Connect, **2026-09-23** —
  https://www.meta.com/blog/meta-connect-2026-everything-we-announced/.
- Glasses: "say your agent's name" (wake word = the user-chosen agent name) and Muse acts on what you are looking at;
  "coming months" — https://about.fb.com/news/2026/09/the-biggest-news-from-connect-2026/,
  https://www.meta.com/blog/muse-personal-agent-ai-glasses/.
- "Muse Charm" pocket device with "a state of the art real-time voice model"; Muse Realtime Voice + Muse Realtime
  Avatar; Muse gets its own email address — https://about.fb.com/news/2026/09/the-biggest-news-from-connect-2026/,
  https://www.meta.com/blog/meta-connect-2026-everything-we-announced/.
- Closed source. Model **Muse Spark** (announced 2026-04-08, updated 2026-05-12): closed weights, "hope to
  open-source future versions", API in private preview for select partners; multimodal (text+image), parallel
  sub-agents; Meta AI voice lets users "interrupt, switch topics, or swap languages" —
  https://about.fb.com/news/2026/04/introducing-muse-spark-meta-superintelligence-labs/. Muse launched on
  "Muse Spark 1.3" — **unverified** (TechCrunch).

**Architecture:**
- *Execution*: **Muse Secure VM** — a dedicated VM per user that holds both the agent and the user's data, with its
  own browser the user can watch — https://about.fb.com/news/2026/09/introducing-muse-personal-ai-agent/.
- *Safety*: a separate **Sentinel** agent gatekeeps approvals and internet requests; Sentinel runs on the same VM
  but isolated at system level (**unverified** detail, TechCrunch); encrypted credential storage where passwords are
  hidden from the agent; audit trail of all actions; approval before sensitive actions; planned "Confidential VM"
  with user-held keys; payments via Link wallet, Shop Pay later — vendor launch post above.
- *Connectors*: per-app choice of access level; Connect added Walmart, Best Buy, Gap, Sephora, Wayfair, Shop Pay,
  PayPal, Instacart, Notion, Granola, GitHub, Box, … — https://about.fb.com/news/2026/09/the-biggest-news-from-connect-2026/.
- *Memory*: remembers preferences; user can tell it to "forget" specific facts; training on queries by default with
  opt-out — vendor launch post.
- *Voice*: Realtime Voice where "Muse gets work done in the background while you're still talking"; voice style
  set by describing it (speed, accent) — https://www.meta.com/blog/meta-connect-2026-everything-we-announced/.
  Pipeline type (S2S vs cascade) not disclosed.
- Controversy: Amazon blocked Muse access on 2026-09-21 — **unverified** (Wikipedia
  https://en.wikipedia.org/wiki/Muse_(AI_agent)).

**Worth borrowing:** a *second, isolated policy agent* (Sentinel) that approves egress/actions instead of trusting
the acting model; secrets vault the agent can use but never read; agent-own email identity; wake word = the
agent's own name; "keep working while still talking" (talk thread and work thread decoupled).

### 1.3 "GPT Dots" — OpenAI "dots" (ChatGPT dots), always-on agents

**What it most likely is.** OpenAI **dots**, always-on agents inside ChatGPT, announced at DevDay
**2026-09-29** — https://openai.com/index/introducing-dots/ (403 to fetch; facts via search extracts of that page and
of https://help.openai.com/en/articles/20001530-getting-started-with-your-dot), https://openai.com/index/devday-2026-recap/.
Other candidates for "GPT Dots": none credible found. ⚠ **Do not use** `github.com/OpenAI-Dots/OpenAI-Dots`: a
user account created 2026-09-30 with 1 star, offering an "official free download" Windows zip — that is a classic
malware lure, not OpenAI (GitHub API, cd 2026-10-07; OpenAI's org is `github.com/openai`).

- Status: rolling out to Pro and Business Premium (first dot included), enterprise beta; EU/CH/UK excluded at
  first (**unverified**, press). Closed source.
- Model: **GPT-6 Astra** (system card https://deploymentsafety.openai.com/gpt-6-astra/).
- Each dot: persistent responsibility between conversations, **its own cloud computer + browser** the user can open
  to watch, can start work on its own (proactive), schedules/recurring checks — openai.com/index/introducing-dots
  and help article (search extracts).
- Tools: >4,000 apps via OpenAI plugins; with permission it can also work directly on the owner's laptop —
  vendor page; laptop control detail confirmed by https://siliconangle.com/2026/09/29/openai-launches-dots-always-on-ai-agents-in-chatgpt-with-their-own-cloud-computers/.
- Channels: message **or call** the dot in ChatGPT desktop/web/mobile; Slack and Teams; context follows the agent
  across channels; user can name it and pick an avatar/pet character — vendor page (search extract) + press.
- Safety: when doing proactive background research, connected apps are **read-only** (cannot send messages,
  change content, or control your browser/computer); installing software or changing a password needs approval;
  account-affecting/sharing actions pass an **auto-review** check; a monitor can pause/stop the agent — vendor page
  (search extract) + SiliconANGLE.

**OpenAI voice + agent plumbing behind it (developer side):**
- **GPT-Live 1** (GA 2026-09-10): full-duplex voice model that "can continue while a backend model or agent handles
  reasoning and tools"; endpoint `v1/live/sessions` only (not Realtime/WebRTC/SIP); audio+text in/out;
  function calling; $0.05/min — https://developers.openai.com/api/docs/models/gpt-live-1.md,
  https://developers.openai.com/api/docs/changelog.
- Realtime API (`gpt-realtime-2.1` is the previous generation per GPT-Live docs). Transcription: GPT Transcribe and
  GPT Live Transcribe (2026-07-28); `whisper-1` and `gpt-4o-*-transcribe` shut down 2027-02-26 — changelog above.
- **Realtime and GPT-Live wire protocols** (checked 2026-10-07; implemented in `aulo-realtime`, T8.10). Primary sources:
  OpenAI's API docs (markdown form: append `.md`), the API reference pages and the openai-python SDK.
  - Realtime WebSocket: `wss://api.openai.com/v1/realtime?model=gpt-realtime-2.1`, header `Authorization: Bearer <key>`,
    optional `OpenAI-Safety-Identifier`; JSON text messages, audio base64 inside them —
    https://developers.openai.com/api/docs/guides/voice-websockets.md. A browser WebSocket may use an ephemeral
    token in the subprotocols (`realtime`, `openai-insecure-api-key.<token>`) — same page.
  - Realtime client events used: `session.update` (`session.type: "realtime"`, `instructions`, `output_modalities`,
    `audio.input.turn_detection`, `audio.input.transcription.model`, `audio.output.voice`, `tools`, `tool_choice`),
    `input_audio_buffer.append` (`audio`, base64, at most 15 MiB, no acknowledgment), `response.cancel`,
    `conversation.item.create` with a `function_call_output` item, `response.create` —
    https://developers.openai.com/api/reference/resources/realtime/client-events.md and
    https://developers.openai.com/api/docs/guides/realtime-conversations.md.
  - Realtime server events used: `session.created`, `error` (`error.type`, `code`, `message`, `event_id`),
    `input_audio_buffer.speech_started` (`audio_start_ms`, `item_id`) and `speech_stopped` (`audio_end_ms`),
    `conversation.item.input_audio_transcription.delta` / `.completed` (`item_id`, `delta` / `transcript`,
    `languages[].code`), `response.output_audio.delta` (`delta`, base64), `response.output_audio_transcript.delta` /
    `.done` (`transcript`), `response.function_call_arguments.done` (`call_id`, `name`, `arguments`), `response.done` —
    https://developers.openai.com/api/reference/resources/realtime/server-events.md.
  - PCM format for Realtime: only 24 kHz mono 16-bit (`{"type":"audio/pcm","rate":24000}`); G.711 `audio/pcmu` and
    `audio/pcma` also exist — https://developers.openai.com/api/reference/resources/realtime/subresources/client_secrets/methods/create.md.
  - GPT-Live WebSocket: `wss://api.openai.com/v1/live/sessions`, no query parameters, same bearer header; the first
    message is `session.start` with `session.model`, `instructions`, `audio.format` (24 kHz or 16 kHz PCM, G.711 at
    8 kHz; one format for both directions), `audio.output.voice`, `delegation`; wait for `session.started`; audio in
    `session.input_audio.append`, out in `session.output_audio.delta` (no timing, no done event); transcript fragments
    `session.input_transcript.delta` / `session.output_transcript.delta` (`delta`, `start_ms`, `end_ms`, no turn
    marker); `session.usage.updated`; `session.close` answered by `session.closed` (`usage.seconds`, `reason`);
    `error` with `client_event_id` — https://developers.openai.com/api/docs/guides/voice-websockets.md (GPT-Live tab),
    https://developers.openai.com/api/docs/guides/live-conversations.md; path confirmed in openai-python v3.26.0
    `src/openai/resources/live/live.py` (`/live/sessions`) — https://github.com/openai/openai-python.
  - Ephemeral client secrets (Realtime): `POST https://api.openai.com/v1/realtime/client_secrets`, body
    `expires_after {anchor: "created_at", seconds: 10..7200, default 600}` and `session` (`type: "realtime"`, `model`,
    `audio.output.voice`, `instructions`); reply `value` (`ek_...`), `expires_at` (seconds since epoch), `session`;
    the session set on the secret can be overridden by the client connection; set `OpenAI-Safety-Identifier` on the
    minting request —
    https://developers.openai.com/api/reference/resources/realtime/subresources/client_secrets/methods/create.md,
    https://developers.openai.com/api/docs/guides/voice-webrtc.md, and `src/openai/resources/realtime/client_secrets.py`
    in openai-python v3.26.0. GPT-Live documents no client secrets: its browsers use WebRTC through a server
    (`POST /v1/live/sessions` with the SDP offer, key kept on the server) — voice-webrtc guide, GPT-Live tab.
  - Stale: the OpenAPI document `manual_spec` at https://github.com/openai/openai-openapi (checked 2026-10-07) lists only
    the old `/realtime/sessions` and `/realtime/transcription_sessions`; it has no `client_secrets` and no `/live`.
  - **Unverified** (docs silent): the close codes, the HTTP status of a bad key at the WebSocket handshake (assumed
    401), and whether a `response.output_audio.delta` can exceed 1 MiB (the client caps messages at 1 MiB).
- **Agents API** public beta 2026-09-10 (managed Codex harness, compaction, recovery); **computer use** in Agents
  API 2026-09-29 in an OpenAI-hosted browser with site-access approvals and sign-in handled by your app; remote MCP
  in Responses API since 2026-05-20 — changelog above. Agents SDK (Python + TS) has sandbox execution, handoffs,
  guardrails, tracing, MCP — https://openai.com/index/the-next-evolution-of-the-agents-sdk/ (search extract).
- Codex CLI can start and steer tasks by voice; voice agent speaks "checking your calendar…" fillers while tools run —
  https://openai.com/index/devday-2026-recap/ (search extract).
- Older: ChatGPT agent (2025-07) with confirmations for high-impact actions, prompt-injection monitoring and "watch
  mode" on sensitive sites (pauses if you leave the tab); logged-out mode — https://help.openai.com/en/articles/11752874-chatgpt-agent,
  https://help.openai.com/en/articles/12628199-using-ask-chatgpt-sidebar-and-chatgpt-agent-on-atlas (search
  extracts). Atlas browser later deprecated — **unverified**.

**Worth borrowing:** split *voice front model* (full-duplex, cheap) from *backend agent* (slow, tool-heavy) — the
front talks, the back works; read-only mode for unattended/proactive work, write mode only with a human present;
audible progress fillers while tools run; one agent identity reachable over many channels with shared context.

---

## Part 2. Other relevant agents (2025–2026)

### 2.1 Anthropic — Claude computer use (API), Cowork/Desktop computer use, Claude in Chrome, voice mode
- **API tool** `computer_toolset_20260801` (GA; older `computer_20251124` being phased out): 17 member actions
  (screenshot, zoom, clicks, drag, mouse down/up, type, key, hold_key, scroll, wait, cursor_position); model can
  batch several actions per turn — run sequentially, stop on first failure; pair with `bash_20250124` and
  `text_editor_20250728`; coordinates in screenshot pixels, scale for Retina; ≤20 screenshots per request
  recommended; screenshot classifiers scan for prompt injection —
  https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool.
  Reference impl: Docker + Xvfb + Mutter/Tint2 — https://github.com/anthropics/anthropic-quickstarts/tree/main/computer-use-demo.
- **Desktop computer use** (beta, Pro/Max, macOS + Windows, launched with **Dispatch** 2026-03-23 — assign tasks from
  the phone, finish on desktop): tool priority **connectors → browser → screen**; per-app permission prompts; some
  app categories (investment, crypto) blocked by default; user blocklist; on macOS 15+ works in **background windows
  without taking the pointer/keyboard** and waits while you type; no sandbox —
  https://claude.com/blog/dispatch-and-computer-use, https://support.claude.com/en/articles/14128542-let-claude-use-your-computer-in-cowork.
- **Claude in Chrome**: per-action "Manually approve" mode, per-site "Always allow", some sites always require
  approval, permission history — https://support.claude.com/en/articles/12902446-claude-in-chrome-permissions-guide.
- **Voice mode** (beta, all plans, mobile/desktop/web): hands-free (listens continuously, responds to pauses; barge-in
  by speaking) or push-to-talk; can use connectors (Gmail, Calendar, Docs, Slack) in voice —
  https://support.claude.com/en/articles/11101966-use-voice-mode. Claude Code has voice dictation —
  https://code.claude.com/docs/en/voice-dictation.
- **Borrow:** connectors-first/screen-last ladder; background-window control so the user keeps the machine; per-app
  allowlist + category blocklist; batched actions with stop-on-first-failure semantics.

### 2.2 Google — Gemini computer use, Chrome auto browse, Gemini Live API
- **Computer Use tool** in Gemini API: models Gemini 3.8 Flash (recommended), 3.7 Flash, 3.5 Flash(-Lite), 3 Flash
  Preview; environments `ENVIRONMENT_BROWSER`, `ENVIRONMENT_MOBILE` (open_app, list_apps), `ENVIRONMENT_DESKTOP`;
  **normalized 0–999 coordinates**; response carries `safety_decision` = `require_confirmation` for six policy
  categories (can be disabled); Playwright reference loop —
  https://ai.google.dev/gemini-api/docs/generate-content/computer-use. Computer use built into Gemini 3.5 Flash
  announced 2026-06-24, with optional confirmation and auto-stop on detected indirect prompt injection —
  https://blog.google/innovation-and-ai/models-and-research/gemini-models/introducing-computer-use-gemini-3-5-flash/.
  Computer Use + Shell sandboxes GA in Gemini Enterprise Agent Platform 2026-09-09 —
  https://docs.cloud.google.com/gemini-enterprise-agent-platform/release-notes (search extract).
- **Chrome auto browse** (Gemini 3, AI Pro/Ultra, US, preview; confirms sensitive actions) —
  https://blog.google/products-and-platforms/products/chrome/gemini-3-auto-browse/ (search extract).
- **Gemini 3.8 Live** + **3.8 Live Extended Thinking** (2026-09-15): native speech-to-speech, **asynchronous
  function calling** (tools run while audio keeps streaming; NON_BLOCKING mode), sees what the user sees, 97+
  languages; $0.005/min in, $0.018/min out; partners include LiveKit and Pipecat —
  https://blog.google/innovation-and-ai/technology/developers-tools/build-real-time-voice-applications-gemini-audio/,
  https://ai.google.dev/gemini-api/docs/live-api/tools.
- **Borrow:** normalized coordinates (resolution-independent); model-emitted `require_confirmation` merged with a
  local policy; async/non-blocking tool calls in a live voice session.

### 2.3 Microsoft — Windows agent workspace, Copilot Actions, MCP on Windows, "Hey Copilot"
- **Agent workspace** (experimental): each agent gets its **own Windows account and session/desktop** (runtime
  isolation, scoped authorization); default access to six known folders with per-agent Allow always / Ask every time /
  Never; tamper-evident audit logs; Copilot Actions runs there —
  https://support.microsoft.com/en-us/windows/ai/ai-features/experimental-agentic-features (last updated 2025-12-05).
- **MCP on Windows**: On-device Agent Registry (ODR) of "agent connectors" (MCP servers), contained by default,
  managed via Settings/Intune; built-in File Explorer and Settings connectors —
  https://learn.microsoft.com/en-us/windows/ai/mcp/overview, https://learn.microsoft.com/en-us/windows/ai/mcp/servers/mcp-containment.
- "Hey Copilot" wake word runs locally and only activates on the phrase; Copilot Vision shares screen in voice chat —
  https://support.microsoft.com/en-us/microsoft-copilot/getting-started-with-copilot-on-windows (search extract),
  https://support.microsoft.com/en-us/microsoft-copilot/using-copilot-vision-with-microsoft-copilot.
- **Borrow:** a separate OS user/session for the agent is the cheapest strong sandbox on a desktop; OS-level registry
  of MCP servers with per-agent grants.

### 2.4 OpenClaw (open source, local-first personal agent — the "Clawdbot-style" one)
- https://github.com/openclaw/openclaw — MIT, TypeScript, ~391k stars, latest **v2026.9.8 (2026-10-03)**, very active.
  Stewarded by the OpenClaw Foundation (501(c)(3)), no paid tier (README). Earlier names Clawdbot/Moltbot —
  **unverified** (not checked in primary source).
- Architecture: a local **Gateway** = control plane for sessions, tools, events, channels; Control UI / CLI / TUI;
  20+ chat channels (WhatsApp, Telegram, Slack, Discord, Signal, iMessage, …); companion apps/"nodes" on
  macOS/iOS/Android/Windows/Linux add voice, Canvas, camera, screen, device-local actions; models and agent harnesses
  (Claude, Codex, local) are swappable plugins; skills follow AgentSkills spec, distributed via ClawHub —
  README. Design rule "**trusted gateway, untrusted execution, deterministic policy**": credentials + policy stay in
  the Gateway; execution in sandboxes/nodes/cloud machines without standing credentials; tool denial is
  rule-based, approvals **fail closed**; sandbox off by default (`agents.defaults.sandbox.mode: "all"`, Docker or
  OpenShell backends); in-process native plugins mitigated by pinning and SDK boundaries; `openclaw skills verify` —
  https://docs.openclaw.ai/start/why-openclaw. Unknown DM senders must be paired/approved (README).
- **Voice ("Talk mode")**: native STT on Apple platforms via `SFSpeechRecognizer` (may use network), Android speech
  service; TTS providers ElevenLabs, local **MLX** (macOS, voice cloning), or system; `interruptOnSpeech` (barge-in,
  default true); end-of-turn by silence timeout (700 ms macOS/Android, 900 ms iOS); code-heavy replies are spoken as
  "look at the screen"; optional realtime S2S: OpenAI Realtime `gpt-realtime-2.1` over WebRTC, Google over
  WebSocket, GPT-Live; realtime tool calls routed through `realtime.brain` = `agent-consult` (through Gateway
  policy) / `direct-tools` / `none` — https://docs.openclaw.ai/nodes/talk.
- **Voice wake**: one global wake-word list owned by the Gateway (≤32 triggers; defaults `openclaw`, `claude`,
  `computer`), synced to nodes over WebSocket (`voicewake.changed`); macOS requires macOS 26+; Android on-device,
  foreground only, pauses when another feature owns audio; triggers can route to a session or agent —
  https://docs.openclaw.ai/nodes/voicewake.
- **Borrow:** the realtime voice model never calls tools directly — it *consults* the main agent, which enforces
  policy (`agent-consult`); wake-word → route-to-agent mapping; audio-ownership arbitration; "say less, show on screen"
  for code. **Improve:** sandbox off by default; in-process plugins.

### 2.5 Goose (open source, Rust, MCP-native)
- https://github.com/aaif-goose/goose — Apache-2.0, Rust, ~55k stars, latest **v1.53.0 (2026-10-02)**; moved from
  `block/goose` to the Agentic AI Foundation at the Linux Foundation (README). Desktop app (mac/Linux/Windows), CLI,
  API; 15+ providers incl. Ollama, ACP to reuse Claude/ChatGPT/Gemini subscriptions; 70+ MCP extensions (README).
- Uses **`rmcp` 3.4.1** and **`candle` 0.11** (workspace `Cargo.toml`); built-in MCP servers in `crates/goose-mcp`:
  `computercontroller`, `memory`, `peekaboo` (macOS screen capture/inspection), `autovisualiser`; **dictation** with
  local Whisper (`dictation/whisper.rs`, feature `local-inference`) or OpenAI-compatible / ElevenLabs transcription
  (`crates/goose/src/dictation/providers.rs`) — repo tree via GitHub API, cd 2026-10-07.
- **Borrow:** the closest Rust reference for an MCP-host agent with local STT; "custom distros" pattern.

### 2.6 Open Interpreter / 01
- https://github.com/openinterpreter/openinterpreter — now a **Rust fork of OpenAI Codex** optimized for low-cost
  open models (Kimi K3, GLM 5.3) with switchable "harness emulation" (`/harness`: native, claude-code, kimi-code,
  qwen-code…); Apache-2.0, latest `rust-v0.0.55` (2026-09-30) — README.
- https://github.com/OpenInterpreter/01 (voice interface for desktop/mobile/ESP32, AGPL-3.0) — **last push
  2024-11-01: unmaintained**.
- **Borrow:** harness emulation (prompt/tool-format per model family) to get the most out of cheap/local models.

### 2.7 UI-TARS Desktop / Agent TARS (ByteDance) and Agent S (Simular)
- https://github.com/bytedance/UI-TARS-desktop — Apache-2.0, TS, ~39k stars, pushed 2026-10-05, last release
  v0.3.0 (2025-11-04): native GUI agent on the UI-TARS VLM with **local and remote computer operators and browser
  operators**; Agent TARS CLI with MCP — README.
- https://github.com/simular-ai/Agent-S — Apache-2.0, Python; pure screenshot → click/type/scroll framework;
  Agent S3 72.6% OSWorld (2025-12-15), hosted "Sai" 73% on OSWorld 2.0 (2026-08-28, vendor claim) — README.
- **Borrow:** pluggable "operator" abstraction (local desktop / remote VM / browser) behind one agent loop.

### 2.8 Voice-agent frameworks (pipelines, not computer control)
- **Pipecat** https://github.com/pipecat-ai/pipecat — BSD-2, Python, latest **v1.12.0 (2026-09-26)**; frame-based
  composable pipelines; each pipeline is an agent (handoff, parallel fan-out, sidecars); transports Daily/LiveKit
  WebRTC, WebSocket; S2S services (Gemini Live, Nova Sonic, Azure Voice Live, OpenAI Realtime); Silero VAD, Krisp;
  Whisker debugger — README. **Smart Turn v3** end-of-turn model, BSD-2 — https://github.com/pipecat-ai/smart-turn,
  https://huggingface.co/pipecat-ai/smart-turn-v3.
- **LiveKit Agents** https://github.com/livekit/agents — Apache-2.0, latest **livekit-agents@1.8.5 (2026-10-06)**;
  mix STT/LLM/TTS or a realtime model in one `AgentSession`; **semantic turn detection** transformer (model under
  "LiveKit Model License", HF tag `license:other` — https://huggingface.co/livekit/turn-detector); native MCP;
  SIP telephony; agent handoffs — README. Rust client SDK https://github.com/livekit/rust-sdks (`livekit` crate 0.9.3).
- **Kyutai Unmute** https://github.com/kyutai-labs/unmute — MIT; cascade wrapper that makes any text LLM talk using
  Kyutai streaming STT/TTS over WebSockets (STT model `kyutai/stt-1b-en_fr`, TTS `kyutai/tts-1.6b-en_fr`,
  CC-BY-4.0) — README, HF API. Moshi (full-duplex S2S, Apache-2.0) https://github.com/kyutai-labs/moshi.
- **Handy** https://github.com/cjpais/Handy — MIT, Rust/Tauri, ~33k stars, **v0.9.8 (2026-10-03)**; offline
  push-to-talk STT: `cpal` + `rubato` + Silero VAD (`vad-rs`) + Whisper / **Parakeet V3** via `transcribe-rs`
  0.3.8 / `transcribe-cpp`, text injection via `enigo` 0.6.1, global keys via `rdev` fork and `handy-keys`, signed
  Tauri updater — README + `src-tauri/Cargo.toml`. The best ready Rust reference for the *input half* of a voice agent.

### 2.9 Browser-control agents/servers
- **Playwright MCP** https://github.com/microsoft/playwright-mcp — Apache-2.0, **v0.0.83 (2026-09-28)**; acts on
  Playwright **accessibility snapshots**, no vision model; README now notes coding agents prefer a token-cheaper
  **CLI + skill** form over MCP schemas — README.
- **Chrome DevTools MCP** https://github.com/ChromeDevTools/chrome-devtools-mcp — Apache-2.0, **v1.10.1
  (2026-09-23)**; Puppeteer/CDP, debugging + performance traces; sends usage stats and optional CrUX lookups
  (disable flags) — README.
- **browser-use** https://github.com/browser-use/browser-use — MIT, Python, **0.13.10 (2026-09-04)**; library, CLI,
  and hosted cloud — README.

---

## Part 3. Architecture patterns across the field

| Concern | What leaders do (2026) | Sources |
|---|---|---|
| Voice front end | Full-duplex S2S front model that delegates to a backend agent (GPT-Live 1, Gemini 3.8 Live async tools, Muse Realtime Voice "works while you talk"); cascade STT→LLM→TTS still common for local/any-LLM (Unmute, OpenClaw Talk, Pipecat, LiveKit) | 1.3, 2.2, 1.2, 2.4, 2.8 |
| Turn-taking | Server VAD (xAI, OpenAI); semantic end-of-turn models (LiveKit turn-detector, Smart Turn v3); silence timeout 700–900 ms (OpenClaw); barge-in on by default | 1.1, 2.8, 2.4 |
| Wake word | Local only; wake word = agent's name (Muse glasses, OpenClaw list, "Hey Copilot") | 1.2, 2.4, 2.3 |
| Where the agent runs | Cloud VM per user/agent (Grok Bot, Muse Secure VM, dots), separate OS account (Windows agent workspace), or the user's own desktop in background windows (Claude, Muse for Mac, dots with permission) | 1.x, 2.1, 2.3 |
| How it controls | Ladder: API connectors/MCP → browser via accessibility tree/DOM/CDP → screenshots + coordinates; human takeover for secrets/2FA/CAPTCHA | 2.1, 1.1, 2.9 |
| Safety | Per-app/per-site grants; category blocklists (finance, crypto); model-emitted `require_confirmation`; separate reviewer (Muse Sentinel, OpenAI/Grok "auto-review"); read-only mode when unattended; prompt-injection classifiers on screenshots; tamper-evident audit logs; credentials vaulted out of model reach | 2.1, 2.2, 1.2, 1.3, 1.1, 2.3 |
| Skills | Teach-by-demonstration (Grok "Teach a task"); AgentSkills spec + registry (OpenClaw/ClawHub); CLI+skill instead of heavy MCP schemas (Playwright) | 1.1, 2.4, 2.9 |
| Memory | Preferences + role + summaries of prior work; user can "forget" | 1.1, 1.2 |

---

## Part 4. Building blocks (prefer Rust)

Versions: crates.io API `max_stable_version` (or newest pre-release) and GitHub releases API, cd 2026-10-07.
"Maintained" = release or push within ~6 months. Dates for crates are the crate's `updated_at` on crates.io (≈ last publish).

### 4.1 Speech-to-text

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| whisper.cpp | https://github.com/ggml-org/whisper.cpp | v1.9.5, 2026-10-06 | yes | MIT, C/C++, Metal/CUDA |
| `whisper-rs` | https://codeberg.org/tazz4843/whisper-rs (moved from GitHub) | 0.16.0, 2026-03-12 | yes | bindings to whisper.cpp |
| **sherpa-onnx** (+ official `sherpa-onnx` crate) | https://github.com/k2-fsa/sherpa-onnx | v1.13.8, 2026-09-10 (crate 1.13.8) | yes, very active | Apache-2.0; one Rust crate covers streaming Zipformer, **Parakeet TDT**, Whisper, **Moonshine v2**, SenseVoice, Qwen3-ASR, plus VAD, KWS, TTS, diarization, speaker ID, speech enhancement — `rust-api-examples/examples/` |
| `sherpa-rs` (3rd-party) | https://github.com/thewh1teagle/sherpa-rs | 0.6.8, 2025-10-05 | slowing | superseded by official crate |
| Parakeet TDT 0.6B v3 | https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3 | modified 2026-08-05 | yes | CC-BY-4.0, 25 languages per HF tags incl. **ru** and uk; NeMo repo now `NVIDIA-NeMo/Speech` v3.0.0 2026-08-07 |
| `parakeet-rs` | https://github.com/altunenes/parakeet-rs | 0.3.8, 2026-09-23 | yes | ONNX via `ort` |
| `transcribe-rs` | https://github.com/cjpais/transcribe-rs | 0.3.12, 2026-10-02 | yes | multi-engine (Whisper, Parakeet, Moonshine…) used by Handy |
| Moonshine | https://github.com/moonshine-ai/moonshine | v0.1.5, 2026-08-24 | yes | streaming models (`moonshine-streaming-medium` MIT on HF); repo license NOASSERTION — check per model |
| Vosk | https://github.com/alphacep/vosk-api | v0.3.50, 2024-04-22 (push 2026-08) | stale releases | `vosk` crate 0.3.1 (2024-10) — avoid for new work |
| Kyutai STT | https://github.com/kyutai-labs/delayed-streams-modeling | no releases, push 2026-01 | moderate | streaming, has Rust server; CC-BY-4.0 weights |
| Apple SpeechAnalyzer/SpeechTranscriber | https://developer.apple.com/documentation/speech/speechanalyzer | macOS/iOS 26 | yes (OS) | on-device; Rust via `objc2-speech` 0.3.2 or a Swift helper |
| Remote: Deepgram | https://github.com/deepgram/deepgram-rust-sdk | `deepgram` 0.11.0, 2026-09-14 | yes | official Rust SDK |
| Remote: OpenAI | GPT Live Transcribe / GPT Transcribe | 2026-07-28 | yes | `whisper-1` shuts 2027-02-26 (changelog) |
| Remote: xAI | Grok Voice Transcribe 2.0 | 2026-09 | yes | docs.x.ai release notes |

### 4.2 Text-to-speech

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| Piper (new home) | https://github.com/OHF-Voice/piper1-gpl | v1.8.0, 2026-09-04 | yes | **GPL-3.0** now; old `rhasspy/piper` (MIT) archived; voices MIT on HF `rhasspy/piper-voices` |
| `piper-rs` + `espeak-rs` | https://github.com/thewh1teagle/piper-rs | 0.2.0 / 0.2.0, 2026-05-21 | yes | phonemizer via espeak-ng (GPL-3.0) — license impact |
| Kokoro-82M | https://huggingface.co/hexgrad/Kokoro-82M | weights Apache-2.0, 2025-04-10 | model frozen | run via sherpa-onnx (`kokoro_tts_en.rs`) or `kokoro-tts` crate 0.3.3 (2026-06-01) |
| sherpa-onnx TTS | (above) | 1.13.8 | yes | Kokoro, Kitten, Matcha, VITS/Piper, Pocket TTS, Supertonic, ZipVoice |
| espeak-ng | https://github.com/espeak-ng/espeak-ng | 1.52.0, 2024-12-12 (push 2026-09) | slow | GPL-3.0, robotic but tiny |
| System TTS | `tts` crate https://github.com/ndarilek/tts-rs | 0.26.3, 2024-07-06 | stale | wraps AVSpeech/SAPI/Speech Dispatcher; or call `say` / AVSpeechSynthesizer via objc2 |
| Kyutai TTS | https://huggingface.co/kyutai/tts-1.6b-en_fr | 2025-09-11 | moderate | streaming, CC-BY-4.0 |
| Chatterbox / CosyVoice / Orpheus / Dia | github.com/resemble-ai/chatterbox (MIT), QwenAudio/CosyVoice (Apache), canopyai/Orpheus-TTS, nari-labs/dia | pushes 2025-11…2026-07 | mixed | Python/GPU, heavier |
| Remote: ElevenLabs | `elevenlabs_rs` https://github.com/rwxbytes/elevenlabs_rs | 0.7.1, 2026-07-27 | community | no official Rust SDK found |
| Remote: OpenAI / xAI / Gemini TTS | `async-openai` 0.42.1 (2026-09-28) https://github.com/64bit/async-openai | — | yes | xAI Voice API is OpenAI-Realtime-shaped |

Kokoro catalog entry `kokoro-multi-lang-v1_0` (T7.9), checked 2026-10-07:

- Archive: release `tts-models` asset `kokoro-multi-lang-v1_0.tar.bz2` of https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models — GitHub API digest `sha256:c5f7e2d2caf082bc1d20fb70334a61d99d20b484500aad32e7cf84c128ea3298`, 349,906,910 bytes (asset updated 2026-09-08). Per-file sizes and SHA-256 in `crates/aulo-models/data/models.json` were computed from that archive (377 files, 401,239,297 bytes).
- Download: the same files from https://huggingface.co/csukuangfj/kokoro-multi-lang-v1_0 at commit `f7b96bb6bef5c5da4d3aa4f4e0498fbbf62dc78b`; all 377 sizes and hashes (LFS SHA-256, git blob SHA-1) matched the archive.
- Voice ids (sid 0–53): `scripts/kokoro/v1.0/generate_voices_bin.py` at https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.8, matching the model's `speaker2id` metadata. Languages and grades: https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md. No Russian voice.
- Licences: weights Apache-2.0 (https://huggingface.co/hexgrad/Kokoro-82M); `espeak-ng-data` GPL-3.0-or-later (spec §17 D9).

### 4.3 VAD, turn detection, wake word

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| Silero VAD | https://github.com/snakers4/silero-vad | v6.2.3, 2026-09-23 | yes | MIT; ONNX; in sherpa-onnx; `voice_activity_detector` crate 0.2.1 (2025-08) |
| **earshot** | https://github.com/pykeio/earshot | 1.2.2, 2026-08-19 | yes | pure-Rust streaming VAD, 16 ms frames @16 kHz; vendor claims 40× faster than Silero v6 / TEN VAD and more accurate (self-reported) |
| `webrtc-vad` | https://github.com/kaegi/webrtc-vad | 0.4.0, **2019-10-01** | no | legacy GMM VAD, avoid |
| TEN VAD | via sherpa-onnx (`ten_vad_remove_silence.rs`) | — | yes | alternative model |
| Smart Turn v3 | https://github.com/pipecat-ai/smart-turn | HF 2026-01-07 | moderate | BSD-2, semantic end-of-turn (audio) |
| LiveKit turn-detector | https://huggingface.co/livekit/turn-detector | 2026-02-11 | yes | text-based EOU, custom license |
| openWakeWord | https://github.com/dscripka/openWakeWord | v0.6.0, 2024-02-11 (push 2025-12) | low | code Apache-2.0 but **pre-trained models CC BY-NC-SA 4.0** (README); Rust port `oww-rs` 0.3.3 (2026-06-12) |
| Porcupine | https://github.com/Picovoice/porcupine | v4.0, 2025-12-11 | yes | needs Picovoice **AccessKey**; v4 bindings list has **no Rust** (`binding/`: android, dotnet, flutter, ios, java, nodejs, python, react(-native), web); `pv_porcupine` crate stale |
| `rustpotter` | https://github.com/GiviMAD/rustpotter | 3.0.2, **2023-10-01** | no | avoid |
| **sherpa-onnx KWS** | (above) `keyword_spotter.rs` | 1.13.8 | yes | open-vocabulary keywords (no per-word training), Apache-2.0 — best Rust wake-word option |

### 4.4 Audio I/O and echo cancellation

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| `cpal` | https://github.com/RustAudio/cpal | 0.18.2, 2026-08-16 | yes | CoreAudio/WASAPI/ALSA/PipeWire |
| `rodio` | https://github.com/RustAudio/rodio | 0.22.2, 2026-03-05 | yes | playback on top of cpal |
| `rubato` | https://github.com/HEnquist/rubato | 5.0.1, 2026-10-01 | yes | resampling to 16 kHz |
| `ringbuf` | https://github.com/agerasev/ringbuf | 0.5.2, 2026-09-13 | yes | lock-free audio buffers |
| `webrtc-audio-processing` | https://github.com/tonarino/webrtc-audio-processing | 2.1.0, 2026-05-13 | yes | WebRTC APM: AEC3, NS, AGC (C++ build) |
| `aec3` (pure Rust port) | https://github.com/RubyBit/aec3-rs | 0.4.0, 2026-09-16 | young | evaluate before relying on it |
| macOS Voice Processing I/O | `AVAudioInputNode.setVoiceProcessingEnabled` — https://developer.apple.com/videos/play/wwdc2019/510/, https://developer.apple.com/videos/play/wwdc2023/10235/ | OS | yes | system AEC/NS/AGC; needs both I/O nodes in VP mode, engine stopped when enabling, may change channel count |
| sherpa-onnx speech enhancement | GTCRN / DPDFNet examples | 1.13.8 | yes | denoise, not AEC |

### 4.5 Agent plumbing: LLM clients, MCP, plugins

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| **`rmcp`** (official MCP Rust SDK) | https://github.com/modelcontextprotocol/rust-sdk | **3.5.1, 2026-10-05** | yes | client+server, stdio/child-process/streamable HTTP, OAuth, protocol negotiation incl. new `server/discover` lifecycle |
| MCP spec | https://github.com/modelcontextprotocol/modelcontextprotocol/tree/main/schema | latest dated schema **2026-07-28** (prev 2025-11-25) | yes | — |
| `async-openai` | https://github.com/64bit/async-openai | 0.42.1, 2026-09-28 | yes | OpenAI incl. realtime types |
| `genai` | https://github.com/jeremychone/rust-genai | 0.6.5, 2026-10-05 | yes | multi-provider |
| `rig-core` | https://github.com/0xPlaygrounds/rig | 0.43.0, 2026-09-30 | yes | agent framework |
| `tokio-tungstenite` | https://github.com/snapview/tokio-tungstenite | 0.30.0, 2026-07-11 | yes | realtime WebSocket APIs |
| `livekit` | https://github.com/livekit/rust-sdks | 0.9.3, 2026-09-25 | yes | WebRTC client |
| `ort` | https://github.com/pykeio/ort | 2.0.0-rc.13, 2026-07-28 | yes (still RC) | ONNX Runtime for VAD/KWS/STT models |
| `candle-core` | https://github.com/huggingface/candle | 0.11.0, 2026-06-26 | yes | pure-Rust inference (Goose uses it) |
| `wasmtime` / `wasmtime-wasi` | https://github.com/bytecodealliance/wasmtime | **49.0.2, 2026-10-02** | yes | component model + WIT for sandboxed plugins; `component-model-async` evolving (v46 notes) — WASI P3 readiness **not verified** |
| `extism` | https://github.com/extism/extism | 1.30.0, 2026-06-04 | yes (slower) | simpler WASM plugin ABI, many guest PDKs |
| `jsonrpsee` | https://github.com/paritytech/jsonrpsee | 0.26.1, 2026-09-30 | yes | subprocess/WebSocket JSON-RPC plugins (or just use MCP over stdio) |
| `rquickjs` / `mlua` | github.com/DelSkayn/rquickjs, github.com/mlua-rs/mlua | 0.14.0 / 0.12.2 (2026-09/10) | yes | embedded scripting alternative |

### 4.6 Browser and desktop control

| Block | Repo | Latest | Maintained | Notes |
|---|---|---|---|---|
| `chromiumoxide` | https://github.com/mattsse/chromiumoxide | 0.9.1, 2026-02-25 (push 2026-04) | moderate | async CDP client, typed protocol |
| `headless_chrome` | https://github.com/rust-headless-chrome/rust-headless-chrome | 1.0.22, 2026-06-11 | yes | sync CDP |
| `fantoccini` / `thirtyfour` | github.com/jonhoo/fantoccini, github.com/stevepryde/thirtyfour | 0.22.1 / 0.37.5 | yes | WebDriver (incl. BiDi in thirtyfour) |
| `playwright` crate | https://github.com/octaltree/playwright-rust | 0.0.20, **2022-08-20** | no | use Playwright MCP / CLI as subprocess instead |
| Playwright MCP / Chrome DevTools MCP | see 2.9 | 0.0.83 / 1.10.1 | yes | spawn via `rmcp` child-process transport |
| `enigo` | https://github.com/enigo-rs/enigo | 0.6.1, 2025-08-28 (push 2026-10) | yes | synthetic mouse/keyboard, mac/Win/X11/Wayland(libei) |
| `xcap` | https://github.com/nashaofu/xcap | 0.9.8, 2026-08-01 | yes | screenshots per monitor/window |
| macOS AX | `objc2-application-services` 0.3.2 (AXUIElement), https://github.com/madsmtm/objc2; `accessibility`/`accessibility-sys` 0.2.0 (2025-03) https://github.com/eiz/accessibility; `macos-accessibility-client` 0.0.2 (trust prompt) | 2025-10 | yes (objc2) | read AX tree, perform AXPress; needs Accessibility + Screen Recording TCC grants |
| Linux AT-SPI | `atspi` https://github.com/odilia-app/atspi | 0.30.0, 2026-05-06 | yes | over `zbus` 5.19.0 |
| Windows UIA | `uiautomation` https://github.com/leexgone/uiautomation-rs | 0.25.1, 2026-09-04 | yes | or raw `windows` crate 0.62.2 |
| `accesskit` | https://github.com/AccessKit/accesskit | 0.25.1, 2026-09-25 | yes | **provider** side (makes *your* UI accessible); not for controlling other apps |
| `rdev` | https://github.com/Narsil/rdev | 0.5.3, 2023-06-26 | no (forks used, e.g. rustdesk-org) | global hotkeys; `handy-keys` 0.3.4 (2026-08-07) is a maintained alternative |
| `tauri` | https://github.com/tauri-apps/tauri | 2.12.1, 2026-10-01 | yes | desktop shell (Handy) |

### 4.7 Local speech-to-speech models (for a fully local full-duplex front end)
- Moshi (Kyutai) — https://github.com/kyutai-labs/moshi, Apache-2.0/MIT, repo has `rust/`, `moshi_mlx/` and Python implementations; push 2026-09.
- Qwen3-Omni-30B-A3B — https://huggingface.co/Qwen/Qwen3-Omni-30B-A3B-Instruct, `license:other`, 2025-09-22.
- Ultravox — https://github.com/fixie-ai/ultravox, MIT, v0.6 (2025-08-18), push 2025-12 (slowing).
- Sesame CSM — https://github.com/SesameAILabs/csm, Apache-2.0, last push 2025-05 (stale).

---

## Part 5. Conclusions and ideas to borrow (for a Rust voice computer-control agent)

1. **Two-brain voice design.** Front: cheap full-duplex/realtime voice (GPT-Live 1, Gemini 3.8 Live, xAI Voice
   Agent) or a local cascade. Back: the real agent with tools. The front never executes tools directly; it consults
   the back, which enforces policy (OpenClaw `agent-consult`, GPT-Live "delegate to backend agent"). Speak progress
   fillers while tools run (DevDay 2026), and "look at the screen" instead of reading code.
2. **Local cascade in Rust is fully buildable today**: `cpal` → macOS VP-IO or `webrtc-audio-processing` (AEC) →
   `earshot` or Silero (VAD) → sherpa-onnx KWS (wake word = agent's name) → sherpa-onnx Parakeet/Moonshine streaming
   STT (or Apple SpeechAnalyzer) → LLM → sherpa-onnx Kokoro TTS (Apache weights; avoid Piper's GPL if that matters)
   → `rodio`. Add a semantic end-of-turn model (Smart Turn v3, BSD-2) on top of VAD; barge-in on by default.
   One crate (`sherpa-onnx` 1.13.8) covers VAD+KWS+STT+TTS — fewest moving parts.
3. **Control ladder**: MCP/connectors (`rmcp` 3.5.1) → browser via accessibility tree (Playwright MCP / CDP via
   `chromiumoxide`) → OS accessibility tree (AX / AT-SPI / UIA) → screenshot + vision + `enigo` as last resort. Use
   normalized coordinates (Gemini 0–999) to stay resolution-independent; batch actions with stop-on-first-failure.
4. **Isolation options**, cheapest first: background-window control on the user's desktop (Claude, macOS 15+);
   separate OS user/session (Windows agent workspace idea, portable to macOS as a second user + Screen Sharing);
   VM/container (Grok Bot, Muse Secure VM, dots). Human takeover for passwords/2FA/CAPTCHA — never let the agent type
   secrets; vault credentials out of model reach (Muse).
5. **Permission model** that holds up: per-app and per-site grants (Allow once / Always / Never), category blocklist
   (finance, crypto), deterministic fail-closed policy outside the model (OpenClaw), a separate reviewer step for
   consequential actions (Muse Sentinel, "auto-review" at OpenAI/Grok), **read-only mode when unattended** (dots),
   tamper-evident audit log (Windows). Voice must not widen access (Grok Bot).
6. **Skills**: record-a-demo → skill (Grok "Teach a task"); AgentSkills-style folders; prefer CLI+skill over huge MCP
   schemas for token cost (Playwright MCP README).
7. **Plugins**: MCP over stdio as the default extension ABI (every product above speaks it); WASM components via
   `wasmtime` 49 for untrusted in-process plugins; avoid in-process native plugins (OpenClaw's weak spot).
8. **Avoid / stale**: `webrtc-vad` (2019), `rustpotter` (2023), `playwright` crate (2022), `vosk` releases (2024),
   `OpenInterpreter/01` (2024), Porcupine for Rust (no binding, AccessKey), openWakeWord pre-trained models in
   commercial use (CC BY-NC-SA), the fake `OpenAI-Dots` GitHub repo.

---

## Part 6. Reusable code in the listepo workspace

Read from the local checkouts on 2026-10-07 (primary source: the repositories' own files). Paths are relative to
`/Users/listepo/GitHub/listepo/`. GitHub homes: `pyrlyn/cox`, `pyrlyn/runa`, `pyrlyn/rtok`, `pyrlyn/ketch`.

### 6.1 cox (terminal coding agent, Rust, GPL-3.0-or-later OR LicenseRef-cox-Royalty-Free — `apps/cox/Cargo.toml`)

None of the cox crates is on crates.io; internal deps use `path` + `version = "0.1.2"`. Crate ownership:
`apps/cox/AGENTS.md:28-62`.

| What | Where | Notes |
| --- | --- | --- |
| Provider trait | `apps/cox/crates/cox-protocol/src/traits.rs:172-197` | `stream(req, sink, cancel) -> Usage`; `Caps`, `count_tokens` |
| Neutral types | `apps/cox/crates/cox-protocol/src/types.rs:1566` (`Request`), `:1594` (`ProviderEvent`), `:1658` (`Caps`), `:1673` (`ToolSpec` with `Risk`, `Concurrency`) | Streamed text and tool calls on every wire |
| Submission / Event | `types.rs:976`, `types.rs:1147` | The event-driven core every surface consumes |
| Wires | `cox-provider-openai` (Chat, Responses), `cox-provider-anthropic` (Messages, typify types), `cox-provider/src/lmstudio.rs` | OpenAI-compatible vendors need no code (`apps/cox/config/default.toml:45-145`) |
| Credentials | `apps/cox/crates/cox-provider-http/src/http.rs:53-77` | env var first, then keyring `cox/<section>` |
| One provider per session (limitation) | `apps/cox/crates/cox-session/src/provider.rs:27-120` | Tiers only change the model |
| Model catalog | `apps/cox/crates/cox-models/src/catalog.rs:247` (`Catalog::load`), `:294` (`get`) | Built-in < config < prices.toml + served overlay |
| Agent loop | `apps/cox/crates/cox-core/src/session.rs:289` (`Session::new`), `:908` (`submit`) | Loop states and rules: `apps/cox/plan.md:315-347` |
| Tool trait | `apps/cox/crates/cox-protocol/src/traits.rs:242` | `spec`, `subject`, `risk`, `touches`, `call(input, &ToolCx)` |
| Permissions | `apps/cox/crates/cox-permission/src/lib.rs:115` (`decide`) | Claude Code rule grammar; decision order `apps/cox/plan.md:560-576` |
| Bash risk classifier | `apps/cox/crates/cox-tools/src/bash/classify.rs` | tree-sitter |
| Sandbox | `apps/cox/crates/cox-sandbox/src/sandbox/mod.rs:87` (`command`), `:126` (`argv`), `src/path.rs:41` (`confine`) | Seatbelt, bwrap, Landlock+seccomp |
| Sanitizer | `apps/cox/crates/cox-sanitize/src/lib.rs:33` | Terminal escapes and bidi |
| MCP client and server | `apps/cox/crates/cox-mcp/src/client.rs:244` (`connect`), `:357` (`tools`), `:442` (`connect_all`); `discovery.rs`, `auth.rs`, `elicit.rs`, `server.rs` | rmcp 3.2 (lock 3.5.0); failing server becomes a notice |
| Hooks, skills | `apps/cox/crates/cox-ext/src/hooks.rs` (`HookChain`:123), `skills.rs`; hook events `cox-protocol/src/types.rs:488` | Claude Code JSON protocol |
| WASM plugins | `apps/cox/crates/cox-plugin` (extism 1.30, wasmtime 43), `grant.rs` | Grants tied to the package digest, per-call deadline and memory cap |
| Voice (push-to-talk) | `apps/cox/crates/cox-voice/src/capture.rs:36,68`, `transcribe.rs:29,55`, `lib.rs:85`; trait `cox-protocol/src/traits.rs:728` | whisper-rs 0.16, cpal 0.18, rubato 5; pinned model hashes in `data/whisper-models.json` |
| Browser trait (host-supplied) | `apps/cox/crates/cox-app/src/browser.rs` | `browser_open`, `browser_read`, `browser_screenshot` |
| Config, store, telemetry | `cox-config` (figment, provenance, schema drift test), `cox-store` (Diesel + SQLite), `cox-telemetry` | Pattern for aulo-config/store/telemetry |
| Crate-boundary test | `apps/cox/crates/cox/tests/deps.rs` | Pattern for T2.2 |

### 6.2 runa (local-first AI runner, Rust, MIT OR Apache-2.0 — `apps/runa/Cargo.toml`)

| What | Where | Notes |
| --- | --- | --- |
| OpenAI/Anthropic-compatible server | `apps/runa/crates/runa/src/serve.rs:143-149` | `/v1/chat/completions`, `/v1/messages`, `/v1/embeddings`, `/v1/audio/transcriptions`; axum 0.8, port 8080 |
| Streaming caveat | `serve.rs:548` (`generate_events` returns `Vec<GenEvent>`), `serve.rs:330-339` (replayed as SSE) | Time to first token = full generation time (T1.19) |
| Engine | `apps/runa/crates/runa-engine/src/load.rs:685`, `generate.rs:134,146` | llama-cpp-2 =0.1.133, optional mistralrs =0.8.1 |
| Fit checker | `apps/runa/crates/runa-fit` (`verdict.rs`, `planner.rs`) | GGUF header reader, hardware probe |
| ASR and audio | `apps/runa/crates/runa-media/src/asr.rs:100,300-325,441` | whisper-rs, energy VAD, decode/resample, WER helper; Parakeet is a stub |
| Cloud clients | `apps/runa/crates/runa-cloud/src/openai.rs:131,150`, `anthropic.rs:147`, `secrets.rs` | Collect-then-return, not incremental; `reject_inline_secrets` |
| MCP client | `apps/runa/crates/runa/src/mcp.rs` | stdio only, `pub(crate)` |
| Bin-only crate | `apps/runa/crates/runa` | serve/pool/mcp are `pub(crate)` (T1.20) |

### 6.3 rtok, ketch, packages

| What | Where | Notes |
| --- | --- | --- |
| MCP server over stdio | `apps/rtok/src/mcp.rs:61` (`run`), `:573`, `:591`, `:657` (`call_tool`) | rmcp 3.2 model types |
| Register MCP in agent hosts | `apps/rtok/crates/rtok-agent-sdk/src/lib.rs:84` (`backup`), `:325` (`write_atomic`), `:377` (`register_mcp`), `:424` (`unregister_mcp`) | Foreign-config rule in practice |
| Plugin SDK and WASM host | `apps/rtok/crates/rtok-plugin-sdk/src/lib.rs:328` (`trait Plugin`), `apps/rtok/src/plugins/wasm.rs:74,101,243` | wasmi host behind feature `wasm-host` |
| Hooks with fail-open | `apps/ketch/crates/ketch-core/src/hooks.rs:96,100,144` | `run_or_warn` |
| Resident IPC | `apps/rtok/crates/rtok-hook/src/lib.rs:1-6` | Length-prefixed frames over UDS / named pipe |
| Process capture | `apps/rtok/src/proc.rs:34` | Never waits on pipe EOF |
| Single-instance lock, kill process group | `apps/rtok/crates/rtok-sys/src/lib.rs:11` | The only crate allowed `unsafe` |
| Layered config | `apps/rtok/src/config/layers.rs:379,504,584`; `apps/ketch/crates/ketch-core/src/toml_file.rs:21,27,105` | Known duplicate to extract (T1.14) |
| Sanitizer | `apps/rtok/src/sanitize.rs:34,70,82` | Duplicate of cox-sanitize and ketch changelog::sanitize |
| Rotating log | `apps/rtok/crates/rtok-log/src/lib.rs:23,76,121` | |
| Storage | `apps/rtok/src/store/mod.rs` | One SQLite file, WAL, FTS5, Diesel |
| Signature checks | `apps/ketch/crates/ketch-core/src/trust.rs` | sigstore / minisign / pgp (T11.11) |
| File backup | `packages/crates/file-backup/src/lib.rs:19,31` | crates.io 0.1.1 |
| CI templates | `packages/infra/.github/workflows/ci-rust.yml`; `packages/crates/.github/workflows/{ci,bump,release,pipeline}.yml`; `apps/ketch/.github/workflows/ci.yml:28-63` | Linux x86_64/aarch64, macOS aarch64, Windows x86_64 |
| Release | `apps/ketch/dist-workspace.toml` | cargo-dist 0.32.0, Homebrew tap |

### 6.4 Not found anywhere in the workspace

TTS, wake-word detection, streaming ASR, Silero VAD, gRPC, browser automation, desktop accessibility control, and
a tray/desktop UI written in Rust (ketch and cox desktops are SwiftUI/WinUI over UniFFI). These are new code in aulo,
built on the blocks in Part 4.
