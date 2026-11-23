# Connect

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11229) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11592) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-9008) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-7621) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=69-8983) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-16023)

## Purpose

What the app shows when it starts and has never reached a server, or the stored server does
not answer and there is nothing in the local cache to show.

## Layout

A centred block on the window background (desktop, web) or the screen (phones), about 360 dp
wide:

1. The aulo mark (a 48 dp rounded square in the accent colour with a white "a").
2. Title "Connect to aulo".
3. Muted line "Start the aulo-chat server, then enter its address."
4. Text field "Address", prefilled with the default `127.0.0.1:50051`.
5. Primary button "Connect", full width of the block.
6. Status line under the button: empty, "Connecting…" with a small spinner, or the error in
   the danger colour.
7. Muted footnote with the command that starts the development server:
   `./gradlew :server:run`, in the kit's monospace style.

Desktop keeps the window chrome (traffic lights, Windows caption buttons, Adwaita header bar)
and no sidebar.

## Components

Text field, primary button, progress indicator (spinner) of each kit; the aulo mark.

## States

| State | What the user sees |
| --- | --- |
| Idle | Field and button enabled. |
| Connecting | Button disabled with a spinner, status "Connecting…". |
| Failed | Status "Cannot reach <address>: <reason>." in the danger colour; field and button enabled. |
| Invalid address | Field error "Use host:port, port 1–65535."; button disabled. |

## Behaviour

- On success the app opens the newest chat or the empty state, and stores the address.
- When the cache has chats, the app never shows this screen: it opens the cache and shows the
  offline banner instead (`offline.md`).
- Enter in the field connects.

## Errors and offline

This screen is itself the offline state for a first start.

## Code

`ConnectScreen.kt`, shown by `App.kt` while `ChatUiState.needsServer` is true; the address
field and status line are shared with `SettingsScreen.kt`.
