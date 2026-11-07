# T5.7. Unattended mode is read-only

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 1 task(s)

When no client is attached or the run is a routine, write/exec/input tools are denied or queued for approval; reads still work.

Done when:

- test: routine run cannot write a file
