# aulo-agent

One chat session per conversation, running on the neutral `agent-loop`
crate (spec §4, decision D5): `Submission`s go in, `SessionEvent`s come
out, with cancellation, max steps and parallel or exclusive tool dispatch
owned by the loop.

- [`Session`] wraps one `agent_loop::Agent` for one chat and re-keys the
  loop's ids onto `aulo-types` ids.
- [`SessionEvent`] is the daemon's seam: the loop's events mapped onto
  the aulo-types events (`AuloEvent`) plus the agent-loop block the
  `aulo.v1.Event` proto mirrors.
- [`TurnContext`] is the default request assembly (model, system prompt,
  tool specs); the tool specs come from a [`ToolSpecSource`], which the
  tool registry (T4.7) implements.

Policy is not here: every tool call the loop makes passes the
`Approvals` gate the daemon installs (`aulo-policy`, T5.1), outside the
model.
