Outcome: the daemon opens `{home}/aulo.db` and mounts `ChatService` and `ApprovalService` list/revoke on one connection. `crates/aulo/tests/persist.rs` starts `aulod`, reads one grant from that file, creates and lists a chat, and revokes the grant over the local socket. `Decide` answers not-found and does not insert a row. Message, tool-call, usage and audit writes stay on the store for T4.8 and T5.9; `create_bot` stays for T14.1.

The task as the old `done.md` described it:

`aulod` never opened `~/.aulo/aulo.db`, so chat, grant and audit methods ran only under test.

The outcome as the old `plan.md` card recorded it:

Outcome: `aulod` opens `{home}/aulo.db` and mounts `ChatService` plus the list/revoke half of `ApprovalService` on that connection. `crates/aulo/tests/persist.rs` starts the binary, lists a grant seeded in that file, creates and lists a chat, and revokes the grant over the local socket. `Decide` returns not-found and does not insert a grant; message, tool, usage and audit writes stay library entry points for T4.8 and T5.9.
