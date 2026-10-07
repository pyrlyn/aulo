# Protocol Documentation
<a name="top"></a>

## Table of Contents

- [aulo/v1/common.proto](#aulo_v1_common-proto)
    - [ModelRef](#aulo-v1-ModelRef)
  
    - [ApprovalDecision](#aulo-v1-ApprovalDecision)
    - [ToolCallStatus](#aulo-v1-ToolCallStatus)
  
- [aulo/v1/event.proto](#aulo_v1_event-proto)
    - [ApprovalRequired](#aulo-v1-ApprovalRequired)
    - [ApprovalResolved](#aulo-v1-ApprovalResolved)
    - [Event](#aulo-v1-Event)
    - [Notice](#aulo-v1-Notice)
    - [SpeechEnded](#aulo-v1-SpeechEnded)
    - [SpeechStarted](#aulo-v1-SpeechStarted)
    - [TakeoverRequested](#aulo-v1-TakeoverRequested)
    - [TextDelta](#aulo-v1-TextDelta)
    - [ToolCallFinished](#aulo-v1-ToolCallFinished)
    - [ToolCallOutput](#aulo-v1-ToolCallOutput)
    - [ToolCallStarted](#aulo-v1-ToolCallStarted)
    - [Transcript](#aulo-v1-Transcript)
    - [TtsChunk](#aulo-v1-TtsChunk)
    - [TurnFinished](#aulo-v1-TurnFinished)
    - [TurnStarted](#aulo-v1-TurnStarted)
    - [VoiceStateChanged](#aulo-v1-VoiceStateChanged)
  
    - [NoticeLevel](#aulo-v1-NoticeLevel)
    - [TakeoverReason](#aulo-v1-TakeoverReason)
    - [TranscriptKind](#aulo-v1-TranscriptKind)
    - [TurnEndReason](#aulo-v1-TurnEndReason)
    - [VoiceState](#aulo-v1-VoiceState)
  
- [aulo/v1/approval.proto](#aulo_v1_approval-proto)
    - [DecideRequest](#aulo-v1-DecideRequest)
    - [DecideResponse](#aulo-v1-DecideResponse)
    - [Grant](#aulo-v1-Grant)
    - [ListGrantsRequest](#aulo-v1-ListGrantsRequest)
    - [ListGrantsResponse](#aulo-v1-ListGrantsResponse)
    - [ListPendingRequest](#aulo-v1-ListPendingRequest)
    - [ListPendingResponse](#aulo-v1-ListPendingResponse)
    - [RevokeGrantRequest](#aulo-v1-RevokeGrantRequest)
    - [RevokeGrantResponse](#aulo-v1-RevokeGrantResponse)
  
    - [GrantDecision](#aulo-v1-GrantDecision)
  
    - [ApprovalService](#aulo-v1-ApprovalService)
  
- [aulo/v1/audit.proto](#aulo_v1_audit-proto)
    - [AuditEntry](#aulo-v1-AuditEntry)
    - [ExportRequest](#aulo-v1-ExportRequest)
    - [ExportResponse](#aulo-v1-ExportResponse)
    - [QueryRequest](#aulo-v1-QueryRequest)
    - [QueryResponse](#aulo-v1-QueryResponse)
    - [VerifyRequest](#aulo-v1-VerifyRequest)
    - [VerifyResponse](#aulo-v1-VerifyResponse)
  
    - [AuditService](#aulo-v1-AuditService)
  
- [aulo/v1/chat.proto](#aulo_v1_chat-proto)
    - [Chat](#aulo-v1-Chat)
    - [CreateChatRequest](#aulo-v1-CreateChatRequest)
    - [CreateChatResponse](#aulo-v1-CreateChatResponse)
    - [DeleteChatRequest](#aulo-v1-DeleteChatRequest)
    - [DeleteChatResponse](#aulo-v1-DeleteChatResponse)
    - [GetChatRequest](#aulo-v1-GetChatRequest)
    - [GetChatResponse](#aulo-v1-GetChatResponse)
    - [ListChatsRequest](#aulo-v1-ListChatsRequest)
    - [ListChatsResponse](#aulo-v1-ListChatsResponse)
    - [ListMessagesRequest](#aulo-v1-ListMessagesRequest)
    - [ListMessagesResponse](#aulo-v1-ListMessagesResponse)
    - [Message](#aulo-v1-Message)
    - [RenameChatRequest](#aulo-v1-RenameChatRequest)
    - [RenameChatResponse](#aulo-v1-RenameChatResponse)
    - [SearchMessagesRequest](#aulo-v1-SearchMessagesRequest)
    - [SearchMessagesResponse](#aulo-v1-SearchMessagesResponse)
    - [ToolCall](#aulo-v1-ToolCall)
  
    - [MessageRole](#aulo-v1-MessageRole)
  
    - [ChatService](#aulo-v1-ChatService)
  
- [aulo/v1/config.proto](#aulo_v1_config-proto)
    - [AudioDevice](#aulo-v1-AudioDevice)
    - [GetConfigRequest](#aulo-v1-GetConfigRequest)
    - [GetConfigResponse](#aulo-v1-GetConfigResponse)
    - [ListAudioDevicesRequest](#aulo-v1-ListAudioDevicesRequest)
    - [ListAudioDevicesResponse](#aulo-v1-ListAudioDevicesResponse)
    - [ListModelsRequest](#aulo-v1-ListModelsRequest)
    - [ListModelsResponse](#aulo-v1-ListModelsResponse)
    - [ListProvidersRequest](#aulo-v1-ListProvidersRequest)
    - [ListProvidersResponse](#aulo-v1-ListProvidersResponse)
    - [Model](#aulo-v1-Model)
    - [Provider](#aulo-v1-Provider)
    - [SetConfigRequest](#aulo-v1-SetConfigRequest)
    - [SetConfigResponse](#aulo-v1-SetConfigResponse)
    - [SetDeviceRequest](#aulo-v1-SetDeviceRequest)
    - [SetDeviceResponse](#aulo-v1-SetDeviceResponse)
    - [SetProviderKeyRequest](#aulo-v1-SetProviderKeyRequest)
    - [SetProviderKeyResponse](#aulo-v1-SetProviderKeyResponse)
  
    - [AudioDirection](#aulo-v1-AudioDirection)
  
    - [ConfigService](#aulo-v1-ConfigService)
  
- [aulo/v1/mcp.proto](#aulo_v1_mcp-proto)
    - [ListServersRequest](#aulo-v1-ListServersRequest)
    - [ListServersResponse](#aulo-v1-ListServersResponse)
    - [McpServer](#aulo-v1-McpServer)
    - [McpServiceDisableRequest](#aulo-v1-McpServiceDisableRequest)
    - [McpServiceDisableResponse](#aulo-v1-McpServiceDisableResponse)
    - [McpServiceEnableRequest](#aulo-v1-McpServiceEnableRequest)
    - [McpServiceEnableResponse](#aulo-v1-McpServiceEnableResponse)
    - [McpServiceRestartRequest](#aulo-v1-McpServiceRestartRequest)
    - [McpServiceRestartResponse](#aulo-v1-McpServiceRestartResponse)
    - [ServerStatusRequest](#aulo-v1-ServerStatusRequest)
    - [ServerStatusResponse](#aulo-v1-ServerStatusResponse)
  
    - [McpServerState](#aulo-v1-McpServerState)
  
    - [McpService](#aulo-v1-McpService)
  
- [aulo/v1/plugin.proto](#aulo_v1_plugin-proto)
    - [Plugin](#aulo-v1-Plugin)
    - [PluginServiceDisableRequest](#aulo-v1-PluginServiceDisableRequest)
    - [PluginServiceDisableResponse](#aulo-v1-PluginServiceDisableResponse)
    - [PluginServiceEnableRequest](#aulo-v1-PluginServiceEnableRequest)
    - [PluginServiceEnableResponse](#aulo-v1-PluginServiceEnableResponse)
    - [PluginServiceInstallRequest](#aulo-v1-PluginServiceInstallRequest)
    - [PluginServiceInstallResponse](#aulo-v1-PluginServiceInstallResponse)
    - [PluginServiceListRequest](#aulo-v1-PluginServiceListRequest)
    - [PluginServiceListResponse](#aulo-v1-PluginServiceListResponse)
    - [PluginServiceRemoveRequest](#aulo-v1-PluginServiceRemoveRequest)
    - [PluginServiceRemoveResponse](#aulo-v1-PluginServiceRemoveResponse)
  
    - [PluginKind](#aulo-v1-PluginKind)
  
    - [PluginService](#aulo-v1-PluginService)
  
- [aulo/v1/session.proto](#aulo_v1_session-proto)
    - [Attach](#aulo-v1-Attach)
    - [ConverseRequest](#aulo-v1-ConverseRequest)
    - [ConverseResponse](#aulo-v1-ConverseResponse)
    - [DecideApproval](#aulo-v1-DecideApproval)
    - [Interrupt](#aulo-v1-Interrupt)
    - [SwitchModel](#aulo-v1-SwitchModel)
    - [TakeoverDone](#aulo-v1-TakeoverDone)
    - [UserTurn](#aulo-v1-UserTurn)
  
    - [SessionService](#aulo-v1-SessionService)
  
- [aulo/v1/voice.proto](#aulo_v1_voice-proto)
    - [ListVoicesRequest](#aulo-v1-ListVoicesRequest)
    - [ListVoicesResponse](#aulo-v1-ListVoicesResponse)
    - [SetVoiceRequest](#aulo-v1-SetVoiceRequest)
    - [SetVoiceResponse](#aulo-v1-SetVoiceResponse)
    - [StartListeningRequest](#aulo-v1-StartListeningRequest)
    - [StartListeningResponse](#aulo-v1-StartListeningResponse)
    - [StopListeningRequest](#aulo-v1-StopListeningRequest)
    - [StopListeningResponse](#aulo-v1-StopListeningResponse)
    - [TalkRequest](#aulo-v1-TalkRequest)
    - [TalkResponse](#aulo-v1-TalkResponse)
    - [TalkStart](#aulo-v1-TalkStart)
    - [TtsAudio](#aulo-v1-TtsAudio)
    - [Voice](#aulo-v1-Voice)
  
    - [AudioFormat](#aulo-v1-AudioFormat)
    - [TalkControl](#aulo-v1-TalkControl)
  
    - [VoiceService](#aulo-v1-VoiceService)
  
- [Scalar Value Types](#scalar-value-types)



<a name="aulo_v1_common-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/common.proto



<a name="aulo-v1-ModelRef"></a>

### ModelRef
A provider-qualified model, for example provider &#34;anthropic&#34; and model
&#34;claude-sonnet-5-5&#34;. Both parts are the ids ConfigService.ListModels returns.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| provider | [string](#string) |  | Provider id from [providers.*] in the config. At most 64 bytes. |
| model | [string](#string) |  | Model id as the provider names it. At most 128 bytes. |





 


<a name="aulo-v1-ApprovalDecision"></a>

### ApprovalDecision
How a human answered an approval. Voice never widens access: when
ApprovalRequired.needs_click is true, only a deliberate click or key may
produce one of these.

| Name | Number | Description |
| ---- | ------ | ----------- |
| APPROVAL_DECISION_UNSPECIFIED | 0 |  |
| APPROVAL_DECISION_ALLOW_ONCE | 1 | Run this call once. |
| APPROVAL_DECISION_ALLOW_CHAT | 2 | Allow this tool and subject for the rest of this chat. |
| APPROVAL_DECISION_ALLOW_ALWAYS | 3 | Allow this tool and subject from now on; stored as a grant. |
| APPROVAL_DECISION_DENY_ONCE | 4 | Refuse this call once. |
| APPROVAL_DECISION_DENY_ALWAYS | 5 | Refuse this tool and subject from now on; stored as a grant. |



<a name="aulo-v1-ToolCallStatus"></a>

### ToolCallStatus
Where a tool call is in its life.

| Name | Number | Description |
| ---- | ------ | ----------- |
| TOOL_CALL_STATUS_UNSPECIFIED | 0 |  |
| TOOL_CALL_STATUS_RUNNING | 1 |  |
| TOOL_CALL_STATUS_SUCCEEDED | 2 |  |
| TOOL_CALL_STATUS_FAILED | 3 |  |
| TOOL_CALL_STATUS_DENIED | 4 | Policy or a human refused it; it never ran. |
| TOOL_CALL_STATUS_CANCELLED | 5 | The turn was interrupted while it ran. |


 

 

 



<a name="aulo_v1_event-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/event.proto



<a name="aulo-v1-ApprovalRequired"></a>

### ApprovalRequired
The agent wants to run a call that policy sent to Ask. Any attached client
may answer with ApprovalService.Decide or a Converse decision; no answer
before the server timeout means deny.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |
| turn_id | [string](#string) |  | ULID. |
| call_id | [string](#string) |  | ULID. |
| tool | [string](#string) |  | Tool name, for example &#34;shell&#34; or &#34;mcp__server__tool&#34;. Untrusted: plugin and MCP names come from third parties. At most 128 bytes. |
| summary | [string](#string) |  | Human-readable description of the call, built from model-chosen arguments. Untrusted; render as plain text. At most 2 KiB; secrets appear as handles. |
| needs_click | [bool](#bool) |  | Voice never widens access: when true, only a click or key approves. |






<a name="aulo-v1-ApprovalResolved"></a>

### ApprovalResolved
An approval was answered, so every other attached client can dismiss its
prompt.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |
| decision | [ApprovalDecision](#aulo-v1-ApprovalDecision) |  |  |
| timed_out | [bool](#bool) |  | True when nobody answered in time and the server denied. |






<a name="aulo-v1-Event"></a>

### Event
One event pushed to a client. The first block of variants mirrors aulo-types
AuloEvent one to one (same names, same fields), so the server maps them
without a translation table. The second block carries the agent loop: the
turn, its streamed text and its tool calls.

Events never carry audio samples: TTS audio travels as VoiceService TtsAudio
messages so events stay small and the audio path stays bounded.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| voice_state | [VoiceStateChanged](#aulo-v1-VoiceStateChanged) |  | Mirrors of AuloEvent. |
| speech_started | [SpeechStarted](#aulo-v1-SpeechStarted) |  |  |
| speech_ended | [SpeechEnded](#aulo-v1-SpeechEnded) |  |  |
| transcript | [Transcript](#aulo-v1-Transcript) |  |  |
| tts_chunk | [TtsChunk](#aulo-v1-TtsChunk) |  |  |
| approval_required | [ApprovalRequired](#aulo-v1-ApprovalRequired) |  |  |
| takeover_requested | [TakeoverRequested](#aulo-v1-TakeoverRequested) |  |  |
| notice | [Notice](#aulo-v1-Notice) |  |  |
| turn_started | [TurnStarted](#aulo-v1-TurnStarted) |  | Agent loop. |
| text_delta | [TextDelta](#aulo-v1-TextDelta) |  |  |
| tool_call_started | [ToolCallStarted](#aulo-v1-ToolCallStarted) |  |  |
| tool_call_output | [ToolCallOutput](#aulo-v1-ToolCallOutput) |  |  |
| tool_call_finished | [ToolCallFinished](#aulo-v1-ToolCallFinished) |  |  |
| approval_resolved | [ApprovalResolved](#aulo-v1-ApprovalResolved) |  |  |
| turn_finished | [TurnFinished](#aulo-v1-TurnFinished) |  |  |






<a name="aulo-v1-Notice"></a>

### Notice



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| level | [NoticeLevel](#aulo-v1-NoticeLevel) |  |  |
| message | [string](#string) |  | Untrusted when it quotes a plugin, MCP server or engine. At most 2 KiB. |
| source | [string](#string) | optional | The engine, server or plugin the notice is about, if any. |






<a name="aulo-v1-SpeechEnded"></a>

### SpeechEnded



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| at_ms | [uint64](#uint64) |  |  |






<a name="aulo-v1-SpeechStarted"></a>

### SpeechStarted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| at_ms | [uint64](#uint64) |  | Offset into the capture stream, so clients can align it with audio. |






<a name="aulo-v1-TakeoverRequested"></a>

### TakeoverRequested



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |
| call_id | [string](#string) |  | ULID. |
| reason | [TakeoverReason](#aulo-v1-TakeoverReason) |  |  |






<a name="aulo-v1-TextDelta"></a>

### TextDelta
A streamed piece of the assistant reply. Concatenate deltas of one turn in
arrival order.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| text | [string](#string) |  | Untrusted model output. At most 16 KiB per delta. |






<a name="aulo-v1-ToolCallFinished"></a>

### ToolCallFinished



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |
| status | [ToolCallStatus](#aulo-v1-ToolCallStatus) |  |  |






<a name="aulo-v1-ToolCallOutput"></a>

### ToolCallOutput
A streamed piece of tool output.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |
| text | [string](#string) |  | Untrusted: tool, web and plugin output. At most 16 KiB per event and truncated server-side beyond a per-call total. |






<a name="aulo-v1-ToolCallStarted"></a>

### ToolCallStarted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| call_id | [string](#string) |  | ULID. |
| tool | [string](#string) |  | Untrusted; see ApprovalRequired.tool. |
| arguments_json | [string](#string) |  | Model-chosen arguments as a JSON object. Untrusted. Truncated server-side to 16 KiB for display; the persisted call keeps the full text. |






<a name="aulo-v1-Transcript"></a>

### Transcript



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| kind | [TranscriptKind](#aulo-v1-TranscriptKind) |  |  |
| text | [string](#string) |  | Untrusted: STT output of whatever was said or played near the microphone. At most 64 KiB; the server truncates longer text. |
| language | [string](#string) | optional | BCP 47 tag when the engine detected or was told the language. |






<a name="aulo-v1-TtsChunk"></a>

### TtsChunk
Metadata of one synthesized audio chunk. The samples follow in the matching
TtsAudio message (same turn_id and seq) on the Talk stream.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| seq | [uint32](#uint32) |  | Position in the reply, so a client can detect a dropped chunk. |
| sample_rate_hz | [uint32](#uint32) |  |  |
| channels | [uint32](#uint32) |  | Rust side is u8; protobuf has no 8-bit integer. |
| duration_ms | [uint32](#uint32) |  |  |
| last | [bool](#bool) |  | True on the last chunk of the reply. |






<a name="aulo-v1-TurnFinished"></a>

### TurnFinished



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| reason | [TurnEndReason](#aulo-v1-TurnEndReason) |  |  |
| error | [string](#string) | optional | Set when reason is FAILED. Server-written, no secrets, at most 2 KiB. |






<a name="aulo-v1-TurnStarted"></a>

### TurnStarted
A user turn was accepted and the agent started on it.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID, assigned by the server. |






<a name="aulo-v1-VoiceStateChanged"></a>

### VoiceStateChanged
AuloEvent::VoiceState. Named Changed because the enum already owns the name.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| state | [VoiceState](#aulo-v1-VoiceState) |  |  |





 


<a name="aulo-v1-NoticeLevel"></a>

### NoticeLevel
Severity of a Notice.

| Name | Number | Description |
| ---- | ------ | ----------- |
| NOTICE_LEVEL_UNSPECIFIED | 0 |  |
| NOTICE_LEVEL_INFO | 1 |  |
| NOTICE_LEVEL_WARN | 2 |  |
| NOTICE_LEVEL_ERROR | 3 |  |



<a name="aulo-v1-TakeoverReason"></a>

### TakeoverReason
What a human has to do themselves because the agent must not.

| Name | Number | Description |
| ---- | ------ | ----------- |
| TAKEOVER_REASON_UNSPECIFIED | 0 |  |
| TAKEOVER_REASON_PASSWORD | 1 |  |
| TAKEOVER_REASON_TWO_FACTOR | 2 |  |
| TAKEOVER_REASON_CAPTCHA | 3 |  |
| TAKEOVER_REASON_OTHER | 4 |  |



<a name="aulo-v1-TranscriptKind"></a>

### TranscriptKind
Whether a transcript may still change.

| Name | Number | Description |
| ---- | ------ | ----------- |
| TRANSCRIPT_KIND_UNSPECIFIED | 0 |  |
| TRANSCRIPT_KIND_PARTIAL | 1 | A streaming hypothesis the engine may revise. |
| TRANSCRIPT_KIND_FINAL | 2 | The engine&#39;s committed text for the utterance. |



<a name="aulo-v1-TurnEndReason"></a>

### TurnEndReason
Why a turn ended.

| Name | Number | Description |
| ---- | ------ | ----------- |
| TURN_END_REASON_UNSPECIFIED | 0 |  |
| TURN_END_REASON_COMPLETED | 1 |  |
| TURN_END_REASON_INTERRUPTED | 2 |  |
| TURN_END_REASON_FAILED | 3 |  |
| TURN_END_REASON_MAX_STEPS | 4 | The agent hit its step limit before finishing. |



<a name="aulo-v1-VoiceState"></a>

### VoiceState
Where the voice pipeline is for the current client.

| Name | Number | Description |
| ---- | ------ | ----------- |
| VOICE_STATE_UNSPECIFIED | 0 |  |
| VOICE_STATE_IDLE | 1 |  |
| VOICE_STATE_LISTENING | 2 |  |
| VOICE_STATE_THINKING | 3 |  |
| VOICE_STATE_SPEAKING | 4 |  |


 

 

 



<a name="aulo_v1_approval-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/approval.proto



<a name="aulo-v1-DecideRequest"></a>

### DecideRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |
| decision | [ApprovalDecision](#aulo-v1-ApprovalDecision) |  |  |






<a name="aulo-v1-DecideResponse"></a>

### DecideResponse







<a name="aulo-v1-Grant"></a>

### Grant
A standing answer stored by an ALLOW_ALWAYS or DENY_ALWAYS decision.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | ULID. |
| subject | [string](#string) |  | What the grant covers, written by aulo-policy; opaque to clients, which show it and never build one. |
| scope | [string](#string) |  |  |
| decision | [GrantDecision](#aulo-v1-GrantDecision) |  |  |
| expires_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) | optional | Absent means it never expires. |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="aulo-v1-ListGrantsRequest"></a>

### ListGrantsRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |






<a name="aulo-v1-ListGrantsResponse"></a>

### ListGrantsResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| grants | [Grant](#aulo-v1-Grant) | repeated |  |
| next_page_token | [string](#string) |  |  |






<a name="aulo-v1-ListPendingRequest"></a>

### ListPendingRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) | optional | Only this chat. |






<a name="aulo-v1-ListPendingResponse"></a>

### ListPendingResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| approvals | [ApprovalRequired](#aulo-v1-ApprovalRequired) | repeated |  |






<a name="aulo-v1-RevokeGrantRequest"></a>

### RevokeGrantRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| grant_id | [string](#string) |  | ULID. |






<a name="aulo-v1-RevokeGrantResponse"></a>

### RevokeGrantResponse






 


<a name="aulo-v1-GrantDecision"></a>

### GrantDecision


| Name | Number | Description |
| ---- | ------ | ----------- |
| GRANT_DECISION_UNSPECIFIED | 0 |  |
| GRANT_DECISION_ALLOW | 1 |  |
| GRANT_DECISION_DENY | 2 |  |


 

 


<a name="aulo-v1-ApprovalService"></a>

### ApprovalService
Human answers to the questions policy asks, and the standing grants those
answers create. Scope: `control`.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| ListPending | [ListPendingRequest](#aulo-v1-ListPendingRequest) | [ListPendingResponse](#aulo-v1-ListPendingResponse) | Approvals still waiting, oldest first. Lets a client that attached late show prompts it missed. |
| Decide | [DecideRequest](#aulo-v1-DecideRequest) | [DecideResponse](#aulo-v1-DecideResponse) | Answers one pending approval. Always a deliberate click or key from the client&#39;s own UI: spoken answers are resolved inside the daemon and never arrive here, so a client must not forward a spoken yes for a call whose needs_click is true. An unknown or already answered call_id fails with NOT_FOUND. |
| ListGrants | [ListGrantsRequest](#aulo-v1-ListGrantsRequest) | [ListGrantsResponse](#aulo-v1-ListGrantsResponse) |  |
| RevokeGrant | [RevokeGrantRequest](#aulo-v1-RevokeGrantRequest) | [RevokeGrantResponse](#aulo-v1-RevokeGrantResponse) |  |

 



<a name="aulo_v1_audit-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/audit.proto



<a name="aulo-v1-AuditEntry"></a>

### AuditEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| seq | [uint64](#uint64) |  | Position in the chain, assigned by the database. |
| prev_hash | [string](#string) |  | Hex digests; hash covers prev_hash and the row&#39;s content. |
| hash | [string](#string) |  |  |
| kind | [string](#string) |  | Row kind, for example &#34;tool_call&#34; or &#34;approval&#34;. |
| payload_json | [string](#string) |  | JSON object. Untrusted: it quotes model-chosen arguments and tool output. |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="aulo-v1-ExportRequest"></a>

### ExportRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| from_seq | [uint64](#uint64) | optional | Inclusive bounds on seq; absent means from the first or to the last row. |
| to_seq | [uint64](#uint64) | optional |  |






<a name="aulo-v1-ExportResponse"></a>

### ExportResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chunk | [bytes](#bytes) |  | A slice of the JSON Lines stream, at most 64 KiB. A row may span chunks; concatenate in order. |






<a name="aulo-v1-QueryRequest"></a>

### QueryRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |
| kind | [string](#string) | optional | Only rows of this kind. |
| since | [google.protobuf.Timestamp](#google-protobuf-Timestamp) | optional | Only rows created at or after / before these instants. |
| until | [google.protobuf.Timestamp](#google-protobuf-Timestamp) | optional |  |






<a name="aulo-v1-QueryResponse"></a>

### QueryResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| entries | [AuditEntry](#aulo-v1-AuditEntry) | repeated |  |
| next_page_token | [string](#string) |  |  |






<a name="aulo-v1-VerifyRequest"></a>

### VerifyRequest







<a name="aulo-v1-VerifyResponse"></a>

### VerifyResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| ok | [bool](#bool) |  |  |
| rows_checked | [uint64](#uint64) |  |  |
| first_bad_seq | [uint64](#uint64) | optional | Set when ok is false: the first row whose hash does not match. |





 

 

 


<a name="aulo-v1-AuditService"></a>

### AuditService
The append-only, hash-chained audit log of tool calls, approvals, egress and
plugin loads. Read only: no RPC writes or deletes rows. Scope: `admin`,
because payloads can contain what a user asked the agent to do.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Query | [QueryRequest](#aulo-v1-QueryRequest) | [QueryResponse](#aulo-v1-QueryResponse) | Oldest first (seq ascending). |
| Verify | [VerifyRequest](#aulo-v1-VerifyRequest) | [VerifyResponse](#aulo-v1-VerifyResponse) | Recomputes the whole hash chain. |
| Export | [ExportRequest](#aulo-v1-ExportRequest) | [ExportResponse](#aulo-v1-ExportResponse) stream | Streams rows as JSON Lines, one AuditEntry per line, in chunks. |

 



<a name="aulo_v1_chat-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/chat.proto



<a name="aulo-v1-Chat"></a>

### Chat



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | ULID. |
| bot_id | [string](#string) | optional | The bot (agent persona) the chat belongs to; absent for the default bot. |
| title | [string](#string) |  | Untrusted when generated by the model. At most 200 bytes. |
| model | [ModelRef](#aulo-v1-ModelRef) |  | The model the next turn uses; changed by a SwitchModel submission. |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |
| updated_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="aulo-v1-CreateChatRequest"></a>

### CreateChatRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| bot_id | [string](#string) | optional | Defaults to the default bot. |
| title | [string](#string) | optional | Defaults to an empty title that the cheap tier fills in after the first turn. At most 200 bytes. |
| model | [ModelRef](#aulo-v1-ModelRef) | optional | Defaults to the bot&#39;s configured main model. |






<a name="aulo-v1-CreateChatResponse"></a>

### CreateChatResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat | [Chat](#aulo-v1-Chat) |  |  |






<a name="aulo-v1-DeleteChatRequest"></a>

### DeleteChatRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |






<a name="aulo-v1-DeleteChatResponse"></a>

### DeleteChatResponse







<a name="aulo-v1-GetChatRequest"></a>

### GetChatRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |






<a name="aulo-v1-GetChatResponse"></a>

### GetChatResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat | [Chat](#aulo-v1-Chat) |  |  |






<a name="aulo-v1-ListChatsRequest"></a>

### ListChatsRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |
| bot_id | [string](#string) | optional | Only chats of this bot. |






<a name="aulo-v1-ListChatsResponse"></a>

### ListChatsResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chats | [Chat](#aulo-v1-Chat) | repeated |  |
| next_page_token | [string](#string) |  | Empty on the last page. |






<a name="aulo-v1-ListMessagesRequest"></a>

### ListMessagesRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |






<a name="aulo-v1-ListMessagesResponse"></a>

### ListMessagesResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| messages | [Message](#aulo-v1-Message) | repeated |  |
| next_page_token | [string](#string) |  |  |






<a name="aulo-v1-Message"></a>

### Message



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | ULID. |
| chat_id | [string](#string) |  | ULID. |
| role | [MessageRole](#aulo-v1-MessageRole) |  |  |
| content | [string](#string) |  | Untrusted for every role except what the signed-in user typed. Text is never interpreted as markup by the server. |
| tool_calls | [ToolCall](#aulo-v1-ToolCall) | repeated | Calls the assistant made in this message, oldest first. |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="aulo-v1-RenameChatRequest"></a>

### RenameChatRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |
| title | [string](#string) |  | Non-empty, at most 200 bytes. |






<a name="aulo-v1-RenameChatResponse"></a>

### RenameChatResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat | [Chat](#aulo-v1-Chat) |  |  |






<a name="aulo-v1-SearchMessagesRequest"></a>

### SearchMessagesRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| query | [string](#string) |  | Untrusted user input for the FTS index; at most 1 KiB. Treated as plain words, never as query syntax. |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |
| chat_id | [string](#string) | optional | Only this chat. |






<a name="aulo-v1-SearchMessagesResponse"></a>

### SearchMessagesResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| messages | [Message](#aulo-v1-Message) | repeated |  |
| next_page_token | [string](#string) |  |  |






<a name="aulo-v1-ToolCall"></a>

### ToolCall



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | ULID. |
| name | [string](#string) |  | Untrusted; see Event ApprovalRequired.tool. |
| arguments_json | [string](#string) |  | Model-chosen arguments as a JSON object. Untrusted. |
| output | [string](#string) | optional | Tool output, absent while running. Untrusted; the server caps it at 64 KiB here and keeps the full text only in storage. |
| status | [ToolCallStatus](#aulo-v1-ToolCallStatus) |  |  |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |





 


<a name="aulo-v1-MessageRole"></a>

### MessageRole


| Name | Number | Description |
| ---- | ------ | ----------- |
| MESSAGE_ROLE_UNSPECIFIED | 0 |  |
| MESSAGE_ROLE_USER | 1 |  |
| MESSAGE_ROLE_ASSISTANT | 2 |  |
| MESSAGE_ROLE_TOOL | 3 |  |


 

 


<a name="aulo-v1-ChatService"></a>

### ChatService
Chat CRUD and message history. Scopes: reads need `read`; create, rename and
delete need `chat`.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| CreateChat | [CreateChatRequest](#aulo-v1-CreateChatRequest) | [CreateChatResponse](#aulo-v1-CreateChatResponse) |  |
| ListChats | [ListChatsRequest](#aulo-v1-ListChatsRequest) | [ListChatsResponse](#aulo-v1-ListChatsResponse) | Newest activity first (updated_at descending). |
| GetChat | [GetChatRequest](#aulo-v1-GetChatRequest) | [GetChatResponse](#aulo-v1-GetChatResponse) |  |
| RenameChat | [RenameChatRequest](#aulo-v1-RenameChatRequest) | [RenameChatResponse](#aulo-v1-RenameChatResponse) |  |
| DeleteChat | [DeleteChatRequest](#aulo-v1-DeleteChatRequest) | [DeleteChatResponse](#aulo-v1-DeleteChatResponse) | Deletes the chat with its messages, tool calls and usage rows. Permanent. |
| ListMessages | [ListMessagesRequest](#aulo-v1-ListMessagesRequest) | [ListMessagesResponse](#aulo-v1-ListMessagesResponse) | Oldest first (message id ascending). |
| SearchMessages | [SearchMessagesRequest](#aulo-v1-SearchMessagesRequest) | [SearchMessagesResponse](#aulo-v1-SearchMessagesResponse) | Full-text search across all chats, best match first. |

 



<a name="aulo_v1_config-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/config.proto



<a name="aulo-v1-AudioDevice"></a>

### AudioDevice



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | Stable id of the device on this host. At most 256 bytes. |
| name | [string](#string) |  | Untrusted: the OS or driver supplies it. |
| direction | [AudioDirection](#aulo-v1-AudioDirection) |  |  |
| is_default | [bool](#bool) |  |  |






<a name="aulo-v1-GetConfigRequest"></a>

### GetConfigRequest







<a name="aulo-v1-GetConfigResponse"></a>

### GetConfigResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| config_json | [string](#string) |  | The effective configuration as JSON that validates against schemas/aulo.schema.json. Secrets are never part of it; inline keys are rejected by the config loader. |






<a name="aulo-v1-ListAudioDevicesRequest"></a>

### ListAudioDevicesRequest







<a name="aulo-v1-ListAudioDevicesResponse"></a>

### ListAudioDevicesResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| devices | [AudioDevice](#aulo-v1-AudioDevice) | repeated |  |






<a name="aulo-v1-ListModelsRequest"></a>

### ListModelsRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| provider_id | [string](#string) | optional | Only this provider. |
| page_size | [uint32](#uint32) |  |  |
| page_token | [string](#string) |  |  |






<a name="aulo-v1-ListModelsResponse"></a>

### ListModelsResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| models | [Model](#aulo-v1-Model) | repeated |  |
| next_page_token | [string](#string) |  |  |






<a name="aulo-v1-ListProvidersRequest"></a>

### ListProvidersRequest







<a name="aulo-v1-ListProvidersResponse"></a>

### ListProvidersResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| providers | [Provider](#aulo-v1-Provider) | repeated |  |






<a name="aulo-v1-Model"></a>

### Model



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| ref | [ModelRef](#aulo-v1-ModelRef) |  |  |
| display_name | [string](#string) |  | Untrusted: comes from a provider&#39;s server. At most 128 bytes. |
| served | [bool](#bool) |  | True when the provider&#39;s server reports it as served right now, as opposed to only being in the catalog. |






<a name="aulo-v1-Provider"></a>

### Provider



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | Section name under [providers.*]. At most 64 bytes. |
| kind | [string](#string) |  | Wire family, for example &#34;openai&#34;, &#34;anthropic&#34; or &#34;openai-compatible&#34;. |
| base_url | [string](#string) | optional | Absent for hosted providers with a fixed endpoint. |
| key_set | [bool](#bool) |  | Whether a credential is available (keychain or env). The value is never sent. |






<a name="aulo-v1-SetConfigRequest"></a>

### SetConfigRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| merge_patch_json | [string](#string) |  | An RFC 7386 JSON merge patch applied to the user layer (~/.aulo/config.toml) and validated against the schema before it is written; unknown keys are rejected. At most 64 KiB. |






<a name="aulo-v1-SetConfigResponse"></a>

### SetConfigResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| config_json | [string](#string) |  | The effective configuration after the patch, as in GetConfigResponse. |






<a name="aulo-v1-SetDeviceRequest"></a>

### SetDeviceRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| direction | [AudioDirection](#aulo-v1-AudioDirection) |  |  |
| device_id | [string](#string) |  | Empty selects the system default. |






<a name="aulo-v1-SetDeviceResponse"></a>

### SetDeviceResponse







<a name="aulo-v1-SetProviderKeyRequest"></a>

### SetProviderKeyRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| provider_id | [string](#string) |  |  |
| key | [string](#string) |  | At most 4 KiB. Never echoed, logged or audited. |






<a name="aulo-v1-SetProviderKeyResponse"></a>

### SetProviderKeyResponse






 


<a name="aulo-v1-AudioDirection"></a>

### AudioDirection


| Name | Number | Description |
| ---- | ------ | ----------- |
| AUDIO_DIRECTION_UNSPECIFIED | 0 |  |
| AUDIO_DIRECTION_INPUT | 1 |  |
| AUDIO_DIRECTION_OUTPUT | 2 |  |


 

 


<a name="aulo-v1-ConfigService"></a>

### ConfigService
Providers, models, audio devices and the configuration. Reads need `read`;
every write needs `admin`.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| ListProviders | [ListProvidersRequest](#aulo-v1-ListProvidersRequest) | [ListProvidersResponse](#aulo-v1-ListProvidersResponse) |  |
| SetProviderKey | [SetProviderKeyRequest](#aulo-v1-SetProviderKeyRequest) | [SetProviderKeyResponse](#aulo-v1-SetProviderKeyResponse) | Stores the key in the OS keychain. Write-only: no RPC ever returns a key, and the key must not appear in logs, audit rows or events. A remote client should only call it over TLS. |
| ListModels | [ListModelsRequest](#aulo-v1-ListModelsRequest) | [ListModelsResponse](#aulo-v1-ListModelsResponse) | The catalog merged with what the provider&#39;s server reports as served. |
| ListAudioDevices | [ListAudioDevicesRequest](#aulo-v1-ListAudioDevicesRequest) | [ListAudioDevicesResponse](#aulo-v1-ListAudioDevicesResponse) |  |
| SetDevice | [SetDeviceRequest](#aulo-v1-SetDeviceRequest) | [SetDeviceResponse](#aulo-v1-SetDeviceResponse) | Picks the audio device for a direction; the choice persists. An unplugged device falls back to the default with a Notice. |
| GetConfig | [GetConfigRequest](#aulo-v1-GetConfigRequest) | [GetConfigResponse](#aulo-v1-GetConfigResponse) |  |
| SetConfig | [SetConfigRequest](#aulo-v1-SetConfigRequest) | [SetConfigResponse](#aulo-v1-SetConfigResponse) |  |

 



<a name="aulo_v1_mcp-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/mcp.proto



<a name="aulo-v1-ListServersRequest"></a>

### ListServersRequest







<a name="aulo-v1-ListServersResponse"></a>

### ListServersResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| servers | [McpServer](#aulo-v1-McpServer) | repeated |  |






<a name="aulo-v1-McpServer"></a>

### McpServer



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  | Key under [mcp.servers]. At most 64 bytes. |
| state | [McpServerState](#aulo-v1-McpServerState) |  |  |
| last_error | [string](#string) | optional | Why the server is FAILED. Untrusted: it can quote the server&#39;s own output. At most 2 KiB. |






<a name="aulo-v1-McpServiceDisableRequest"></a>

### McpServiceDisableRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  |  |






<a name="aulo-v1-McpServiceDisableResponse"></a>

### McpServiceDisableResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| server | [McpServer](#aulo-v1-McpServer) |  |  |






<a name="aulo-v1-McpServiceEnableRequest"></a>

### McpServiceEnableRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  |  |






<a name="aulo-v1-McpServiceEnableResponse"></a>

### McpServiceEnableResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| server | [McpServer](#aulo-v1-McpServer) |  |  |






<a name="aulo-v1-McpServiceRestartRequest"></a>

### McpServiceRestartRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  |  |






<a name="aulo-v1-McpServiceRestartResponse"></a>

### McpServiceRestartResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| server | [McpServer](#aulo-v1-McpServer) |  |  |






<a name="aulo-v1-ServerStatusRequest"></a>

### ServerStatusRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  |  |






<a name="aulo-v1-ServerStatusResponse"></a>

### ServerStatusResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| server | [McpServer](#aulo-v1-McpServer) |  |  |
| tools | [string](#string) | repeated | Tool names as the model sees them (mcp__server__tool). Untrusted: the server chooses them. |
| recent_stderr | [string](#string) | repeated | The last lines the server wrote to stderr, oldest first. Untrusted; the server keeps at most 100 lines of 4 KiB each. |





 


<a name="aulo-v1-McpServerState"></a>

### McpServerState


| Name | Number | Description |
| ---- | ------ | ----------- |
| MCP_SERVER_STATE_UNSPECIFIED | 0 |  |
| MCP_SERVER_STATE_DISABLED | 1 |  |
| MCP_SERVER_STATE_STARTING | 2 |  |
| MCP_SERVER_STATE_READY | 3 |  |
| MCP_SERVER_STATE_FAILED | 4 |  |


 

 


<a name="aulo-v1-McpService"></a>

### McpService
The MCP servers aulo is a client of. Reads need `read`; Enable, Disable and
Restart need `control`. A broken server is a notice and a FAILED state, never
an RPC error for the others. Enable, Disable and Restart messages carry the
service name because PluginService has RPCs of the same names and message
names are package-wide.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| ListServers | [ListServersRequest](#aulo-v1-ListServersRequest) | [ListServersResponse](#aulo-v1-ListServersResponse) |  |
| ServerStatus | [ServerStatusRequest](#aulo-v1-ServerStatusRequest) | [ServerStatusResponse](#aulo-v1-ServerStatusResponse) | One server with its tool names and recent stderr. |
| Enable | [McpServiceEnableRequest](#aulo-v1-McpServiceEnableRequest) | [McpServiceEnableResponse](#aulo-v1-McpServiceEnableResponse) |  |
| Disable | [McpServiceDisableRequest](#aulo-v1-McpServiceDisableRequest) | [McpServiceDisableResponse](#aulo-v1-McpServiceDisableResponse) |  |
| Restart | [McpServiceRestartRequest](#aulo-v1-McpServiceRestartRequest) | [McpServiceRestartResponse](#aulo-v1-McpServiceRestartResponse) |  |

 



<a name="aulo_v1_plugin-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/plugin.proto



<a name="aulo-v1-Plugin"></a>

### Plugin



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | Manifest id. At most 64 bytes. |
| name | [string](#string) |  | Untrusted: the plugin&#39;s own manifest text. At most 128 bytes. |
| version | [string](#string) |  |  |
| kind | [PluginKind](#aulo-v1-PluginKind) |  |  |
| enabled | [bool](#bool) |  |  |






<a name="aulo-v1-PluginServiceDisableRequest"></a>

### PluginServiceDisableRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  |  |






<a name="aulo-v1-PluginServiceDisableResponse"></a>

### PluginServiceDisableResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| plugin | [Plugin](#aulo-v1-Plugin) |  |  |






<a name="aulo-v1-PluginServiceEnableRequest"></a>

### PluginServiceEnableRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  |  |






<a name="aulo-v1-PluginServiceEnableResponse"></a>

### PluginServiceEnableResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| plugin | [Plugin](#aulo-v1-Plugin) |  |  |






<a name="aulo-v1-PluginServiceInstallRequest"></a>

### PluginServiceInstallRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [string](#string) |  | A filesystem path on the daemon&#39;s host or a git URL. Untrusted; at most 2 KiB. A relative path is rejected. |






<a name="aulo-v1-PluginServiceInstallResponse"></a>

### PluginServiceInstallResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| plugin | [Plugin](#aulo-v1-Plugin) |  |  |






<a name="aulo-v1-PluginServiceListRequest"></a>

### PluginServiceListRequest







<a name="aulo-v1-PluginServiceListResponse"></a>

### PluginServiceListResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| plugins | [Plugin](#aulo-v1-Plugin) | repeated |  |






<a name="aulo-v1-PluginServiceRemoveRequest"></a>

### PluginServiceRemoveRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  |  |






<a name="aulo-v1-PluginServiceRemoveResponse"></a>

### PluginServiceRemoveResponse






 


<a name="aulo-v1-PluginKind"></a>

### PluginKind
How the host runs the plugin.

| Name | Number | Description |
| ---- | ------ | ----------- |
| PLUGIN_KIND_UNSPECIFIED | 0 |  |
| PLUGIN_KIND_MCP_PROCESS | 1 |  |
| PLUGIN_KIND_GRPC_PROCESS | 2 |  |
| PLUGIN_KIND_WASM | 3 |  |


 

 


<a name="aulo-v1-PluginService"></a>

### PluginService
Installed plugins (spec section 9). Scope: List needs `read`; every other
RPC needs `admin`, because a plugin is code that runs on the host.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| List | [PluginServiceListRequest](#aulo-v1-PluginServiceListRequest) | [PluginServiceListResponse](#aulo-v1-PluginServiceListResponse) |  |
| Install | [PluginServiceInstallRequest](#aulo-v1-PluginServiceInstallRequest) | [PluginServiceInstallResponse](#aulo-v1-PluginServiceInstallResponse) | Installs from a local path or a git URL and leaves the plugin disabled until Enable. The manifest is validated and the plugin&#39;s declared capabilities are shown for approval before anything runs. |
| Enable | [PluginServiceEnableRequest](#aulo-v1-PluginServiceEnableRequest) | [PluginServiceEnableResponse](#aulo-v1-PluginServiceEnableResponse) |  |
| Disable | [PluginServiceDisableRequest](#aulo-v1-PluginServiceDisableRequest) | [PluginServiceDisableResponse](#aulo-v1-PluginServiceDisableResponse) |  |
| Remove | [PluginServiceRemoveRequest](#aulo-v1-PluginServiceRemoveRequest) | [PluginServiceRemoveResponse](#aulo-v1-PluginServiceRemoveResponse) |  |

 



<a name="aulo_v1_session-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/session.proto



<a name="aulo-v1-Attach"></a>

### Attach
Binds the stream to a chat. Must be the first message and may be sent once.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | ULID. |






<a name="aulo-v1-ConverseRequest"></a>

### ConverseRequest
A client submission. Names follow the agent loop&#39;s Submission type.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| attach | [Attach](#aulo-v1-Attach) |  |  |
| user_turn | [UserTurn](#aulo-v1-UserTurn) |  |  |
| decide_approval | [DecideApproval](#aulo-v1-DecideApproval) |  |  |
| interrupt | [Interrupt](#aulo-v1-Interrupt) |  |  |
| switch_model | [SwitchModel](#aulo-v1-SwitchModel) |  |  |
| takeover_done | [TakeoverDone](#aulo-v1-TakeoverDone) |  |  |






<a name="aulo-v1-ConverseResponse"></a>

### ConverseResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| event | [Event](#aulo-v1-Event) |  |  |






<a name="aulo-v1-DecideApproval"></a>

### DecideApproval
Answers an ApprovalRequired. The same decision as ApprovalService.Decide;
both reach one approval queue and the first answer wins.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |
| decision | [ApprovalDecision](#aulo-v1-ApprovalDecision) |  |  |






<a name="aulo-v1-Interrupt"></a>

### Interrupt
Stops the running turn and its tool calls within 200 ms.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| all | [bool](#bool) |  | Kill switch: stop every running turn, tool call and speech on the daemon, not only this chat. Requires the `control` scope. |






<a name="aulo-v1-SwitchModel"></a>

### SwitchModel
Changes the model for this chat from the next turn on.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| model | [ModelRef](#aulo-v1-ModelRef) |  |  |






<a name="aulo-v1-TakeoverDone"></a>

### TakeoverDone
The human finished what a TakeoverRequested asked for; the turn resumes only
after this arrives.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| call_id | [string](#string) |  | ULID. |






<a name="aulo-v1-UserTurn"></a>

### UserTurn
A user message. The server answers with TurnStarted.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| text | [string](#string) |  | At most 64 KiB; longer text is rejected, not truncated, so a client never sends a prompt the user did not write. |





 

 

 


<a name="aulo-v1-SessionService"></a>

### SessionService
Text conversation with the agent. Scope: `chat`; an Interrupt with
all=true needs `control`.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Converse | [ConverseRequest](#aulo-v1-ConverseRequest) stream | [ConverseResponse](#aulo-v1-ConverseResponse) stream | One stream drives one chat. The first request must be an Attach; anything else closes the stream with FAILED_PRECONDITION. The server answers with events of that chat only. Several clients may attach to one chat; each sees every event of it. History before the attach is read with ChatService.ListMessages, not replayed here. |

 



<a name="aulo_v1_voice-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## aulo/v1/voice.proto



<a name="aulo-v1-ListVoicesRequest"></a>

### ListVoicesRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| engine | [string](#string) | optional | Defaults to the active engine. |






<a name="aulo-v1-ListVoicesResponse"></a>

### ListVoicesResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| voices | [Voice](#aulo-v1-Voice) | repeated |  |






<a name="aulo-v1-SetVoiceRequest"></a>

### SetVoiceRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| engine | [string](#string) |  |  |
| voice_id | [string](#string) |  |  |
| bot_id | [string](#string) |  |  |
| chat_id | [string](#string) |  |  |






<a name="aulo-v1-SetVoiceResponse"></a>

### SetVoiceResponse







<a name="aulo-v1-StartListeningRequest"></a>

### StartListeningRequest







<a name="aulo-v1-StartListeningResponse"></a>

### StartListeningResponse







<a name="aulo-v1-StopListeningRequest"></a>

### StopListeningRequest







<a name="aulo-v1-StopListeningResponse"></a>

### StopListeningResponse







<a name="aulo-v1-TalkRequest"></a>

### TalkRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| start | [TalkStart](#aulo-v1-TalkStart) |  |  |
| audio | [bytes](#bytes) |  | One frame of microphone audio in the format given in Start, ideally 20 ms (640 bytes of PCM). At most 16 KiB (512 ms); a larger frame closes the stream. The server buffers audio in a fixed-size ring and counts overflow, so a client that sends faster than real time loses audio. |
| control | [TalkControl](#aulo-v1-TalkControl) |  |  |






<a name="aulo-v1-TalkResponse"></a>

### TalkResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| event | [Event](#aulo-v1-Event) |  |  |
| tts_audio | [TtsAudio](#aulo-v1-TtsAudio) |  |  |






<a name="aulo-v1-TalkStart"></a>

### TalkStart



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| chat_id | [string](#string) |  | The chat that receives the final transcripts. ULID. |
| format | [AudioFormat](#aulo-v1-AudioFormat) |  | Format of every audio frame that follows. |






<a name="aulo-v1-TtsAudio"></a>

### TtsAudio
Synthesized speech. Sample rate, channel count and duration come in the
TtsChunk event with the same turn_id and seq, which is sent first.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| turn_id | [string](#string) |  | ULID. |
| seq | [uint32](#uint32) |  |  |
| pcm_s16le | [bytes](#bytes) |  | Signed 16-bit little-endian PCM. At most 32 KiB per message. |






<a name="aulo-v1-Voice"></a>

### Voice



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| engine | [string](#string) |  | TTS engine id from [voice.tts], for example &#34;kokoro&#34;. |
| id | [string](#string) |  | Engine-specific voice id. At most 128 bytes. |
| name | [string](#string) |  | Display name; untrusted when a plugin engine supplies it. |
| language | [string](#string) |  | BCP 47 tag. |





 


<a name="aulo-v1-AudioFormat"></a>

### AudioFormat


| Name | Number | Description |
| ---- | ------ | ----------- |
| AUDIO_FORMAT_UNSPECIFIED | 0 |  |
| AUDIO_FORMAT_PCM_S16LE_16KHZ_MONO | 1 | Signed 16-bit little-endian PCM, 16 kHz, mono: the default for remote clients. Opus is added later as another value (T6.7). |



<a name="aulo-v1-TalkControl"></a>

### TalkControl
Push-to-talk and playback control inside a Talk stream.

| Name | Number | Description |
| ---- | ------ | ----------- |
| TALK_CONTROL_UNSPECIFIED | 0 |  |
| TALK_CONTROL_START_LISTENING | 1 |  |
| TALK_CONTROL_STOP_LISTENING | 2 |  |


 

 


<a name="aulo-v1-VoiceService"></a>

### VoiceService
Server-side voice pipeline for clients that have no audio stack of their own
(remote clients) or want the daemon&#39;s. Scope: `voice`; SetVoice also needs
`control`.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Talk | [TalkRequest](#aulo-v1-TalkRequest) stream | [TalkResponse](#aulo-v1-TalkResponse) stream | The client streams microphone audio and control; the server runs VAD, STT, the agent and TTS and streams back state, transcripts and synthesized audio. The first request must be a Start; anything else closes the stream with FAILED_PRECONDITION. The stream carries only events about this client&#39;s voice session: VoiceStateChanged, SpeechStarted, SpeechEnded, Transcript, TtsChunk, Notice, ApprovalRequired and TakeoverRequested. |
| StartListening | [StartListeningRequest](#aulo-v1-StartListeningRequest) | [StartListeningResponse](#aulo-v1-StartListeningResponse) | Push-to-talk for the daemon&#39;s own microphone (local mode). Does nothing useful for remote clients, which use TalkControl on their stream. |
| StopListening | [StopListeningRequest](#aulo-v1-StopListeningRequest) | [StopListeningResponse](#aulo-v1-StopListeningResponse) |  |
| ListVoices | [ListVoicesRequest](#aulo-v1-ListVoicesRequest) | [ListVoicesResponse](#aulo-v1-ListVoicesResponse) | Lists the voices of the active or named TTS engine. |
| SetVoice | [SetVoiceRequest](#aulo-v1-SetVoiceRequest) | [SetVoiceResponse](#aulo-v1-SetVoiceResponse) | Takes effect from the next utterance; a sentence being spoken finishes. |

 



## Scalar Value Types

| .proto Type | Notes | C++ | Java | Python | Go | C# | PHP | Ruby |
| ----------- | ----- | --- | ---- | ------ | -- | -- | --- | ---- |
| <a name="double" /> double |  | double | double | float | float64 | double | float | Float |
| <a name="float" /> float |  | float | float | float | float32 | float | float | Float |
| <a name="int32" /> int32 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint32 instead. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="int64" /> int64 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint64 instead. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="uint32" /> uint32 | Uses variable-length encoding. | uint32 | int | int/long | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="uint64" /> uint64 | Uses variable-length encoding. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum or Fixnum (as required) |
| <a name="sint32" /> sint32 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int32s. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sint64" /> sint64 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int64s. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="fixed32" /> fixed32 | Always four bytes. More efficient than uint32 if values are often greater than 2^28. | uint32 | int | int | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="fixed64" /> fixed64 | Always eight bytes. More efficient than uint64 if values are often greater than 2^56. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum |
| <a name="sfixed32" /> sfixed32 | Always four bytes. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sfixed64" /> sfixed64 | Always eight bytes. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="bool" /> bool |  | bool | boolean | boolean | bool | bool | boolean | TrueClass/FalseClass |
| <a name="string" /> string | A string must always contain UTF-8 encoded or 7-bit ASCII text. | string | String | str/unicode | string | string | string | String (UTF-8) |
| <a name="bytes" /> bytes | May contain any arbitrary sequence of bytes. | string | ByteString | str | []byte | ByteString | string | String (ASCII-8BIT) |

