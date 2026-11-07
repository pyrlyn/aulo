# T16.3. `OfflineStt::begin` validates the language before cancelling

`crates/aulo-speech-local/src/offline.rs` returned `unsupported` before `self.cancel()`, so an utterance left in `Recording` or `Waiting` survived a failed `begin` — violating the trait contract "`begin` … implies `cancel`" (`aulo-speech/src/stt.rs`) that deepgram and the macOS engine honour. Found by the 2026-10-07 audit. Fix: the cancel runs first; a rejected language still ends the utterance in progress.
