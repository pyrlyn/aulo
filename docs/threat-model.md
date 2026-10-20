# Threat model

This is the map required by spec §7 and T5.13. It names the trust boundaries around
the daemon and, for each threat, the guard that covers it.

A guard cell is either a test that already runs, written as
`test:` `path` `function`, or a plan task that builds the guard, written as
`task:` `T<stage>.<n>`. Nothing in the tables is an unnamed control. A task
citation means the control is specified and not shipped yet. T5.1 (the policy
engine every tool call must pass) is the gate most of the later rows depend on;
until it lands, the listener, the store and the config crate are what actually
refuse a request.

## Assets

- The machine the daemon runs on: files, processes, windows, keystrokes.
- Provider keys, the API token, and any other credential the user stores.
- Chats, transcripts and the audit log.
- Money and accounts reached through the browser or an app (payments, banking,
  password managers).

## What is trusted

The operating system reports the local peer (Unix `SO_PEERCRED`, Windows
process token) and enforces file modes. The TLS stack checks the pinned
certificate. Policy code, once T5.1 exists, decides Allow, Deny or Ask outside
the model. The model, a tool result, a web page, a screenshot, an MCP server, a
plugin and a remote client are not trusted. A spoken "yes" is not a click
(spec: voice never widens access).

## Assumptions

- The person who starts `aulod` is the owner. Another local account is an
  attacker. A process that is already running as the owner can do what the
  owner can; the socket and pipe checks do not try to stop that process.
- The daemon fails closed: a request with no `Caller`, a tier with no provider,
  and a tool call that policy cannot decide are refused, not allowed.
- Unattended work (no client attached, or a routine) is read-only. That rule
  is T5.7; it is not enforced yet.

## Pipeline

Spec §7 runs every tool call, including one that arrives through MCP or a
plugin, through the same steps. Each step is a task until the test named in
that task exists.

| Step | What it stops | Guard |
| --- | --- | --- |
| Rules | A call that matches a deny, or that no allow covers | `task:` T5.1, grammar from T1.8 |
| Risk fallback | A shell command no rule named | `task:` T5.2, classifier from T1.10 |
| Blocklist | Banking, payments, crypto, password managers, system settings | `task:` T5.6 |
| Mode | Writes and exec while nobody is at the keyboard | `task:` T5.7 |
| Sentinel | A consequential action the rules allowed but a second model will not | `task:` T5.8. It cannot allow what a rule denied |
| Approval | Anything still marked Ask | `task:` T5.3. Timeout denies. Voice only for low and medium risk: `task:` T8.9 |
| Sandbox | A command that should not touch the rest of the disk | `task:` T5.4, from T1.9 |
| Audit | A later claim that the call did not happen | `task:` T5.9. The store already rejects a forked hash chain: `test:` `crates/aulo-store/src/tests.rs` `audit_rejects_a_forked_chain` |
| Secrets | The model reading a key | `task:` T5.10. Keys already stay out of config and responses; see the secrets boundary below |
| Untrusted content | Instructions hidden in a page, a tool result or a screenshot | `task:` T5.12, sanitizer from T1.11 |
| Kill switch | A turn that should have stopped | `task:` T5.5 |

## Remote client

