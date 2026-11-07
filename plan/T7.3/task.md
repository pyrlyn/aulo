# T7.3. Model manager

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 4 task(s)

Download speech models from Hugging Face with pinned SHA-256 (the coding agent whisper-models.json pattern) into ~/.aulo/models, progress events, aulo models pull/ls/rm. Refuse a hash mismatch.


Done when:

- test: corrupted download is rejected
