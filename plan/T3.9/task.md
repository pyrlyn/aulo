# T3.9. Run aulod as a user service

Stage: S3 · Area: api · Depends on: T3.6 · Blocks: 0 task(s)


aulod service install/uninstall: launchd agent on macOS, systemd user unit on Linux, Task Scheduler entry on Windows.

Done when:

- install and uninstall are idempotent (tested with a fake service manager)
