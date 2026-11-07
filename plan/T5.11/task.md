---
status: todo
priority: P1
complexity: 2
readiness: 0%
agent:
---

# T5.11. Human takeover

Stage: S5 · Area: safety · Depends on: T5.3 · Blocks: 0 task(s)

request_takeover(reason) pauses the turn, notifies every client (desktop notification, spoken prompt) and resumes on "done". Used for passwords, 2FA, CAPTCHA, payments.

Done when:

- test: turn resumes only after the takeover-done submission
