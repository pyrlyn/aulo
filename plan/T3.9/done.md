# T3.9 done

`aulod service install|uninstall|status` installs aulod as a service of the logged-in user, in PR https://github.com/pyrlyn/aulo/pull/107 (stacked on #106).

- `aulo/app/service`: launchd (LaunchAgent `dev.aulo.aulod`), systemd (`aulod.service` user unit) and Task Scheduler (task `aulod` from UTF-16 XML) backends. They reach the machine only through `ServiceManager`; `OsServiceManager` runs the tools with pyrlyn-kit's bounded `Command` and caps definition reads at 64 KiB.
- The service runs `<aulod> run --home <home>`; `--home` is a new global option ahead of `AULO_HOME`, so no backend needs an environment.
- Install is idempotent: an unchanged running service is left alone, a changed definition restarts it, a stopped one is started.
- Logs: `<home>/logs/aulod.service.log` on macOS and Linux. A Task Scheduler task cannot redirect output, so on Windows aulod's own logs (T2.7) are the record.
- Design ported from cofre's `owner-listener` service module. pyrlyn-kit's Windows command-line quoting is internal, so `TaskScheduler.windowsArg` repeats that small rule.
- One label per user: installing from a second home replaces the first home's service.
- Sources: research.md §4.13; the encoding `/XML` accepts and the localisation of `/Query` remain unverified.

Checked: ktlint, `checkModuleGraph`, aulo tests on macOS arm64 and on Linux x64/arm64 under Colima (22/22 each), mingwX64 test and aulod link, CI run 45181072100. `FakeServiceManager` covers all three backends; a real read-only `aulod service status` on macOS printed "the aulod service is not installed". No real service was installed.
