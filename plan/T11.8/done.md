Outcome: `crates/aulo-skills` discovers skills under the aulo home and a project's `.aulo/skills`. The index omits the body; `load_body` returns it. A malformed file warns and is skipped. The tool spec is deferred.

The outcome as the old `plan.md` card recorded it:

Outcome: `aulo-skills` loads `SKILL.md` through `agent-ext` from `<home>/skills` and `<project>/.aulo/skills`. The prompt index is one line per skill and does not contain the body. `Skills::load_body` returns the body, capped at 64 KiB; an unknown name is an error. A broken file is a notice and is skipped. A project skill replaces a home skill of the same name. `agent-ext` is pinned at `319d363` of pyrlyn/crates. Registering the tool in the agent registry stays T4.7.
