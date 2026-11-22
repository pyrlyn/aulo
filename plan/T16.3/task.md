# T16.3. `OfflineStt::begin` validates the language before cancelling

`modules/aulo-speech-local/src/offline.kt` returned `unsupported` before `self.cancel()`, so an utterance left in `Recording` or `Waiting` survived a failed `begin` — violating the trait contract "`begin` … implies `cancel`" (the STT trait contract) that deepgram and the macOS engine honour. Found by the 2026-10-07 audit. Fix: the cancel runs first; a rejected language still ends the utterance in progress.
