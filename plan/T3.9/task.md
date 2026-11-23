---
status: in progress
priority: P0
complexity: 3
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T3.9. Run aulod as a user service

Stage: S3 · Area: api · Depends on: T3.6 · Blocks: 0 task(s)

aulod service install/uninstall: launchd agent on macOS, systemd user unit on Linux, Task Scheduler entry on Windows.

Done when:

- install and uninstall are idempotent (tested with a fake service manager)

## Execution plan

The design follows cofre's `owner-listener` service module (Rust): every backend talks only to
a `ServiceManager` (read, write, remove, make a directory, run a program), so tests drive all
three against a fake on any host. Nothing is copied; kotlin-kit gives `currentExe()` and
`runBounded`.

1. `aulod run --home DIR`: the home for this run, ahead of `AULO_HOME`. Every backend starts
   `<exe> run --home <home>` with the home of the install, so the service needs no environment
   (a Windows task has none).
2. `aulo/app/service`: `ServiceManager` and the real `OsServiceManager` (no shell, bounded
   output and time), `ServiceSpec` (exe, home, names; control characters refused), and three
   backends with `install` (Fresh, Updated, Unchanged), `uninstall` (true, false) and `status`:
   - macOS: `~/Library/LaunchAgents/<label>.plist`, `launchctl bootstrap|bootout|print` in
     `gui/<uid>`; stdout and stderr to `<home>/logs/aulod.service.log`.
   - Linux: `~/.config/systemd/user/aulod.service`, `systemctl --user daemon-reload`,
     `enable --now`, `restart` when the unit changed while running, `disable --now`.
   - Windows: a Task Scheduler task with a logon trigger for the installing user, created from
     XML (no run-time limit, no battery stop, restart on failure); output relies on aulod's own
     logs (T2.7).
3. `aulod service install|uninstall|status`: prints what happened; a repeat install of the same
   definition changes nothing; uninstall of nothing says so and exits 0.
4. Tests: a fake service manager that models each manager's state; install twice, change the
   binary, uninstall twice, a failing tool, rendered files (quoting of paths with spaces,
   quotes, `%`, `$`, `&`); command fixtures for the CLI.
5. Verify: macOS, linuxX64 and linuxArm64 on Colima, mingwX64 link, ktlint, module graph; CI.
