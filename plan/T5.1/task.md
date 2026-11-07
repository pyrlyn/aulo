# T5.1. aulo-policy: rule engine for every tool call

Stage: S5 · Area: safety · Depends on: T1.8, T4.7 · Blocks: 15 task(s)

Every tool call passes perm-rules before running. Subjects: Shell(cmd), App(bundle), Site(host), File(path), Mcp(server, tool), Input(kind). Decisions: Allow, Deny, Ask. Policy is deterministic code outside the model and fails closed.

Done when:

- table tests for rule grammar per subject
