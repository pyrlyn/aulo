# Toolchain

Only what the project uses today. The Rust toolchain, crates and build tools are added by the task that wires each of them in (T2.1 onwards); the planned stack is in `spec.md` and `research.md` Part 4.

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| python3 | system / mise | Runs `scripts/sync-issues.py` (stdlib only) | https://github.com/python/cpython |
| gh | brew / mise | Creates labels, milestones and issues from `plan.md` | https://github.com/cli/cli |
