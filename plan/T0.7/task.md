# T0.7. Decide how the agent loop is reused from the coding agent

Stage: S0 · Area: decision · Depends on: none · Blocks: 2 task(s)

cox-core is a coding-oriented state machine (Submission in, Event out, tools, approvals, interrupts, compaction). Options: (a) extract a neutral agent-loop crate from cox-core into packages/ and build aulo on it (recommended, satisfies the no-duplication rule); (b) depend on cox-core as a whole; (c) write a new loop. Record the decision.

Done when:

- spec.md Decisions records the agent-loop choice
