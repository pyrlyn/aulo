//! Synthetic mouse and keyboard input (spec §6.1, T9.16).
//!
//! The last rung of the control ladder: the model names points in the
//! normalized 0–999 coordinates [`aulo_screen`] speaks, a key spec or text,
//! and [`Input`] performs it through an [`InputBackend`]. Everything the
//! model sends is untrusted: key specs and text are validated and capped,
//! coordinates clamp to the screen, and a batch stops at the first failure
//! (spec §6.2).
//!
//! Policy integration point: every action maps to the `Input(kind)` subject
//! via [`InputAction::policy_call`]. The host passes that `perm-rules` call
//! through `aulo-policy` before running the action; until T5.1 lands the
//! subject is defined here on top of `perm-rules` directly.

use aulo_screen::{Norm, Pixel, Size, pixel};

use perm_rules::{Call, Risk};

mod enigo_backend;

pub use enigo_backend::EnigoBackend;

/// Text typed by one `Type` action is capped: it is untrusted model output
/// and a paste-sized string into the wrong field is damage.
pub const TEXT_MAX_BYTES: usize = 64 * 1024;

/// A key spec is capped for the same reason.
pub const KEY_SPEC_MAX_BYTES: usize = 64;

/// Wheel clicks per scroll event, per axis. One click is a 15° notch.
pub const SCROLL_MAX_CLICKS: i32 = 100;

/// The tool name policy rules match on, as `Input(kind)` in spec §6.1.
pub const TOOL: &str = "input";

/// A mouse button a click or drag may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary button.
    Left,
    /// Secondary button; opens context menus.
    Right,
    /// Middle button.
    Middle,
}

/// One synthetic input action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    /// Press and release `button` at `at`.
    Click {
        /// Where, in 0–999.
        at: Norm,
        /// Which button.
        button: MouseButton,
    },
    /// Type `text` into whatever has focus.
    Type {
        /// The text; capped at [`TEXT_MAX_BYTES`].
        text: String,
    },
    /// Press a key or combination, e.g. `enter`, `cmd+c`.
    Key {
        /// The key spec; validated by [`parse_key`].
        key: String,
    },
    /// Turn the wheel at `at`.
    Scroll {
        /// Where the pointer sits, in 0–999.
        at: Norm,
        /// Horizontal wheel clicks, right positive.
        dx: i32,
        /// Vertical wheel clicks, down positive.
        dy: i32,
    },
    /// Press at `from`, move to `to`, release.
    Drag {
        /// Where the button goes down, in 0–999.
        from: Norm,
        /// Where it comes up, in 0–999.
        to: Norm,
    },
}

impl InputAction {
    /// The policy subject kind: `click`, `type`, `key`, `scroll` or `drag`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            InputAction::Click { .. } => "click",
            InputAction::Type { .. } => "type",
            InputAction::Key { .. } => "key",
            InputAction::Scroll { .. } => "scroll",
            InputAction::Drag { .. } => "drag",
        }
    }

    /// The `perm-rules` call this action is judged by: tool `input`, subject
    /// the kind, and the risk the kind carries.
    ///
    /// Typing text is `Write`; every pointer and key action is `Exec`,
    /// because synthetic input acts as the user at the keyboard and can
    /// trigger anything the user could. Under `ApprovalPolicy::OnRequest`
    /// both ask, so no input runs without a decision.
    #[must_use]
    pub fn policy_call(&self) -> Call {
        let risk = match self {
            InputAction::Type { .. } => Risk::Write,
            _ => Risk::Exec,
        };
        Call::new(TOOL, self.kind(), risk)
    }
}

/// Why an action could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    /// The typed text is over [`TEXT_MAX_BYTES`].
    #[error("text is {got} bytes, over the {max} byte cap")]
    TextTooLarge {
        /// Bytes received.
        got: usize,
        /// The cap.
        max: usize,
    },
    /// The key spec is over [`KEY_SPEC_MAX_BYTES`].
    #[error("key spec is over the {max} byte cap")]
    KeySpecTooLarge {
        /// The cap.
        max: usize,
    },
    /// The key spec names no known key or modifier.
    #[error("unknown key `{spec}`")]
    UnknownKey {
        /// The spec as received.
        spec: String,
    },
    /// A wheel amount is over [`SCROLL_MAX_CLICKS`].
    #[error("scroll is {got} clicks, over the {max} click cap")]
    ScrollTooLarge {
        /// The amount received.
        got: i32,
        /// The cap.
        max: i32,
    },
    /// The platform refused the event: no Accessibility permission, or the
    /// event source could not be created.
    #[error("input backend refused: {reason}")]
    Backend {
        /// What the platform said.
        reason: String,
    },
}

