# Voice

Figma (desktop): [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-16945) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29621)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19261) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26285)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-21779) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23679))

## Purpose

Talking to a bot instead of typing. The user holds push to talk (or says the wake word);
the chat shows a live transcript while they speak and the bot answers in text and, if
enabled, aloud.

## Layout

The desktop window (see `desktop.md`). The messages show an earlier spoken turn ("Voice ·
0:06", the transcribed user bubble and the answer), then the turn in progress: a Voice marker
"Listening · 0:04" and a user bubble with the partial transcript at reduced opacity. The
Activity line says "Listening…", and the Bot row says the same.

The composer is replaced by the **voice recorder**: a recording dot, a level meter, the
elapsed time and a Stop button; the push-to-talk button next to it stays highlighted.

## Behaviour

- Push to talk is a hotkey (Settings → Devices → Hotkeys) or the button beside the composer.
  Releasing it sends the transcript as the user's message.
- The partial transcript updates as speech recognition streams; it is replaced by the final
  text when the turn ends.
- Stop discards the recording; nothing is sent.
- If the speech engine fails, the next engine in Settings → Voice takes over; voice is never
  the only way in, the composer comes back when recording stops.
- A spoken approval does not approve destructive, payment or credential actions (see
  `approval.md`).

## Code

Not built. Speech input and output come with aulod's `VoiceService` (spec §5, §11).
