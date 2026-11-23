# Themes

Every screen exists in a dark and a light theme. The screens in the other files are drawn
dark; the frames named "<Platform> / <Screen> · Light" in the Aulo Figma file are
the same layers with each kit's colour collection switched to its Light mode.

## Rules

- Colours, radii and text styles in the frames are bound to the kit's variables and styles;
  a frame never holds a hardcoded colour. A light frame differs from its dark frame only in
  the variable mode.
- Settings → Appearance chooses System, Light or Dark (`settings.md`). System follows the OS
  setting and changes live.
- The code reads the same roles (`ChatColors` in `composeApp/.../theme/PlatformTokens.kt`)
  with a dark and a light value per platform, taken from that kit's variables.
- Glass surfaces (Linux popovers, macOS sidebar) keep their blur in both themes. The app
  draws them as flat colours: Compose has no backdrop blur.
- Where a kit's light accent is too pale to read as text (Linux mint), the code's `accent`
  takes the kit's accent-text tone and the user bubble keeps the fill.

## Light modes

| Kit | Collection and mode |
| --- | --- |
| macOS | "Desktop · Color" and "macOS · Color", Light |
| Windows 11 | "Windows · Color" and "Windows · Color (Mailune)", Light |
| Linux | "Linux · Color" and "Linux · Color (Pyrlyn)", Light (values from libadwaita's light palette) |
| iOS, Android | "iOS · Color", "Android · Color", Light |

In the iOS, Android and Web sections the light frames sit to the
right of the dark ones, in the same order. In the macOS, Windows 11 and Linux sections the
light frames form the row under the dark ones (`desktop.md`). Frames that show the desktop
itself (Quick talk) also swap the wallpaper for the platform's light one where the kit has it.
