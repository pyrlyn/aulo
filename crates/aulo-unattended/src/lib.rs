//! Unattended mode (spec §7, T5.7).
//!
//! A routine, and any run with no client attached, may read. Writes, exec
//! and synthetic input are denied. Queueing them for a later approval is
//! T5.3; until that exists the action does not run.

use std::fs;
use std::path::Path;

/// Whether a person is at a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// A client is attached.
    Attached,
    /// Nobody is attached.
    Detached,
}

/// Why the run exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    /// A person started it.
    Interactive,
    /// A schedule or other unattended run.
    Routine,
}

/// The kind of tool a run wants to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Read a file or other state.
    Read,
    /// Create or change a file.
    Write,
    /// Run a program.
    Exec,
    /// Click, type, or other synthetic input.
    Input,
}

/// What unattended mode adds. Other policy can still deny an allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Unattended mode does not object.
    Allow,
    /// The tool must not run.
    Deny,
}

/// Why a file helper stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AccessError {
    /// A routine or a detached run asked to change something.
    #[error("unattended mode denies this action")]
    Denied,
    /// The bytes are over [`MAX_FILE_BYTES`]. Nothing was written.
    #[error("file is over the size cap")]
    TooLarge,
    /// The operating system refused. The path and the bytes are not included.
    #[error("file operation failed")]
    Io,
}

/// Largest file this helper will read or write.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// `Deny` when nobody is attached or the run is a routine and the tool is
/// not a read. Reads stay allowed.
#[must_use]
pub fn gate(presence: Presence, run: RunKind, tool: ToolKind) -> Gate {
    let unattended = presence == Presence::Detached || run == RunKind::Routine;
    if unattended && tool != ToolKind::Read {
        Gate::Deny
    } else {
        Gate::Allow
    }
}

/// Writes `bytes` to `path` when the run is allowed to. A denial leaves the
/// path untouched.
///
/// # Errors
///
/// [`AccessError::Denied`] in a routine or while detached.
/// [`AccessError::TooLarge`] when `bytes` exceeds [`MAX_FILE_BYTES`].
/// [`AccessError::Io`] when the write fails. The error does not carry the path.
pub fn write_file(
    presence: Presence,
    run: RunKind,
    path: &Path,
    bytes: &[u8],
) -> Result<(), AccessError> {
    if gate(presence, run, ToolKind::Write) == Gate::Deny {
        return Err(AccessError::Denied);
    }
    let len = u64::try_from(bytes.len()).map_err(|_| AccessError::TooLarge)?;
    if len > MAX_FILE_BYTES {
        return Err(AccessError::TooLarge);
    }
    fs::write(path, bytes).map_err(|_| AccessError::Io)
}

/// Reads `path` when the run may read. The bytes are capped.
///
/// # Errors
///
/// [`AccessError::TooLarge`] when the file is over [`MAX_FILE_BYTES`].
/// [`AccessError::Io`] when the read fails.
pub fn read_file(presence: Presence, run: RunKind, path: &Path) -> Result<Vec<u8>, AccessError> {
    if gate(presence, run, ToolKind::Read) == Gate::Deny {
        return Err(AccessError::Denied);
    }
    let meta = fs::metadata(path).map_err(|_| AccessError::Io)?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(AccessError::TooLarge);
    }
    fs::read(path).map_err(|_| AccessError::Io)
}
