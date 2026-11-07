Outcome: `crates/aulo-unattended` denies writes, exec and input for a routine or a detached client. A routine `write_file` leaves the path absent. An attached interactive run can write, and a routine can read that file. Queuing the denial for approval is T5.3.

The outcome as the old `plan.md` card recorded it:

Outcome: `aulo-unattended` denies write, exec and input when no client is attached or the run is a routine. Reads stay allowed. `write_file` in a routine returns denied and does not create the file; the same bytes write when a client is attached to an interactive run, and a routine can read them back. An oversized payload is refused without writing. Approval queueing for those denials stays T5.3.