/// A named key a key spec may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    /// Return / Enter.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Space bar.
    Space,
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// A function key, 1 through 12.
    Function(u8),
}

/// One key press with its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyStroke {
    /// The key itself: a character or a named key.
    pub base: KeyBase,
    /// Command (macOS) / Meta held.
    pub meta: bool,
    /// Control held.
    pub control: bool,
    /// Option / Alt held.
    pub alt: bool,
    /// Shift held.
    pub shift: bool,
}

/// The key a stroke presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBase {
    /// A single character; typed as Unicode.
    Char(char),
    /// A named key.
    Named(NamedKey),
}

/// Parses a key spec: a base key with any of the modifiers `cmd`, `ctrl`,
/// `alt` and `shift` joined by `+`, e.g. `cmd+shift+z`, `enter`, `a`.
///
/// # Errors
///
/// [`InputError::KeySpecTooLarge`] over [`KEY_SPEC_MAX_BYTES`],
/// [`InputError::UnknownKey`] for an unknown name, a control character, or a
/// spec with no base key.
pub fn parse_key(spec: &str) -> Result<KeyStroke, InputError> {
    if spec.len() > KEY_SPEC_MAX_BYTES {
        return Err(InputError::KeySpecTooLarge {
            max: KEY_SPEC_MAX_BYTES,
        });
    }
    let mut stroke = KeyStroke {
        base: KeyBase::Named(NamedKey::Enter),
        meta: false,
        control: false,
        alt: false,
        shift: false,
    };
    let mut base: Option<KeyBase> = None;
    for part in spec.split('+') {
        match part {
            "" => {
                return Err(InputError::UnknownKey {
                    spec: spec.to_owned(),
                });
            }
            "cmd" | "command" | "meta" | "super" => stroke.meta = true,
            "ctrl" | "control" => stroke.control = true,
            "alt" | "option" => stroke.alt = true,
            "shift" => stroke.shift = true,
            other => {
                if base.is_some() {
                    return Err(InputError::UnknownKey {
                        spec: spec.to_owned(),
                    });
                }
                base = Some(key_base(other).ok_or_else(|| InputError::UnknownKey {
                    spec: spec.to_owned(),
                })?);
            }
        }
    }
    stroke.base = base.ok_or_else(|| InputError::UnknownKey {
        spec: spec.to_owned(),
    })?;
    Ok(stroke)
}

/// The key a spec part means: a named key, or a single character. Anything
/// else (empty, multi-character, control) is `None`.
fn key_base(part: &str) -> Option<KeyBase> {
    named_key(part).or_else(|| char_base(part))
}

/// The named key a spec part means, or `None` for anything unknown.
fn named_key(part: &str) -> Option<KeyBase> {
    let named = match part {
        "enter" | "return" => NamedKey::Enter,
        "tab" => NamedKey::Tab,
        "escape" | "esc" => NamedKey::Escape,
        "space" => NamedKey::Space,
        "backspace" => NamedKey::Backspace,
        "delete" | "del" => NamedKey::Delete,
        "up" | "arrowup" => NamedKey::Up,
        "down" | "arrowdown" => NamedKey::Down,
        "left" | "arrowleft" => NamedKey::Left,
        "right" | "arrowright" => NamedKey::Right,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" => NamedKey::PageUp,
        "pagedown" => NamedKey::PageDown,
        _ => {
            let rest = part.strip_prefix('f')?;
            let n: u8 = rest.parse().ok()?;
            if (1..=12).contains(&n) {
                NamedKey::Function(n)
            } else {
                return None;
            }
        }
    };
    Some(KeyBase::Named(named))
}

/// A single character is a key; anything else is not.
fn char_base(part: &str) -> Option<KeyBase> {
    let mut chars = part.chars();
    let first = chars.next()?;
    chars.next().is_none().then_some(KeyBase::Char(first))
}

