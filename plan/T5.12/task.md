# T5.12. Untrusted-content guards

Stage: S5 · Area: safety · Depends on: T5.1, T1.11 · Blocks: 0 task(s)

Web pages, tool output, screenshot text and plugin output are wrapped as untrusted data, sanitized with text-sanitize, and instruction-like content in them forces Ask for the next consequential action.

Done when:

- fixture: an injected "ignore previous instructions" page forces Ask
