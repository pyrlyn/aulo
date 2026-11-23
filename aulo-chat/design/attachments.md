# Attachments

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-8474) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-7801) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17443) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29701)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19832) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26622)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22323) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23770)) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=56-3495)

See `desktop.md` for the desktop window around the composer.

## Purpose

Add things to the message being written: a text file, an image or a voice note. The screen
shows what can be added, what is already staged for this message, and a voice note being
recorded.

## Layout

Dark theme only. The screen is the chat with a surface on top of it, opened from the attach
button of the composer:

- **Phones (iOS, Android):** a bottom sheet with a drag handle; the chat behind it is dimmed.
- **Desktop and web:** a popover anchored above the composer's attach button, about
  310–420 dp wide.

Inside, top to bottom:

1. Title "Add to message".
2. Three action rows: **Text file** ("Sent inside the message"), **Image** ("Staged only:
   not sent yet"), **Voice note** ("Staged only: not sent yet"). Each has a kind icon, a
   label and the muted second line.
3. The staged attachments, one chip each (Windows 11 and Web put a small "Staged" label
   above them).
4. The voice recorder, only while a voice note is being recorded.
5. Footnote, muted: "Text files travel inside the message. Images and voice stay on this
   device for now."

Behind the surface the composer holds the text typed so far (the mockups show "Keep it
within this budget.") and its Send button is enabled.

## Components

| Component | What it is |
| --- | --- |
| Action row | Icon, label, second line. States: default, hover or pressed, disabled. |
| Attachment chip | Kind icon, `name · detail` (file size, or duration for voice), a remove button. Kinds: text file, image, voice note. |
| Voice recorder | Stop button (red), live waveform, elapsed time `m:ss`. |

The mockups stage `budget.txt · 2 KB` and `lisbon-map.png · 340 KB` and show a recording at
0:07.

## States

| State | What the user sees |
| --- | --- |
| Nothing staged | Title, the three action rows, the footnote. |
| Staged | A chip per attachment under the rows; the same chips appear in the composer above the text field after the surface closes. |
| Recording | The voice recorder row with a moving waveform and a running timer. |
| Limit reached | A warning banner in the chat: "A message takes at most 6 attachments." Nothing is added. |
| Sent | The chips move into the user's bubble, above the text, without remove buttons; the staging area is empty. |
| Offline | The attach button is disabled; the surface cannot be opened. |

## Behaviour

- Choosing Text file or Image opens the platform's file picker; cancelling it adds nothing.
  Choosing Voice note starts the recorder; its stop button ends the recording and stages it.
- A staged chip's remove button drops that attachment. Staged attachments belong to the
  message being written and are cleared when it is sent.
- A message may be sent with attachments and no text.
- What is transmitted: the `aulo.v1` `UserTurn` has no attachment field, so a text file's
  content is placed inside the message text, and images and voice notes are not sent at
  all. After a message with such attachments is sent, an info banner says "Images and voice
  notes are not sent yet; the text was."
- File names come from outside the app: draw them as plain text, one line.

## Errors and offline

- A message that, with its text files inlined, is larger than 64 KiB is not sent: an error
  banner gives the size and the limit, and the text and the staged attachments are put back
  in the composer.
- If the connection is lost while sending, a warning banner "The server cannot be reached."
  appears and the text and the staged attachments are put back.
- Offline nothing can be staged; see `offline.md`.

## Code

Implemented as a dropdown menu with the three kinds (`AttachButton` in
`composeApp/src/commonMain/kotlin/aulo/chat/ui/Composer.kt`), not as the sheet or popover of
the mockups, and without the second lines, the staged list inside the surface, the recorder
and the footnote. Staged chips are drawn in the composer (`AttachmentChip` in
`MessageBubble.kt`). Logic: `ChatViewModel.attach` and `removeAttachment`, the
`AttachmentPicker` interface and the `SendMessage` use case. The only picker so far is
`SampleAttachmentPicker`, which returns a made-up file of the chosen kind; real pickers and
the recorder are not written.