The client is across TCP. It is authenticated with a bearer token. Off
loopback the connection is TLS with a pinned certificate.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Spoofing | A missing, wrong, duplicate or non-bearer token is accepted | `test:` `crates/aulo-server/tests/auth.rs` `tcp_with_a_wrong_token_is_rejected`, `tcp_without_a_token_is_rejected`; `test:` `crates/aulo-server/src/auth/tests.rs` `missing_malformed_wrong_and_duplicate_tokens_are_refused`. The comparison is SHA-256 and constant-time. The token is stripped before a service sees the request: `the_right_token_is_admitted_and_stripped` |
| Spoofing | A TCP listener starts with no token installed | `test:` `crates/aulo-server/tests/auth.rs` `tcp_without_an_installed_token_is_not_served` |
| Spoofing | A token is used for a scope it was not given | `task:` T13.3 |
| Tampering | The token or the RPC crosses the network in clear | `test:` `crates/aulo-server/tests/serve.rs` `tcp_refuses_non_loopback_without_tls`; `test:` `crates/aulo-server/tests/tls.rs` `tls_still_requires_the_token`, `a_wrong_pin_is_refused`, `a_plaintext_client_is_not_served_by_a_tls_listener` |
| Tampering | A client replays or forges a page cursor | `test:` `crates/aulo-server/tests/chat.rs` `tampered_page_tokens_are_rejected` |
| Repudiation | A remote action leaves no record | `task:` T5.9 |
| Information disclosure | The token or a refusal oracle leaks into logs or status text | `test:` `crates/aulo-server/src/auth/tests.rs` `log_redaction_masks_the_token_under_any_field_name`, `debug_output_never_shows_the_token`; `test:` `crates/aulo-telemetry/tests/logging.rs` `secrets_never_reach_the_file`. Refusals share one message |
| Denial of service | A huge frame or a stuck stream holds the daemon | `test:` `crates/aulo-server/tests/serve.rs` `oversize_request_is_rejected_before_the_handler`, `shutdown_abandons_streams_after_the_grace`; `test:` `crates/aulo-server/tests/chat.rs` `message_limits_apply_to_the_chat_service`. Per-client quotas: `task:` T13.4 |
| Elevation | An authenticated client drives a tool the policy would deny | `task:` T5.1. The listener only decides who the caller is |

## Local client

The local endpoint is a Unix socket or a Windows named pipe. It does not ask
for the API token. The kernel must say the peer is the daemon's user.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Spoofing | Another Unix user connects to the socket | `test:` `crates/aulo-server/src/auth/tests.rs` `only_the_daemon_uid_is_the_owner`, `a_local_peer_without_credentials_is_refused`. A peer with no credentials is refused |
| Spoofing | The socket directory is writable by someone else, or the path is not absolute | `test:` `crates/aulo-server/tests/serve.rs` `a_directory_other_users_can_enter_is_refused`, `a_relative_socket_path_is_refused` |
| Spoofing | Any Windows account that can write the pipe is treated as the owner | `task:` T16.5. `GetNamedPipeClientProcessId` and the client process token are checked before `LocalOwner`; Administrators and SYSTEM must not pass on the default DACL alone |
| Tampering | A live socket, a regular file or a stale socket is replaced carelessly | `test:` `crates/aulo-server/tests/serve.rs` `a_live_socket_is_not_clobbered`, `a_regular_file_is_not_replaced`, `a_stale_socket_is_replaced` |
| Information disclosure | The API token is required on the local socket and so gets copied into local clients | `test:` `crates/aulo-server/tests/auth.rs` `unix_socket_from_the_owner_is_accepted_without_a_token` |
| Elevation | A local client skips policy because it is the owner | `task:` T5.1. Being the owner authenticates the caller; it does not authorize a tool |

## Model

The model proposes text and tool calls. It does not decide policy, and it does
not see secret values.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Spoofing | The model says the user already approved | `task:` T5.3. Approval is a client decision. A spoken yes does not approve a destructive, payment or credential action: `task:` T8.9 |
| Tampering | The model names a tool that is not registered, or a deferred tool it has not been shown | `task:` T4.7, then T5.1 |
| Tampering | A cheap model is asked to do the main agent's work, or the sentinel tier is empty and the call proceeds | `test:` `crates/llm-router/tests/route.rs` `two_tiers_route_to_two_providers`, `a_missing_tier_fails_closed`. Wiring tiers from config: `task:` T4.5 |
| Information disclosure | A key is placed in a config file, a prompt, a transcript or a log | `test:` `crates/aulo-config/tests/layers.rs` `inline_api_key_names_the_file_and_hides_the_value`; `test:` `crates/aulo-config/tests/render.rs` `mask_hides_url_userinfo_and_credential_pairs`; `test:` `crates/aulo-server/src/config_api.rs` `set_provider_key_never_comes_back`. Handles instead of values: `task:` T5.10 |
| Denial of service | The model loops on tools | `task:` T4.6 (max steps, cancellation) |
| Elevation | The model calls a tool a rule denied, and a sentinel "allow" overrides that deny | `task:` T5.1 fails closed. `task:` T5.8: the sentinel can only make the decision stricter |

## Tools and computer control

