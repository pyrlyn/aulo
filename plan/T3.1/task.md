# T3.1. aulo-proto: v1 service definitions

Stage: S3 · Area: api · Depends on: T2.4 · Blocks: 2 task(s)

proto/aulo/v1/*.proto: ChatService (Create/List/Get/Rename/Delete chats, ListMessages), SessionService (bidi Converse: client submissions, server events), VoiceService (bidi Talk: PCM frames and control in; transcripts, state, TTS audio out), ApprovalService, ConfigService (providers, models, voices, devices), McpService, PluginService, AuditService. buf.yaml with lint rules.


Done when:

- buf lint passes; spec.md API section matches the protos
