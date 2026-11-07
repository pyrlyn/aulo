# aulo

https://github.com/pyrlyn/aulo

A local-first voice agent that controls the computer (shell, apps, browser, desktop), speaks MCP both ways, takes plugins, and runs on local or remote models. A daemon (`aulod`) serves a gRPC API to the desktop chat app, the CLI and remote clients. See `spec.md` for the specification and `research.md` for the sources.

## Tasks

Every task is a folder `plan/<id>/`:

- `task.md` — the task: front matter (`status`, `priority`, `complexity`, `readiness`, `agent`) on active tasks, then `# <id>. Title`, the `Stage · Area · Depends on · Blocks` line, why it exists and what done means.
- `done.md` — the closing report. A folder with `done.md` is a closed task; one without it is active.

Task ids are `T<stage>.<n>`; the stages are listed below and described in `spec.md` §16. List the active tasks:

```bash
for d in plan/*/; do [ -f "$d/done.md" ] || echo "${d%/}"; done
```

`scripts/sync-issues.py` creates GitHub labels, milestones and issues for the active tasks (dry run by default).

## Stages

The milestone names `scripts/sync-issues.py` creates.

- S0. Decisions and spikes
- S1. Shared crates in packages/
- S2. Workspace foundations
- S3. gRPC API and daemon
- S4. Models, providers and agent loop
- S5. Safety core
- S6. Audio I/O
- S7. Speech engines
- S8. Voice conversation
- S9. Computer control
- S10. MCP
- S11. Plugins, skills and hooks
- S12. Desktop apps (SwiftUI and WinUI)
- S13. Server mode
- S14. Bots, routines and memory
- S15. Release and docs

## Audit 2026-10-07

Findings from a code audit on 2026-10-07, filed as T16.1–T16.7. Verified-clean areas worth noting: strict input caps, constant-time token compare, pinned SHA-256 model downloads, XML/systemd/schtasks escaping, secret redaction at every sink, endpoint and model-path validation.

## Migration notes

- The tasks moved here from `plan.md`, `todo.md` and `done.md`. Where a closed task had both a `plan.md` card and a `done.md` entry written differently, `task.md` keeps the card and `done.md` keeps the entry's report followed by the other wording.
- `done.md` held two different tasks under `T16.7`; the second, "Isolate credential-list fixtures from developer credentials", is now T16.8.
- T4.6, T4.7, T5.1 and T8.1 were marked done in the `plan.md` table without a `done.md` entry; their `done.md` is the card's Outcome.
