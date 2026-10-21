//! Test support for the agent session (T4.9).
//!
//! [`Scripted`] replays an `llm-wire` `test-util` scenario. [`TestDaemon`]
//! serves the API on a temporary home. [`ConverseSession`] drives
//! `SessionService.Converse`. T4.6 and T4.8 tests are the callers this
//! crate exists for; until those services land, the crate's own tests
//! mount a stand-in session that answers from [`Scripted`].

mod converse;
#[cfg(unix)]
mod daemon;
mod error;
mod scripted;

pub use converse::{ConverseSession, attach, drive, interrupt, switch_model, user_turn};
#[cfg(unix)]
pub use daemon::{STORE_FILE, TestDaemon, TestDaemonBuilder};
pub use error::TestkitError;
pub use llm_wire::test_util::scripted::{TurnSpec, parse_scenario};
pub use scripted::Scripted;

/// Platforms without the owner-only Unix socket have no [`TestDaemon`] yet.
#[cfg(not(unix))]
pub use unsupported::TestDaemon;

#[cfg(not(unix))]
mod unsupported {
    use crate::TestkitError;

    /// Placeholder so the crate still type-checks. [`TestDaemon::start`]
    /// returns [`TestkitError::LocalSocket`].
    #[derive(Debug)]
    pub struct TestDaemon;

    impl TestDaemon {
        /// Always returns [`TestkitError::LocalSocket`].
        pub async fn start() -> Result<Self, TestkitError> {
            Err(TestkitError::LocalSocket)
        }
    }
}