/// The platform operations the driver needs. Errors are strings the daemon
/// can log; the driver adds the context.
pub trait InputBackend {
    /// Move the pointer to a pixel on the primary display.
    ///
    /// # Errors
    ///
    /// The platform refused the move.
    fn move_to(&mut self, at: Pixel) -> Result<(), String>;
    /// Press or release a mouse button at the current pointer position.
    ///
    /// # Errors
    ///
    /// The platform refused the event.
    fn button(&mut self, button: MouseButton, down: bool) -> Result<(), String>;
    /// Turn the wheel; amounts are wheel clicks.
    ///
    /// # Errors
    ///
    /// The platform refused the event.
    fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String>;
    /// Type text at the current focus.
    ///
    /// # Errors
    ///
    /// The platform refused the event.
    fn text(&mut self, text: &str) -> Result<(), String>;
    /// Press and release one key stroke.
    ///
    /// # Errors
    ///
    /// The platform refused the event.
    fn key(&mut self, stroke: &KeyStroke) -> Result<(), String>;
}

/// The input driver: an [`InputBackend`] plus the screen its coordinates
/// are normalized against.
#[derive(Debug)]
pub struct Input<B> {
    backend: B,
    screen: Size,
}

impl<B: InputBackend> Input<B> {
    /// Wraps `backend` for a screen of `screen` pixels. The size comes from
    /// the capture the model is looking at, so a point the model names lands
    /// where the screenshot shows it.
    #[must_use]
    pub fn new(backend: B, screen: Size) -> Self {
        Self { backend, screen }
    }

    /// Runs one action. Coordinates clamp to the screen.
    ///
    /// # Errors
    ///
    /// [`InputError`] for a capped or invalid action, or a backend refusal.
    pub fn run(&mut self, action: &InputAction) -> Result<(), InputError> {
        match action {
            InputAction::Click { at, button } => {
                self.pointer(*at)?;
                self.press(*button, true)?;
                self.press(*button, false)?;
            }
            InputAction::Type { text } => {
                if text.len() > TEXT_MAX_BYTES {
                    return Err(InputError::TextTooLarge {
                        got: text.len(),
                        max: TEXT_MAX_BYTES,
                    });
                }
                self.backend
                    .text(text)
                    .map_err(|reason| InputError::Backend { reason })?;
            }
            InputAction::Key { key } => {
                let stroke = parse_key(key)?;
                self.backend
                    .key(&stroke)
                    .map_err(|reason| InputError::Backend { reason })?;
            }
            InputAction::Scroll { at, dx, dy } => {
                for (got, max) in [(*dx, SCROLL_MAX_CLICKS), (*dy, SCROLL_MAX_CLICKS)] {
                    if got.abs() > max {
                        return Err(InputError::ScrollTooLarge { got, max });
                    }
                }
                self.pointer(*at)?;
                self.backend
                    .scroll(*dx, *dy)
                    .map_err(|reason| InputError::Backend { reason })?;
            }
            InputAction::Drag { from, to } => {
                self.pointer(*from)?;
                self.press(MouseButton::Left, true)?;
                self.pointer(*to)?;
                self.press(MouseButton::Left, false)?;
            }
        }
        Ok(())
    }

    /// Runs a batch, stopping at the first failure (spec §6.2). The actions
    /// that ran before it stay done.
    ///
    /// # Errors
    ///
    /// The first [`InputError`], with the actions that succeeded.
    pub fn run_batch(&mut self, actions: &[InputAction]) -> Result<usize, BatchError> {
        for (index, action) in actions.iter().enumerate() {
            match self.run(action) {
                Ok(()) => {}
                Err(error) => return Err(BatchError { done: index, error }),
            }
        }
        Ok(actions.len())
    }

    fn pointer(&mut self, at: Norm) -> Result<(), InputError> {
        let pixel = pixel(at, self.screen);
        self.backend
            .move_to(pixel)
            .map_err(|reason| InputError::Backend { reason })
    }

    fn press(&mut self, button: MouseButton, down: bool) -> Result<(), InputError> {
        self.backend
            .button(button, down)
            .map_err(|reason| InputError::Backend { reason })
    }
}

/// A batch that stopped early: `done` actions succeeded, then `error`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("batch stopped after {done} actions: {error}")]
pub struct BatchError {
    /// Actions that ran before the failure.
    pub done: usize,
    /// Why the next one failed.
    pub error: InputError,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records what the driver asked of it and can fail on demand.
    #[derive(Debug, Default)]
    struct Fake {
        moves: Vec<Pixel>,
        buttons: Vec<(MouseButton, bool)>,
        scrolls: Vec<(i32, i32)>,
        texts: Vec<String>,
        keys: Vec<KeyStroke>,
        /// Fail every backend call after this many succeed.
        fail_after: Option<usize>,
        calls: usize,
    }

    impl Fake {
        fn failing(after: usize) -> Self {
            Self {
                fail_after: Some(after),
                ..Self::default()
            }
        }

