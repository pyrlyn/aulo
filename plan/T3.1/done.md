# T3.1. Done

The `aulo.v1` service definitions. PR: https://github.com/pyrlyn/aulo/pull/96, stacked on #95.

- **No proto change.** `proto/aulo/v1` (10 files), `proto/buf.yaml` (lint `STANDARD`, breaking `FILE`), `buf.gen.yaml` and `.github/workflows/proto.yml` are language-neutral and survived the reset to Kotlin/Native (#81). This task verified them and closed.
- **Services.** Eight services with 37 RPCs: ChatService 7, SessionService 1, VoiceService 5, ApprovalService 4, ConfigService 7, McpService 5, PluginService 5, AuditService 3.
- **Not in the protos.** `grpc.health.v1` and reflection in `spec.md` §11 are standard services that the server registers (T3.3).

How it was checked, with buf 1.73.0:

- `buf lint proto` passes; `buf format proto --diff --exit-code` shows no diff; `buf build proto` passes.
- Every service and RPC name was compared with the table in `spec.md` §11: they match one to one.
- `buf breaking proto --against '.git#ref=origin/main,subdir=proto'` passes.
- `buf generate proto` leaves `docs/api/aulo.v1.md` unchanged.
- `proto.yml` runs lint, format and breaking on a pull request that touches `proto/**`. This pull request does not touch it, so the workflow did not run here.
