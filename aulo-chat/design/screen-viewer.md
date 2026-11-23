# Screen viewer

Figma (desktop): [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19359) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20411)) · [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21632) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22720)) · [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23978) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25074))

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
