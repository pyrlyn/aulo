//! The [`InputBackend`] over `enigo`: real synthetic events on macOS, a
//! refusing stub elsewhere.
//!
//! macOS needs Accessibility permission for synthetic input; `Enigo::new`
//! fails without it, which is the probe T9.17 will surface. enigo's Linux
//! backend links libxkbcommon, which the CI runners do not install, so the
//! real backend is macOS-only for now.

use super::{InputBackend, InputError, KeyStroke, MouseButton};
#[cfg(target_os = "macos")]
use super::{KeyBase, NamedKey};
use aulo_screen::Pixel;

/// The real backend, on macOS.
#[cfg(target_os = "macos")]
pub struct EnigoBackend {
    enigo: enigo::Enigo,
}

#[cfg(target_os = "macos")]
impl EnigoBackend {
    /// Creates the backend. Fails when the process lacks Accessibility
    /// permission, or when the event source cannot be created.
    ///
    /// # Errors
    ///
    /// [`InputError::Backend`] with the platform's reason.
    pub fn new() -> Result<Self, InputError> {
        let settings = enigo::Settings {
            // A dropped Enigo releases stuck keys; that is the safe default
            // for a daemon that may be killed mid-batch.
            release_keys_when_dropped: true,
            ..enigo::Settings::default()
        };
        let enigo = enigo::Enigo::new(&settings).map_err(|e| InputError::Backend {
            reason: e.to_string(),
        })?;
        Ok(Self { enigo })
    }
}

#[cfg(target_os = "macos")]
impl std::fmt::Debug for EnigoBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnigoBackend").finish_non_exhaustive()
    }
}

#[cfg(target_os = "macos")]
impl InputBackend for EnigoBackend {
    fn move_to(&mut self, at: Pixel) -> Result<(), String> {
        use enigo::{Coordinate, Mouse};
        // enigo's absolute origin is the top-left of the main display, the
        // same origin aulo-screen's captures use.
        self.enigo
            .move_mouse(at.x as i32, at.y as i32, Coordinate::Abs)
            .map_err(|e| e.to_string())
    }

    fn button(&mut self, button: MouseButton, down: bool) -> Result<(), String> {
        use enigo::{Direction, Mouse};
        let direction = if down {
            Direction::Press
        } else {
            Direction::Release
        };
        self.enigo
            .button(button.into(), direction)
            .map_err(|e| e.to_string())
    }

    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
        use enigo::{Axis, Mouse};
        if dx != 0 {
            self.enigo
                .scroll(dx, Axis::Horizontal)
                .map_err(|e| e.to_string())?;
        }
        if dy != 0 {
            self.enigo
                .scroll(dy, Axis::Vertical)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), String> {
        use enigo::Keyboard;
        self.enigo.text(text).map_err(|e| e.to_string())
    }

    fn key(&mut self, stroke: &KeyStroke) -> Result<(), String> {
        use enigo::{Direction, Key, Keyboard};
        let mut held = Vec::new();
        if stroke.meta {
            held.push(Key::Meta);
        }
        if stroke.control {
            held.push(Key::Control);
        }
        if stroke.alt {
            held.push(Key::Alt);
        }
        if stroke.shift {
            held.push(Key::Shift);
        }
        let base = match stroke.base {
            KeyBase::Char(c) => Key::Unicode(c),
            KeyBase::Named(named) => named.into(),
        };
        // Modifiers go down, the key clicks, modifiers come up in reverse.
        // A failure still releases what is held, so a refused event cannot
        // leave the keyboard stuck.
        let mut result = Ok(());
        for key in &held {
            if let Err(e) = self.enigo.key(*key, Direction::Press) {
                result = Err(e.to_string());
                break;
            }
        }
        if result.is_ok() {
            result = self
                .enigo
                .key(base, Direction::Click)
                .map_err(|e| e.to_string());
        }
        for key in held.iter().rev() {
            let _ = self.enigo.key(*key, Direction::Release);
        }
        result
    }
}

#[cfg(target_os = "macos")]
impl From<MouseButton> for enigo::Button {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => enigo::Button::Left,
            MouseButton::Right => enigo::Button::Right,
            MouseButton::Middle => enigo::Button::Middle,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<NamedKey> for enigo::Key {
    fn from(named: NamedKey) -> Self {
        use enigo::Key;
        match named {
            NamedKey::Enter => Key::Return,
            NamedKey::Tab => Key::Tab,
            NamedKey::Escape => Key::Escape,
            NamedKey::Space => Key::Space,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::Up => Key::UpArrow,
            NamedKey::Down => Key::DownArrow,
            NamedKey::Left => Key::LeftArrow,
            NamedKey::Right => Key::RightArrow,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Function(n) => match n {
                1 => Key::F1,
                2 => Key::F2,
                3 => Key::F3,
                4 => Key::F4,
                5 => Key::F5,
                6 => Key::F6,
                7 => Key::F7,
                8 => Key::F8,
                9 => Key::F9,
                10 => Key::F10,
                11 => Key::F11,
                _ => Key::F12,
            },
        }
    }
}

/// The stub: synthetic input is not wired on this platform yet.
#[cfg(not(target_os = "macos"))]
pub struct EnigoBackend;

#[cfg(not(target_os = "macos"))]
impl EnigoBackend {
    /// Always fails: this platform has no backend yet.
    ///
    /// # Errors
    ///
    /// [`InputError::Backend`], always.
    pub fn new() -> Result<Self, InputError> {
        Err(InputError::Backend {
            reason: "synthetic input is implemented on macOS only".to_owned(),
        })
    }
}

#[cfg(not(target_os = "macos"))]
impl std::fmt::Debug for EnigoBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnigoBackend").finish_non_exhaustive()
    }
}

#[cfg(not(target_os = "macos"))]
impl InputBackend for EnigoBackend {
    fn move_to(&mut self, _at: Pixel) -> Result<(), String> {
        Err("synthetic input is implemented on macOS only".to_owned())
    }

    fn button(&mut self, _button: MouseButton, _down: bool) -> Result<(), String> {
        Err("synthetic input is implemented on macOS only".to_owned())
    }

    fn scroll(&mut self, _dx: i32, _dy: i32) -> Result<(), String> {
        Err("synthetic input is implemented on macOS only".to_owned())
    }

    fn text(&mut self, _text: &str) -> Result<(), String> {
        Err("synthetic input is implemented on macOS only".to_owned())
    }

    fn key(&mut self, _stroke: &KeyStroke) -> Result<(), String> {
        Err("synthetic input is implemented on macOS only".to_owned())
    }
}