        fn refused(&mut self) -> Result<(), String> {
            self.calls += 1;
            match self.fail_after {
                Some(limit) if self.calls > limit => Err("no".to_owned()),
                _ => Ok(()),
            }
        }
    }

    impl InputBackend for Fake {
        fn move_to(&mut self, at: Pixel) -> Result<(), String> {
            self.refused()?;
            self.moves.push(at);
            Ok(())
        }

        fn button(&mut self, button: MouseButton, down: bool) -> Result<(), String> {
            self.refused()?;
            self.buttons.push((button, down));
            Ok(())
        }

        fn scroll(&mut self, dx: i32, dy: i32) -> Result<(), String> {
            self.refused()?;
            self.scrolls.push((dx, dy));
            Ok(())
        }

        fn text(&mut self, text: &str) -> Result<(), String> {
            self.refused()?;
            self.texts.push(text.to_owned());
            Ok(())
        }

        fn key(&mut self, stroke: &KeyStroke) -> Result<(), String> {
            self.refused()?;
            self.keys.push(stroke.clone());
            Ok(())
        }
    }

    fn screen() -> Size {
        Size {
            width: 1000,
            height: 500,
        }
    }

    fn at(x: u32, y: u32) -> Norm {
        Norm { x, y }
    }

    #[test]
    fn a_click_maps_normalized_coordinates_to_pixels() {
        let mut input = Input::new(Fake::default(), screen());
        input
            .run(&InputAction::Click {
                at: at(500, 500),
                button: MouseButton::Left,
            })
            .expect("click runs");
        // 0–999 of 1000×500: x 500 is pixel 500, y 500 of 999 is pixel 250.
        assert_eq!(input.backend.moves, vec![Pixel { x: 500, y: 250 }]);
        assert_eq!(
            input.backend.buttons,
            vec![(MouseButton::Left, true), (MouseButton::Left, false)]
        );
    }

    #[test]
    fn a_point_is_the_same_on_a_retina_screen() {
        let mut input = Input::new(
            Fake::default(),
            Size {
                width: 2000,
                height: 1000,
            },
        );
        input
            .run(&InputAction::Click {
                at: at(250, 250),
                button: MouseButton::Left,
            })
            .expect("click runs");
        // 250 of 999 on a 2× screen is the same physical point as 500 of
        // 999 on a 1× screen.
        assert_eq!(input.backend.moves, vec![Pixel { x: 500, y: 250 }]);
    }

    #[test]
    fn a_drag_presses_moves_and_releases() {
        let mut input = Input::new(Fake::default(), screen());
        input
            .run(&InputAction::Drag {
                from: at(0, 0),
                to: at(999, 999),
            })
            .expect("drag runs");
        assert_eq!(
            input.backend.moves,
            vec![Pixel { x: 0, y: 0 }, Pixel { x: 999, y: 499 }]
        );
        assert_eq!(
            input.backend.buttons,
            vec![(MouseButton::Left, true), (MouseButton::Left, false)]
        );
    }

    #[test]
    fn typing_text_goes_to_the_backend() {
        let mut input = Input::new(Fake::default(), screen());
        input
            .run(&InputAction::Type {
                text: "hello".to_owned(),
            })
            .expect("type runs");
        assert_eq!(input.backend.texts, vec!["hello".to_owned()]);
    }

    #[test]
    fn oversized_text_is_refused_before_the_backend_sees_it() {
        let mut input = Input::new(Fake::default(), screen());
        let error = input
            .run(&InputAction::Type {
                text: "x".repeat(TEXT_MAX_BYTES + 1),
            })
            .expect_err("oversized text is refused");
        assert_eq!(
            error,
            InputError::TextTooLarge {
                got: TEXT_MAX_BYTES + 1,
                max: TEXT_MAX_BYTES
            }
        );
        assert!(input.backend.texts.is_empty());
    }

    #[test]
    fn a_key_spec_parses_modifiers_and_names() {
        let stroke = parse_key("cmd+shift+z").expect("spec parses");
        assert!(stroke.meta && stroke.shift);
        assert_eq!(stroke.base, KeyBase::Char('z'));
        let enter = parse_key("enter").expect("enter parses");
        assert_eq!(enter.base, KeyBase::Named(NamedKey::Enter));
        assert!(!enter.meta);
        let f5 = parse_key("ctrl+f5").expect("f5 parses");
        assert_eq!(f5.base, KeyBase::Named(NamedKey::Function(5)));
        assert!(f5.control);
    }

