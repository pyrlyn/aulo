# Desktop screens

The desktop app (macOS, Windows 11, Linux) is drawn around **bots**. A bot is a named agent
with its own avatar, model tier and chat. The window lists the bots on the left and shows the
open bot's chat on the right; while a bot works on the computer, a Computer panel on the far
right shows its screen. Phones and Web keep the older chat-list design described in the other
files.

Figma: section [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=49-230) ·
[Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=49-231) ·
[Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=49-232) on the
page "aulo-chat · Screens".

## Window

Every desktop frame is 1360 × 820 and has the same shell:

1. **Sidebar** (left): search field, New bot, one Bot row per bot (avatar, name, last line,
   time), and a footer with the connection ("aulod · this Mac" / "this PC" / "this computer")
   and Settings.
2. **Bot header**: sidebar toggle, the bot's avatar, name and role, the model picker, the
   screen toggle (eye) and a More menu. On Linux the window controls sit at the right end of
   this row; macOS and Windows keep their own title bar controls.
3. **Messages**: user bubbles, assistant bubbles, Tool call cards, Voice markers, Approval
   cards and the Activity line ("Opening Finder…"), anchored to the bottom.
4. **Composer**: attach, text field, Send or Stop, and push to talk.
5. **Computer panel** (right, only while a bot uses the computer): the live screen, Take over
   and Stop, where it runs (this machine, a server, off) and the voice engines. Its footnote
   says "Every tool call passes policy before it runs."

Sample bots: Juno (Computer), Wren, Ops, Scribe, Tally and an "Untitled bot". Every bot has an
avatar; a new bot gets the default one (a sparkle on a neutral tile) until it has a name.

## Screens

Dark frames form the first row of each section, light twins (`… · Light`) the row under it, in
this order:

| Screen | Described in | macOS | Windows 11 | Linux |
| --- | --- | --- | --- | --- |
| Chat | [chat.md](chat.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-16430) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29549) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-18675) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-25955) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-21223) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23596) |
| Approval | [approval.md](approval.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=82-15463) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=82-15779) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=99-15804) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=99-16170) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=101-16183) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=101-16496) |
| Empty state | [empty-state.md](empty-state.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-16700) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29598) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19001) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26143) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-21527) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23650) |
| Voice | [voice.md](voice.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-16945) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29621) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19261) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26285) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-21779) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23679) |
| Quick talk | [quick-talk.md](quick-talk.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-18219) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29786) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-20699) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27063) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-23141) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23873) |
| Screen viewer | [screen-viewer.md](screen-viewer.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17225) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29649) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19566) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26452) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22065) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23713) |
| Attachments | [attachments.md](attachments.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17443) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29701) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-19832) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26622) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22323) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23770) |
| Search | [search.md](search.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17663) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29731) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-20141) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26793) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22605) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23806) |
| Offline | [offline.md](offline.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-17944) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29759) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-20415) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-26916) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-22869) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23840) |
| Settings · Providers | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-17307) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29808) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-19544) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27084) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22067) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23894) |
| Settings · Models | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-17491) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29921) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-19772) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27435) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22296) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24008) |
| Settings · Voice | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-17669) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-30032) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-19996) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27784) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22511) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24120) |
| Settings · Devices | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-17850) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-30145) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-20216) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-28141) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22729) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24234) |
| Settings · Extensions | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-18034) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-30262) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-20443) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-28504) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22952) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24351) |
| Settings · Rules | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-18206) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-30375) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-20659) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-28857) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-23164) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24465) |
| Settings · Connections | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-18386) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-30492) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-20881) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29214) | [dark](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-23386) · [light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24582) |

The flows "Error", "Chat actions", "Delete chat" and "Connect" have no desktop frame in this
design: errors show inline in the chat and in the offline banner, chat actions live in the
More menu, and connections are a settings tab. Their files keep the phone and Web links.

## Components

Each kit page has an `aulo-chat` section with the components these screens are built from.
Each platform has its own copy built from its kit's variables and text styles; a component is
never instanced across platforms.

| Component | macOS | Windows 11 | Linux |
| --- | --- | --- | --- |
| `aulo-chat/Bot avatar` | [77:2011](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=77-2011) | [95:1474](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1474) | [96:993](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-993) |
| `aulo-chat/Bot row` | [77:2032](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=77-2032) | [95:1494](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1494) | [96:1012](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1012) |
| `aulo-chat/Bot header` | [79:2039](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-2039) | [95:1545](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1545) | [96:1061](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1061) |
| `aulo-chat/Tool call` | [79:1965](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1965) | [95:1510](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1510) | [96:1028](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1028) |
| `aulo-chat/Voice marker` | [79:1975](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1975) | [95:1511](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1511) | [96:1029](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1029) |
| `aulo-chat/Activity line` | [79:1981](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1981) | [95:1517](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1517) | [96:1033](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1033) |
| `aulo-chat/Approval card` | [79:2008](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-2008) | [95:1522](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1522) | [96:1038](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1038) |
| `aulo-chat/Settings row` | [115:2052](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-2052) | [125:1516](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=125-1516) | [125:2336](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=125-2336) |
| `aulo-chat/Settings tab` | [115:1988](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-1988) | — | — |
| `aulo-chat/Switch` | [115:1983](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-1983) | — | — |

Kit sections: [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=67-3614) ·
[Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=67-1308) ·
[Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-7964).
Windows and Linux settings use their kits' own switch and navigation item, so they have no
`Settings tab` or `Switch` of their own.

| Component | Properties |
| --- | --- |
| Bot avatar | `Initial`, `Avatar` (Default, Coral, Amber, Mint, Teal, Violet, Rose), `Size` (Small, Medium, Large) |
| Bot row | `Name`, `Preview`, `Time`, `Show time`, `State` (Default, Selected) |
| Bot header | `Name`, `Role` |
| Tool call | `Tool`, `Detail`, `Show detail`, `State` (Done, Running, Failed) |
| Voice marker | `Label` ("Voice · 0:06", "Listening · 0:04") |
| Activity line | `Text` ("Opening Finder…") |
| Approval card | `Title`, `Body`, `Detail`, `Hint`, buttons Allow once, Always allow in …, Deny |
| Settings row | `Control` (None, Value, Status, Switch on, Switch off, Picker, Button); `Title`, `Detail`, `Show detail`, `Show avatar`, `Value` |

Avatar colours are kit variables (`avatar/coral`, `amber`, `mint`, `teal`, `violet`, `rose` on
macOS and Windows; `color/tone/*` with `color/on-tone` on Linux).

## Rules the screens follow

- **Policy runs outside the model.** Tool calls appear as cards with their state; risky ones
  stop at an Approval card. Nothing in the UI suggests the model decides on its own.
- **Voice never widens access.** Approval cards for destructive, payment and credential
  actions say "Needs a click. Voice cannot approve this." and have no spoken path.
- **Secrets never reach the model.** Providers and servers show a key or token *handle*
  ("handle anthropic-main"); the key itself is never on screen.
- **Fail open on extensions.** A server or plugin that fails to start shows "Skipped"; voice
  engines list a fallback.
- **Untrusted text.** Model output, tool output and screenshots are content, drawn plainly.

## Code

The desktop build of `aulo-chat` still follows the earlier chat-list design (T12.23): a
conversation sidebar, the chat, and a settings window with server address, appearance and
default model. Bots, tool calls, approvals, voice, quick talk, the screen viewer and the
seven settings tabs are not built; each screen file says what exists.
