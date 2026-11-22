# aulo

https://github.com/pyrlyn/aulo

A local-first voice agent that controls the computer (shell, apps, browser, desktop), speaks MCP both ways, takes plugins, and runs on local or remote models. A daemon (`aulod`) serves a gRPC API to the desktop chat app, the CLI and remote clients. The implementation stack is **Kotlin/Native** (Gradle KMP, native targets). See `spec.md` for the specification and `research.md` for the sources.

## Reset (2026-11-22)

The project starts from zero on the Kotlin/Native stack. Every previously closed task is
open again: all `done.md` closing reports were removed, and every `task.md` front matter
`status` is `todo`. Implementation notes that described the retired Rust workspace were
cleared from the cards. Task ids, titles, stages and acceptance criteria stay; work is
redone in Kotlin/Native.

## Tasks

Every task is a folder `plan/<id>/`:

- `task.md` — the task: front matter (`status`, `priority`, `complexity`, `readiness`, `agent`) on active tasks, then `# <id>. Title`, the `Stage · Area · Depends on · Blocks` line, why it exists and what done means.
- `done.md` — the closing report. A folder with `done.md` is a closed task; one without it is active. After the reset, no folder has a `done.md`.

Task ids are `T<stage>.<n>`; the stages are listed below and described in `spec.md` §16. List the active tasks:

```bash
for d in plan/*/; do [ -f "$d/done.md" ] || echo "${d%/}"; done
```

`scripts/sync-issues.py` creates GitHub labels, milestones and issues for the active tasks (dry run by default).

## Stages

The milestone names `scripts/sync-issues.py` creates.

- S0. Decisions and spikes
- S1. Shared Kotlin modules
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

T16.1–T16.8 are audit findings from 2026-10-07, now open again so they can be re-checked
on the Kotlin/Native implementation. T900 is the dependency-update task.
