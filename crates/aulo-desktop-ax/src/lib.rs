//! Desktop control adapters (spec §4.2, §6).
//!
//! - [`input`] — synthetic mouse and keyboard events in normalized 0–999
//!   coordinates (T9.16), the last rung of the control ladder.
//! - `ax` — the desktop accessibility tree (T9.11).
//!
//! Both are untrusted-input surfaces: the model names coordinates, key specs
//! and text, so every value is capped and validated here, and each action
//! carries the policy subject the host must judge before it runs.

pub mod input;