Tools are how the model touches the computer. Each subject (shell, app, site,
file, input) is a policy subject in T5.1. The rows below are the threats that
subject has to cover. None of these tools ship yet; the guards are the tasks
that build them.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Tampering | `rm -rf` or another destructive command runs because no rule matched | `task:` T5.2 classifies it Destructive and asks. `task:` T9.1 caps output, times out and kills the process tree |
| Tampering | A command writes outside the workspace | `task:` T5.4. Retry unsandboxed only after approval |
| Elevation | An app or a site on the blocklist is opened under a broad allow | `task:` T5.6. Banking, payments, crypto, password managers and system settings always ask or deny. App launch: `task:` T9.3, T9.4, T9.5. Browser: `task:` T9.6 |
| Elevation | Synthetic input clicks a payment button because the user said "yes" | `task:` T9.16 for the input tool, `task:` T8.9 so voice cannot authorize it |
| Tampering | The model acts on the wrong window | `task:` T9.11 (macOS accessibility refs), T9.13 (Windows UIA), T9.18 (the control ladder stops a batch on failure) |
| Denial of service | A PTY or a browser session grows without a cap | `task:` T9.2 (capped PTY buffers), T9.6 (managed Chromium) |
| Repudiation | A computer-control call is missing from the audit log | `task:` T5.9 |

## MCP servers

An MCP server is a separate process or a remote HTTP endpoint. Its tools are
just more tool calls. A broken server must not take the daemon down.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Denial of service | One server fails to connect and the daemon aborts | `task:` T1.12 and T10.1. A broken server becomes a notice |
| Spoofing | A remote MCP server is used without authenticating | `task:` T1.12 (OAuth with an injectable store), T10.2 |
| Tampering | An MCP tool runs under a name that hides which server it is, or before policy | `task:` T4.7 (`mcp__server__tool`), `task:` T5.1 on every call. Deferred until searched: T10.1 |
| Elevation | aulo's own MCP server lets an external agent skip approvals | `task:` T10.5, T10.6, T10.7. Callers still pass policy |
| Tampering | Installing aulo into another agent's config rewrites that file | `task:` T1.18 and T10.4. Touch only aulo's entry, keep foreign bytes, write atomically, back up |
| Information disclosure | An MCP OAuth token reaches the model | `task:` T5.10 |

## Plugins

A plugin is code the user installed. The manifest is checked before it runs.
A new digest or a new capability needs approval again (T11.2).

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Tampering | The manifest points at a binary outside the package, or names a capability the kind cannot have | `test:` `crates/aulo-plugin/tests/manifest.rs` `entry_paths_must_stay_inside_the_package`, `entry_must_match_the_declared_kinds`, `fs_roots_must_be_absolute_and_specific`, `net_hosts_must_be_well_formed` |
| Tampering | A huge or hostile manifest is parsed whole | `test:` `crates/aulo-plugin/tests/manifest.rs` `oversized_text_is_rejected_before_parsing`, `error_messages_escape_and_truncate_hostile_values` |
| Elevation | A plugin gains a capability it was not granted, or a new build keeps the old grant | `task:` T11.2 (grant bound to the package digest). WASM memory and deadline: `task:` T11.7 |
| Elevation | A plugin tool bypasses policy | `task:` T5.1, with the name `plugin__id__tool` from T4.7 |
| Denial of service | A broken plugin stops voice or the daemon | Spec: fail open on extensions. The engine registry already skips a failed engine: `test:` `crates/aulo-voice/tests/registry.rs` `a_failing_engine_falls_back_with_a_notice`. A plugin cannot steal a built-in id: `a_plugin_cannot_take_over_a_registered_id` |
| Tampering | A hook's stdout is treated as instructions, or a crashing hook blocks the turn | `task:` T11.9 and T1.17. Exit code 2 blocks; any other failure is a warning and the turn continues |
| Elevation | Piper (GPL) is linked into the daemon | `task:` T11.6. It runs as a separate plugin process. espeak-ng inside sherpa-onnx is the one accepted exception (spec D9) |

## Web pages, screenshots and tool output

