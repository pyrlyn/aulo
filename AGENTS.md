# aulo — instructions for agents

**What this is.** A local-first voice agent that controls the computer: speech in and out with switchable engines, shell/app/browser/desktop control, MCP client and server, plugins, local and remote models. One daemon (`aulod`) serves a gRPC API to the desktop chat app, the CLI and remote clients.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `spec.md` — the specification. Section numbers are stable; tasks cite them.
- `research.md` — sources for every external fact and the reuse map of the workspace (Part 6).
- `plan.md` — active tasks. S0 decisions (T0.1–T0.7) block most work; ask the creator, do not decide them yourself.

## Hard rules

- **Policy runs outside the model.** Every tool call — built-in, MCP, plugin or a call made through aulo's own MCP server — passes `aulo-policy`. No tool runs before policy returns Allow. Policy fails closed.
- **Voice never widens access.** Destructive, payment and credential actions need a click or key, never only a spoken yes.
- **Secrets never reach the model.** Tools take secret handles; values never appear in prompts, transcripts or logs.
- **Everything from a model, a tool, a web page, a screenshot, an MCP server, a plugin or a remote client is untrusted input.** Wrap it, sanitize it, cap it, and test the cap.
- **Fail open on extensions.** A broken STT/TTS engine, MCP server, plugin, skill or hook is warned about and skipped; voice falls back to the next engine.
- **No unbounded buffers on the audio path.** Audio callbacks never block or allocate; ring buffers are fixed size and overflow is counted.
- **Reuse before writing.** Provider stack, permissions, sandbox, MCP host, speech capture, config helpers and the agent loop come from the shared crates planned in S1. Do not copy code from cox, runa or rtok into aulo.
- **gRPC is the API.** Protos in `proto/aulo/v1`; `buf lint` and `buf breaking` must pass.
- **Licences.** Check the licence of every engine and model before wiring it. GPL code (for example Piper) only runs as a separate plugin process.

## Layout

- `crates/` — the Cargo workspace members (`aulo-<role>`), created by S2+ tasks.
- `proto/aulo/v1/` — gRPC service definitions (T3.1).
- `plugins/` — plugin SDK examples (T11.5).
- `docs/` — guides, threat model, spike reports (`docs/spikes/`).
- `scripts/sync-issues.py` — creates GitHub issues from `plan.md` (dry run by default).