    #[test]
    fn an_unknown_key_spec_is_refused() {
        for spec in ["", "cmd+", "nope", "f13", "a+b", "+\u{7}"] {
            assert!(parse_key(spec).is_err(), "`{spec}` must not parse as a key");
        }
    }

    #[test]
    fn an_oversized_key_spec_is_refused() {
        let spec = "a".repeat(KEY_SPEC_MAX_BYTES + 1);
        assert!(matches!(
            parse_key(&spec),
            Err(InputError::KeySpecTooLarge { .. })
        ));
    }

    #[test]
    fn a_key_action_presses_the_parsed_stroke() {
        let mut input = Input::new(Fake::default(), screen());
        input
            .run(&InputAction::Key {
                key: "cmd+c".to_owned(),
            })
            .expect("key runs");
        assert_eq!(input.backend.keys.len(), 1);
        assert!(input.backend.keys[0].meta);
        assert_eq!(input.backend.keys[0].base, KeyBase::Char('c'));
    }

    #[test]
    fn scroll_moves_the_pointer_first_and_caps_the_amount() {
        let mut input = Input::new(Fake::default(), screen());
        input
            .run(&InputAction::Scroll {
                at: at(100, 100),
                dx: 0,
                dy: -3,
            })
            .expect("scroll runs");
        assert_eq!(input.backend.moves, vec![Pixel { x: 100, y: 50 }]);
        assert_eq!(input.backend.scrolls, vec![(0, -3)]);

        let mut input = Input::new(Fake::default(), screen());
        assert!(matches!(
            input.run(&InputAction::Scroll {
                at: at(100, 100),
                dx: 0,
                dy: SCROLL_MAX_CLICKS + 1,
            }),
            Err(InputError::ScrollTooLarge { .. })
        ));
        assert!(input.backend.scrolls.is_empty());
    }

    #[test]
    fn a_batch_stops_at_the_first_failure_and_reports_the_count() {
        // Two backend calls succeed; every `Type` is one call, so the third
        // action is the first failure.
        let mut input = Input::new(Fake::failing(2), screen());
        let error = input
            .run_batch(&[
                InputAction::Type {
                    text: "ok".to_owned(),
                },
                InputAction::Type {
                    text: "also ok".to_owned(),
                },
                InputAction::Type {
                    text: "never".to_owned(),
                },
            ])
            .expect_err("the third action fails at the backend");
        assert_eq!(error.done, 2);
        assert_eq!(
            input.backend.texts,
            vec!["ok".to_owned(), "also ok".to_owned()]
        );
    }

    #[test]
    fn a_batch_of_valid_actions_runs_them_all() {
        let mut input = Input::new(Fake::default(), screen());
        let done = input
            .run_batch(&[
                InputAction::Click {
                    at: at(10, 10),
                    button: MouseButton::Left,
                },
                InputAction::Key {
                    key: "enter".to_owned(),
                },
            ])
            .expect("both run");
        assert_eq!(done, 2);
        assert_eq!(input.backend.moves.len(), 1);
        assert_eq!(input.backend.keys.len(), 1);
    }

    #[test]
    fn every_action_carries_the_input_subject() {
        let click = InputAction::Click {
            at: at(0, 0),
            button: MouseButton::Left,
        };
        let call = click.policy_call();
        assert_eq!(call.tool, "input");
        assert_eq!(call.subject, "click");
        assert_eq!(call.risk, Risk::Exec);
        let typed = InputAction::Type {
            text: "x".to_owned(),
        };
        let call = typed.policy_call();
        assert_eq!(call.subject, "type");
        assert_eq!(call.risk, Risk::Write);
    }

    #[test]
    fn the_input_subject_asks_by_default() {
        // The host-side default from aulo-policy T5.2: Default mode,
        // OnRequest, workspace-write, no rules.
        let engine = perm_rules::Engine::compile(
            &perm_rules::RuleSet::default(),
            perm_rules::Grammar::default(),
            None,
            std::path::Path::new("."),
        )
        .expect("empty rules compile");
        let outcome = engine.decide(
            &InputAction::Click {
                at: at(0, 0),
                button: MouseButton::Left,
            }
            .policy_call(),
            perm_rules::PermissionMode::Default,
            perm_rules::ApprovalPolicy::OnRequest,
            perm_rules::SandboxMode::WorkspaceWrite,
            &[],
        );
        assert!(matches!(outcome, perm_rules::Outcome::Ask(_)));
    }
}
