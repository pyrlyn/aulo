# Quick talk

Figma (desktop): [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-18219) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29786)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-20699) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27063)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-23141) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23873))

## Purpose

Talk to a bot from anywhere without opening the main window. A global hotkey opens a small
floating window over whatever app is in front; the user speaks, releases, and the request
goes to the bot.

## Layout

The frame shows the desktop itself: the system wallpaper with the platform's own bar (macOS
menu bar with "aulo · listening", the Windows 11 taskbar, the Linux top bar). In the middle
sits the quick-talk window:

1. The bot's avatar and name ("Juno") and the state "Listening".
2. The hotkey as keycaps: ⌃ ⌥ Space on macOS, Ctrl + Alt + Space on Windows and Linux.
3. The live transcript ("Remind me to send the invoice when I'm back at my desk").
4. A hint: "Release to send · Esc to cancel".

Each platform draws the window with its own overlay surface (macOS glass, Windows flyout,
Linux popover). The light frame uses the platform's light wallpaper.

## Behaviour

- The hotkey is set in Settings → Devices → Hotkeys; it opens quick talk with the last used
  bot.
- Releasing the hotkey sends the transcript; Esc cancels and closes the window.
- The answer arrives in the bot's chat and, if speaking is on, aloud; the window closes
  after sending.
- Like every voice input, it cannot approve actions that need a click.

## Code

Not built.
