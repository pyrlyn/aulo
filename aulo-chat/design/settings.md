# Settings

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11081) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11520) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-8892) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-8684) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=69-8552)

## Purpose

Change what belongs to this device rather than to one chat: which server the app talks to,
the theme, and the model new chats start with.

## Layout

- **Phones (iOS, Android):** a full screen pushed from the conversation list (gear button in
  its top bar), with a back button and the title "Settings". Grouped list in the platform
  kit's style (inset grouped on iOS, Material list with section headers on Android).
- **Desktop (macOS, Windows 11, Linux):** a settings window of about 560 × 480, opened from
  the gear button at the bottom of the sidebar or with Cmd+, (macOS) / Ctrl+, (Windows,
  Linux). macOS: a window with a toolbar title; Windows: a Mica window with a left-aligned
  title; Linux: an Adwaita preferences dialog with rounded corners.
- **Web:** a modal dialog over the dimmed page, the same width as on desktop.

Sections, top to bottom:

1. **Server.** Text field "Address" (`host:port`, placeholder `127.0.0.1:50051`), a muted
   status line under it ("Connected", "Connecting…" or the last error), and a "Connect"
   button that is enabled only when the field differs from the current address.
2. **Appearance.** A three-way segmented control: System, Light, Dark. System follows the OS.
3. **New chats.** "Default model": the platform kit's dropdown or menu, listing the server's
   models; disabled while offline.
4. **Device.** A muted line "The 20 most recent chats are kept on this device." and a
   destructive text button "Clear local cache".
5. **About.** App name and version, muted.

## Components

| Component | What it is |
| --- | --- |
| Settings group | A titled group of rows in the kit's list style. |
| Text field | The kit's text field with a label; error state shows the message under it in the danger colour. |
| Segmented control | The kit's segmented control (iOS, macOS), segmented button (Android, Windows), or toggle group (Linux, Web). |
| Dropdown | The kit's dropdown or popup button; items are model display names. |
| Destructive button | The kit's text or secondary button in the danger colour. |

## States

| State | What the user sees |
| --- | --- |
| Connected | Status "Connected" with the green dot. |
| Editing the address | "Connect" enabled; the status line keeps the current connection. |
| Connecting | Status "Connecting…", the field and the button are disabled. |
| Invalid address | Field in its error state: "Use host:port, port 1–65535." Connect is disabled. |
| Cannot connect | Status line in the danger colour: "Cannot reach <address>." The previous address stays in use. |
| Offline | "Default model" disabled and shows the stored choice. |

## Behaviour

- The address is checked on the client before anything is sent: a host and a port between 1
  and 65535. The app switches to the new address only after it connects; until then the old
  connection stays.
- The address, theme and default model are stored on the device. `AULO_CHAT_HOST` and
  `AULO_CHAT_PORT` (desktop) still win at start, and the field then shows that address.
- "Clear local cache" asks first (see `chat-actions.md`, confirmation dialog) with the text
  "Clear the chats kept on this device? Chats on the server are not touched." and the
  destructive button "Clear".
- Escape (desktop, web) closes the settings window; changes to the theme apply at once.

## Errors and offline

Offline, the server section shows the last error and the Connect button retries. Nothing else
in settings needs the server except the model list.

## Code

- Screen: `SettingsScreen.kt`. On desktop it is its own window (`Main.kt`, Cmd+, or Ctrl+,
  and the sidebar's Settings button); elsewhere a dialog over the app (`App.kt`).
- State: `SettingsViewModel` over `SettingsStore` (SQLDelight `Setting.sq`); a new address
  switches `SwitchableChatRemote` and reconnects `ChatViewModel`.
- Phones get a Settings entry with their own designs.
