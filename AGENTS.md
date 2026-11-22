# aulo — instructions for agents

**What this is.** A local-first voice agent that controls the computer: speech in and out with switchable engines, shell/app/browser/desktop control, MCP client and server, plugins, local and remote models. One daemon (`aulod`) serves a gRPC API to the native desktop apps (SwiftUI, WinUI), the CLI and remote clients.

The implementation stack is **Kotlin/Native** (Gradle KMP, native targets). The project has
been reset: every task in `plan/` is open.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `spec.md` — the specification. Section numbers are stable; tasks cite them.
- `research.md` — sources for every external fact and the reuse map of the workspace (Part 6).
- `plan/` — one folder per task (`plan/<id>/task.md`, closed ones also `done.md`); `plan/README.md` says how to read it. Settled decisions D1–D9 are recorded in `spec.md` §17. Never decide an open question in `spec.md` §17 yourself; ask the creator.

## Hard rules

- **Policy runs outside the model.** Every tool call — built-in, MCP, plugin or a call made through aulo's own MCP server — passes `aulo-policy`. No tool runs before policy returns Allow. Policy fails closed.
- **Voice never widens access.** Destructive, payment and credential actions need a click or key, never only a spoken yes.
- **Secrets never reach the model.** Tools take secret handles; values never appear in prompts, transcripts or logs.
- **Everything from a model, a tool, a web page, a screenshot, an MCP server, a plugin or a remote client is untrusted input.** Wrap it, sanitize it, cap it, and test the cap.
- **Fail open on extensions.** A broken STT/TTS engine, MCP server, plugin, skill or hook is warned about and skipped; voice falls back to the next engine.
- **No unbounded buffers on the audio path.** Audio callbacks never block or allocate; ring buffers are fixed size and overflow is counted.
- **Reuse before writing.** Provider stack, permissions, sandbox, MCP host, speech capture, config helpers and the agent loop come from the shared Kotlin modules planned in S1. Do not copy code from the coding agent, the local model runner or the token-reduction CLI into aulo.
- **gRPC is the API.** Protos in `proto/aulo/v1`; `buf lint` and `buf breaking` must pass.
- **Licences.** Check the licence of every engine and model before wiring it. GPL code (for example Piper) only runs as a separate plugin process. The one exception is espeak-ng, linked by sherpa-onnx (spec §17 D9).

## Layout

- Gradle KMP modules (`aulo-<role>`), created by S2+ tasks, hold the Kotlin/Native daemon and CLI.
- `desktop/macos` (SwiftUI) and `desktop/windows` (WinUI 3) — native apps over `aulo-ffi` (T12.3, T12.15).
- `proto/aulo/v1/` — gRPC service definitions (T3.1).
- `plugins/` — plugin SDK examples (T11.5).
- `docs/` — guides, threat model, spike reports (`docs/spikes/`). Historical spike reports may describe the retired Rust workspace; new work follows this file and `spec.md`.
- `scripts/sync-issues.py` — creates GitHub issues from the active tasks in `plan/` (dry run by default).

## Commands

jactionlint and hk are pinned in `mise.toml`. `.github/jactionlint.yaml` applies the default profile and the
baseline `.github/jactionlint-baseline.json`. `hk.pkl` runs jactionlint as a pre-push hook:

```bash
mise install                               # JDK 21 and the lint tools
mise x -- jactionlint                      # lint workflows, composite actions, dependabot.yml
mise x -- jactionlint --baseline-write     # refresh the baseline after fixing findings
mise x -- hk install --mise                # install the pre-push hook once per clone
just api-docs                              # regenerate docs/api from proto/
```
