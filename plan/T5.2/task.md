# T5.2. Shell risk classification

Stage: S5 · Area: safety · Depends on: T5.1, T1.10 · Blocks: 1 task(s)

shell-classify sets the risk of shell commands (ReadOnly, Write, Exec, Destructive); risk feeds the default decision when no rule matches.

Done when:

- fixtures: rm -rf is Destructive and asks; ls is ReadOnly
