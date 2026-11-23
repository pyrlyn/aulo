# Approval

Figma (desktop): [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=82-15463) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=82-15779)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=99-15804) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=99-16170)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=101-16183) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=101-16496))

## Purpose

A bot wants to do something the rules do not allow on their own, such as deleting files
outside its workspace. The run stops at an Approval card in the chat until the user decides.
Nothing runs before the decision, and voice cannot make it for actions that need a click.

## Layout

The desktop window (see `desktop.md`) with the Computer panel open, because the bot was using
the computer. The chat shows the spoken request (Voice marker "Voice · 0:06" and the user
bubble), the Tool call card that found the files, the assistant's answer, then the Approval
card. The Bot row of the waiting bot says "Waiting for your approval".

## Components

| Component | What it is |
| --- | --- |
| Approval card | Title ("Move 2 files to Trash?"), body ("Juno wants to delete files outside its workspace. Nothing runs until you decide."), detail (the exact paths, monospace), hint ("Needs a click. Voice cannot approve this."), buttons Allow once, Always allow in Downloads, Deny. |
| Tool call | The command that led here, with its state. |
| Computer panel | The bot's live screen with Take over and Stop (see `screen-viewer.md`). |

## Behaviour

- **Allow once** runs this one call. **Always allow in …** also adds a grant, which then shows
  under Settings → Rules → Grants and can be revoked there. **Deny** tells the bot no.
- Destructive, payment and credential actions accept only a click or a key press; a spoken
  "yes" is shown in the transcript but does not approve.
- Every decision lands in the audit list (Settings → Rules → Audit), for example "Move 2 files
  to Trash · Approved by click".
- While the card is open, the composer stays usable: the user can ask the bot questions
  about the request before deciding.

## Code

Not built. The client has no tool calls or approvals yet; they arrive with aulod's policy
service (spec §7, §11).
