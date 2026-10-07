# proto

gRPC service definitions, package `aulo.v1` (T3.1). The service list is in `spec.md` §11.

| File | Contents |
| --- | --- |
| `aulo/v1/common.proto` | Shared enums and `ModelRef`; id, limit and untrusted-input conventions |
| `aulo/v1/event.proto` | `Event`: mirrors `aulo-types` `AuloEvent`, plus the agent-loop events |
| `aulo/v1/chat.proto` | `ChatService` |
| `aulo/v1/session.proto` | `SessionService.Converse` |
| `aulo/v1/voice.proto` | `VoiceService` |
| `aulo/v1/approval.proto` | `ApprovalService` |
| `aulo/v1/config.proto` | `ConfigService` |
| `aulo/v1/mcp.proto` | `McpService` |
| `aulo/v1/plugin.proto` | `PluginService` |
| `aulo/v1/audit.proto` | `AuditService` |

`buf.yaml` is the module root: lint `STANDARD`, breaking `FILE`. From this directory:

```sh
buf lint && buf build && buf format -d
```

Ids are ULID strings, timestamps are `google.protobuf.Timestamp`, lists page with `page_token`. Size caps and which fields are untrusted are written on the fields.
