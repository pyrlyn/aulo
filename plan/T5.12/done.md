Outcome: `crates/aulo-untrusted` wraps the four sources after `text-sanitize`, and `Latch` turns an injected page into Ask for the next consequential action while still allowing a read. A tag, a zero-width character, or a terminal colour sequence does not hide the phrase. Text past the 256 KiB scan cap is not read and still asks.

The outcome as the old `plan.md` card recorded it:

Outcome: `aulo-untrusted` sanitizes web pages, tool output, screenshot text and plugin output with `text-sanitize`, caps the scan at 256 KiB and the kept body at 64 KiB, and wraps the result so a closing marker inside the text cannot end the wrapper. A phrase such as "ignore previous instructions" (also with a tag or a zero-width character in the middle) arms a latch: the next consequential action asks, and a read does not. A body longer than the scan cap asks too, without reading the unread tail. `text-sanitize` is pinned at `319d363` of pyrlyn/crates.
