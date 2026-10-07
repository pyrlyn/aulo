//! The API token: generated once, kept in the OS keychain, never printed.

use std::fmt;
use std::fmt::Write as _;
use std::sync::{Mutex, PoisonError};

/// Random bytes in a token: 256 bits, beyond any online guessing.
const TOKEN_BYTES: usize = 32;
/// A fixed prefix lets the log redaction recognise the token under any field
/// name, as it does `ghp_` or `sk-` keys; aulo-telemetry masks this shape.
const PREFIX: &str = "aulo_";
/// Hex digits per byte.
const HEX_PER_BYTE: usize = 2;

/// A bearer token for the TCP listener. Its `Debug` is redacted and it has no
/// `Display`, so formatting it by accident cannot leak it. No `PartialEq`
/// either: tokens are checked in constant time by the server, never with `==`.
#[derive(Clone)]
pub struct ApiToken(String);

impl fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiToken([REDACTED])")
    }
}

impl ApiToken {
    /// A fresh token from the OS random source.
    pub fn generate() -> Result<Self, TokenError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|e| TokenError::Random(e.to_string()))?;
        let mut text = String::with_capacity(PREFIX.len() + TOKEN_BYTES * HEX_PER_BYTE);
        text.push_str(PREFIX);
        for byte in bytes {
            // Writing to a String cannot fail.
            let _ = write!(text, "{byte:02x}");
        }
        Ok(Self(text))
    }

    /// Accepts only the shape [`generate`](Self::generate) produces, so a
    /// truncated or hand-edited keychain entry is never used as a weak token.
    pub fn parse(text: &str) -> Result<Self, TokenError> {
        let digits = text.strip_prefix(PREFIX).ok_or(TokenError::Malformed)?;
        let well_formed = digits.len() == TOKEN_BYTES * HEX_PER_BYTE
            && digits
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if well_formed {
            Ok(Self(text.to_owned()))
        } else {
            Err(TokenError::Malformed)
        }
    }

    /// The token text, for the owner's tooling and for clients to send.
    /// Never pass it to a log macro.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

/// Why the token could not be produced. Messages never contain the token.
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("the OS random source failed: {0}")]
    Random(String),
    #[error("the OS keychain failed: {0}")]
    Keychain(String),
    #[error("the stored API token is malformed; delete it so aulod generates a new one")]
    Malformed,
}

/// Where the API token lives between runs.
///
/// Implementations may block (a keychain can wait on the user), so call them
/// before the runtime starts or from `spawn_blocking`.
pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<ApiToken>, TokenError>;
    fn save(&self, token: &ApiToken) -> Result<(), TokenError>;

    /// The stored token, or a new one saved on first run. A malformed stored
    /// token is an error, not a reason to replace it silently.
    fn load_or_create(&self) -> Result<ApiToken, TokenError> {
        if let Some(token) = self.load()? {
            return Ok(token);
        }
        let token = ApiToken::generate()?;
        self.save(&token)?;
        tracing::info!("generated a new API token and stored it");
        Ok(token)
    }
}

/// The OS keychain: Keychain on macOS, Credential Manager on Windows, the
/// Secret Service on Linux.
#[derive(Debug, Clone)]
pub struct KeychainTokenStore {
    service: String,
    account: String,
}

impl KeychainTokenStore {
    /// Each `AULO_HOME` should use its own `account`, so a test daemon never
    /// reads or replaces the real daemon's token.
    pub fn new(service: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, TokenError> {
        keyring::Entry::new(&self.service, &self.account).map_err(keychain)
    }
}

// Only the message is kept: some keyring errors carry the stored bytes in
// their `Debug` output, and those bytes are the token.
pub(super) fn keychain(error: keyring::Error) -> TokenError {
    TokenError::Keychain(error.to_string())
}

impl TokenStore for KeychainTokenStore {
    fn load(&self) -> Result<Option<ApiToken>, TokenError> {
        match self.entry()?.get_password() {
            Ok(text) => ApiToken::parse(&text).map(Some),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain(e)),
        }
    }

    fn save(&self, token: &ApiToken) -> Result<(), TokenError> {
        self.entry()?.set_password(token.reveal()).map_err(keychain)
    }
}

/// A store in memory, for tests and for callers that pass a token in.
#[derive(Debug, Default)]
pub struct MemoryTokenStore(Mutex<Option<ApiToken>>);

impl MemoryTokenStore {
    pub fn with_token(token: ApiToken) -> Self {
        Self(Mutex::new(Some(token)))
    }
}

impl TokenStore for MemoryTokenStore {
    // A poisoned lock still holds a whole token: the critical sections only
    // clone or replace it.
    fn load(&self) -> Result<Option<ApiToken>, TokenError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn save(&self, token: &ApiToken) -> Result<(), TokenError> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(token.clone());
        Ok(())
    }
}
