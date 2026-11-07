# T0.1. Decide the licence and how cox code is reused

Stage: S0 · Area: decision · Depends on: none · Blocks: 8 task(s)

cox is GPL-3.0-or-later OR LicenseRef-cox-Royalty-Free, runa is MIT OR Apache-2.0. aulo reuses the provider stack, permissions, sandbox, MCP host and voice capture from cox. Options: (a) license aulo like cox and depend on extracted crates as they are; (b) license aulo MIT OR Apache-2.0 and relicense the extracted crates when they move into packages/. The creator decides; record the answer in spec.md (Decisions) and add LICENSE.

Done when:

- spec.md Decisions lists the licence and the reason
- LICENSE file(s) committed
