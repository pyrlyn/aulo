# T16.6. Unwired store/grants surface

`Store::list_chats` (`aulo-store/src/lib.rs:142-149`) plus `create_bot`/`append_message`/`record_tool_call`/`record_usage` and the whole grants/audit surface (`safety.rs:13-107`) have zero non-test callers — the daemon never opens the database (`crates/aulo/src/daemon/serve.rs` mounts no service). Expected before T4/T5 land, but until then this code runs only under test. Done means: an integration test drives the store and one grant through `aulod` (or the surface is explicitly gated).