This is the prompt-injection boundary. The bytes are data. They are not
instructions, even when they say they are.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Tampering | A page says "ignore previous instructions" and the next consequential action runs | `task:` T5.12 wraps and sanitizes (T1.11), and forces Ask. `task:` T5.8 may escalate. `task:` T5.6 keeps the blocklist in front of a broad allow |
| Elevation | The same injection runs while the user is away | `task:` T5.7. Unattended work cannot write |
| Tampering | A search query is interpreted as FTS syntax | `test:` `crates/aulo-server/tests/chat.rs` `hostile_search_text_is_a_plain_query`; `test:` `crates/aulo-store/src/search/tests.rs` `hostile_queries_are_plain_words_not_errors` |
| Information disclosure | A screenshot or a page is copied into the prompt without a size cap | `task:` T5.12. Chat and search inputs are already capped: `test:` `crates/aulo-server/tests/chat.rs` `search_rejects_bad_input_without_echoing_it` |

## Speech, cloud engines and model files

Cloud speech sends audio to someone else's server. Model files are downloaded.
Both are supply-chain and credential boundaries.

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Spoofing | A cloud engine dials a host the config did not name | `test:` `crates/aulo-speech-cloud/tests/deepgram.rs` `the_endpoint_is_validated_before_any_connection`; `test:` `crates/aulo-speech-cloud/tests/elevenlabs.rs` `the_config_is_validated_before_any_connection` |
| Information disclosure | An API key appears in `Debug`, a close code or a log | `test:` `crates/aulo-speech-cloud/tests/deepgram.rs` `the_key_stays_out_of_debug_output`; `test:` `crates/aulo-speech-cloud/tests/transcribe.rs` `the_key_never_shows_in_debug_output`; `test:` `crates/aulo-speech-cloud/tests/deepgram.rs` `a_close_code_is_reported_as_a_number_only` |
| Tampering | A downloaded model does not match the pinned hash, or the body exceeds the pinned size | `test:` `crates/aulo-models/tests/pull.rs` `a_corrupted_download_is_rejected`, `a_body_over_the_pinned_size_is_cut_off_and_deleted`, `a_corrupt_partial_fails_the_hash_and_is_deleted` |
| Denial of service | A cloud transcript or audio queue grows without bound | `test:` `crates/aulo-speech-cloud/tests/deepgram.rs` `the_transcript_is_capped_at_the_pipeline_limit`, `a_huge_message_fails_the_utterance`, `a_full_audio_queue_overflows_instead_of_blocking` |
| Information disclosure | A benchmark or an engine with no key opens a connection anyway | `test:` `crates/aulo-speech-bench/src/tests.rs` `engines_without_a_key_stay_unbuilt_and_off_the_network` |

## Config, grants and the audit log

| STRIDE | Threat | Guard |
| --- | --- | --- |
| Tampering | An unknown config key is ignored, so a typo silently drops a safety setting | `test:` `crates/aulo-config/tests/layers.rs` `unknown_key_names_the_file`, `unknown_top_level_section_is_rejected`. The committed schema rejects unknown keys: `test:` `crates/aulo-config/tests/schema.rs` `schema_rejects_unknown_keys_and_has_no_null` |
| Information disclosure | `aulo config show` prints a key or a URL password | `test:` `crates/aulo-config/tests/render.rs` `toml_output_masks_every_field_that_could_carry_a_secret` |
| Tampering | Two writers fork the audit hash chain | `test:` `crates/aulo-store/src/tests.rs` `audit_rejects_a_forked_chain`. Detecting an edited row: `task:` T5.9 |
| Elevation | An expired grant is still applied | `test:` `crates/aulo-store/src/tests.rs` `active_grants_filter_by_subject_and_expiry`. Deciding which grant wins, and persisting an "always" grant: `task:` T5.3 |
| Repudiation | A tool call is not in the log the user can verify | `task:` T5.9 (`aulo audit verify`, `aulo audit export`) |

## What this document does not claim

- T5.1 is not implemented. Tool calls are not yet gated by policy, because the
  tools themselves are not implemented. The tests cited above are the gates
  that do run today: authentication, TLS, size caps, secret redaction, model
  hashes, manifest checks.
- A process running as the daemon's user is inside the trust boundary. Local
  authentication does not protect against that user.
- macOS CI and a Windows runner execute the platform-specific tests. A Linux
  host does not run the named-pipe probe (T16.5) or the macOS speech tests.
