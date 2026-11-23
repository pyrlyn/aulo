---
status: in progress
priority: P0
complexity: 2
readiness: 100%
agent: Claude Code / claude-fable-5-2
---

# T3.1. aulo-proto: v1 service definitions

Stage: S3 · Area: api · Depends on: T2.4 · Blocks: 2 task(s)

proto/aulo/v1/*.proto: ChatService (Create/List/Get/Rename/Delete chats, ListMessages), SessionService (bidi Converse: client submissions, server events), VoiceService (bidi Talk: PCM frames and control in; transcripts, state, TTS audio out), ApprovalService, ConfigService (providers, models, voices, devices), McpService, PluginService, AuditService. buf.yaml with lint rules.

Done when:

- buf lint passes; spec.md API section matches the protos

Execution plan:

The protos, `proto/buf.yaml`, `buf.gen.yaml` and `.github/workflows/proto.yml` survived the reset to Kotlin/Native (#81): they are language-neutral. Nothing is rewritten; the task is to prove they still meet the done criteria.

1. `buf lint proto`, `buf format proto --diff --exit-code` and `buf build proto`.
2. Compare every service and RPC in `proto/aulo/v1` with the table in `spec.md` §11.
3. `buf breaking proto` against `origin/main`, and `buf generate proto` with no drift in `docs/api`.
