# T16.6. Unwired store/grants surface

`Store.listChats` in `aulo-store` plus `create_bot`/`append_message`/`record_tool_call`/`record_usage` and the whole grants/audit surface (the grants/audit surface) have zero non-test callers — the daemon never opens the database (`modules/aulo/src/daemon/serve.kt` mounts no service). Expected before T4/T5 land, but until then this code runs only under test. Done means: an integration test drives the store and one grant through `aulod` (or the surface is explicitly gated).
