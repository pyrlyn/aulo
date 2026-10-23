//! Tools the agent calls (spec §4.2, §6.1): shell, PTY, files, clipboard.
//!
//! [`shell`] runs a command inside the proc-sandbox confinement with a
//! timeout, an output cap and a lossless archive of the full output, so the
//! model sees a truncated view with an id it can expand (the cox `expand`
//! pattern).

pub mod shell;
