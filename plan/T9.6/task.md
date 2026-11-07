---
status: todo
priority: P0
complexity: 4
readiness: 0%
agent:
---

# T9.6. Browser: managed Chromium over CDP

Stage: S9 · Area: control · Depends on: T5.6 · Blocks: 4 task(s)

chromiumoxide drives a dedicated aulo browser profile (never the user's own profile by default). Tools: open URL, read page as an accessibility snapshot with element refs, page text, screenshot. Site(host) subject for policy.

Done when:

- headless Chromium test reads a fixture page snapshot
