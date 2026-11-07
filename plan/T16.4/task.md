# T16.4. A stale partial can overwrite a newer held transcript

`crates/aulo-speech-system/src/macos/stt.rs`: when a final arrived before `finish`, the outcome was delivered as a partial and held — but items still queued in the older `partials` channel were not cleared, so the next poll drained one and delivered an older partial ("open the") over the newer held text ("Open the door.") until `finish` released the held final. Found by the 2026-10-07 audit. Fix: holding an outcome drops every partial queued behind it.
