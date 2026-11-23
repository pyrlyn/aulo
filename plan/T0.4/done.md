# T0.4. Done

`spec.md` §17 **D4**, rewritten for Kotlin/Native in the reset (#81), records the approval
rule instead of a fixed list: the Rust crate list does not carry over; the task that first
wires a module chooses a Kotlin/Native library, a cinterop binding or a small in-tree
implementation, records its licence and adds a `toolchain.md` row. A library that is not an
obvious equivalent still needs the creator's approval in that task.

The reset removed this report together with every other `done.md`. Closed again in the pull
request that adds this file, on the creator's word.

Checked: `spec.md` §17 D4 is present on `main` at 9d360a6.
