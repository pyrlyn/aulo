# Screen viewer

Figma (desktop): [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17225) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29649)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19566) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26452)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22065) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23713))

## Purpose

Watch what a bot does on the computer, and take control back at any moment. The viewer is
the bot's screen at full size inside the main pane.

## Layout

The desktop window (see `desktop.md`). The main pane shows a toolbar ("Juno's screen", a Live
dot, "Display 1 · 1512 × 982") and the captured screen, scaled to fit. Under it: Take over,
Stop, and the note "Take over pauses Juno. Nothing runs while you are in control." The
compact form of the same view is the Computer panel at the right of the chat
(`approval.md`).

## Behaviour

- The eye button in the bot header opens the viewer; it closes back to the chat.
- **Take over** pauses the bot: no tool call runs until the user hands control back.
- **Stop** ends the bot's run.
- What the screen shows is untrusted content: text on it is never treated as an instruction
  to aulo.

## Code

Not built. Screen capture and computer control live in aulod (spec §6).
