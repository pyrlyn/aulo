//! Provider API keys. The environment wins, then the OS keychain. A config
//! file is never a place for one: [`crate::Config::load`] rejects secret-named
//! keys and names the file.
//!
//! The value is for the provider stack. [`key_state`] reports only whether a
//! key is available and where, so a list or a gRPC response can be built
//! without the secret.

use std::fmt;

use keychain_secret::{Resolved, Secret, Source, os_store, resolve_with};

use crate::model::{Config, ProviderKind};

/// Keychain service name for provider keys. The daemon's API token uses a
/// different service, so the two never share an entry.
pub const KEYCHAIN_SERVICE: &str = "aulo";

/// `off`, `0`, `false` or `no` makes [`system_store`] a store that finds
/// nothing and refuses writes, so a development run never prompts.
pub const KEYCHAIN_SWITCH: &str = "AULO_KEYCHAIN";

/// Largest key accepted, matching `SetProviderKeyRequest.key`.
pub const MAX_KEY_BYTES: usize = 4096;

pub use keychain_secret::{MemoryStore, SecretStore};

/// Why a provider key could not be stored or looked up. Messages never
/// contain the key.
#[derive(Debug, thiserror::Error)]
pub enum CredentialsError {
    #[error(
        "provider id must be 1 to 64 bytes of ASCII letters, digits, '.', '_' or '-', and must not start with '.' or '-'"
    )]
    BadId,
    #[error("no key was provided")]
    Empty,
    #[error("a provider key must be at most {MAX_KEY_BYTES} bytes")]
    TooLong,
    #[error(transparent)]
    Store(#[from] keychain_secret::Error),
}

impl CredentialsError {
    /// The keychain switch is off, so nothing can be stored.
    #[must_use]
    pub fn is_keychain_disabled(&self) -> bool {
        matches!(self, Self::Store(keychain_secret::Error::Disabled))
    }
}

/// Whether a provider key is available, without the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyState {
    /// Neither an environment variable nor the keychain has a key.
    Unset,
    /// `variable` is set and not blank. It wins over the keychain.
    Env { variable: String },
    /// The keychain entry is set and no environment variable overrides it.
    Keychain,
}

impl KeyState {
    #[must_use]
    pub fn is_set(&self) -> bool {
        !matches!(self, Self::Unset)
    }
}

impl fmt::Display for KeyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unset => f.write_str("unset"),
            Self::Env { variable } => write!(f, "set\tenvironment\t{variable}"),
            Self::Keychain => f.write_str("set\tkeychain"),
        }
    }
}

/// One configured endpoint and whether a key is available for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderKeyInfo {
    pub id: String,
    pub kind: ProviderKind,
    pub state: KeyState,
}

/// The OS keychain, or a store that refuses writes when [`KEYCHAIN_SWITCH`] is off.
#[must_use]
pub fn system_store() -> Box<dyn SecretStore> {
    os_store(KEYCHAIN_SERVICE, KEYCHAIN_SWITCH)
}

/// Accepts a provider id. The rules keep it usable as a keychain account and
/// as one path segment of an environment variable name.
pub fn validate_provider_id(id: &str) -> Result<&str, CredentialsError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && !id.starts_with(['.', '-'])
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(id)
    } else {
        Err(CredentialsError::BadId)
    }
}

/// `AULO_<ID>_API_KEY`, with every character that is not an ASCII letter or
/// digit turned into `_`. `my.proxy` and `my_proxy` therefore share a variable.
#[must_use]
pub fn provider_env_var(id: &str) -> String {
    let mut out = String::from("AULO_");
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(char::from(byte.to_ascii_uppercase()));
        } else {
            out.push('_');
        }
    }
    out.push_str("_API_KEY");
    out
}

fn account(id: &str) -> String {
    format!("provider:{id}")
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

/// Where a key would come from. `kind` adds that provider's conventional
/// variable (`OPENAI_API_KEY`) after the per-endpoint `AULO_<ID>_API_KEY`.
pub fn key_state(
    id: &str,
    kind: Option<ProviderKind>,
    store: &dyn SecretStore,
) -> Result<KeyState, CredentialsError> {
    key_state_with(id, kind, store, |name| std::env::var(name).ok())
}

/// [`key_state`] with the environment injected, so tests never read the process.
pub fn key_state_with(
    id: &str,
    kind: Option<ProviderKind>,
    store: &dyn SecretStore,
    getenv: impl Fn(&str) -> Option<String>,
) -> Result<KeyState, CredentialsError> {
    let id = validate_provider_id(id)?;
    let endpoint = provider_env_var(id);
    if nonempty(getenv(&endpoint)).is_some() {
        return Ok(KeyState::Env { variable: endpoint });
    }
    if let Some(variable) = kind.and_then(ProviderKind::api_key_env)
        && nonempty(getenv(variable)).is_some()
    {
        return Ok(KeyState::Env {
            variable: variable.to_owned(),
        });
    }
    let found = store.get(&account(id))?;
    Ok(match found {
        Some(secret) if !secret.reveal().trim().is_empty() => KeyState::Keychain,
        _ => KeyState::Unset,
    })
}

/// The key itself, for a provider call. Callers must not log, return or write
/// [`Resolved::secret`]. Same precedence as [`key_state`].
pub fn resolve_provider_key(
    id: &str,
    kind: Option<ProviderKind>,
    store: &dyn SecretStore,
) -> Result<Option<Resolved>, CredentialsError> {
    resolve_provider_key_with(id, kind, store, |name| std::env::var(name).ok())
}

/// [`resolve_provider_key`] with the environment injected.
pub fn resolve_provider_key_with(
    id: &str,
    kind: Option<ProviderKind>,
    store: &dyn SecretStore,
    getenv: impl Fn(&str) -> Option<String>,
) -> Result<Option<Resolved>, CredentialsError> {
    let id = validate_provider_id(id)?;
    let endpoint = provider_env_var(id);
    if let Some(value) = nonempty(getenv(&endpoint)) {
        return Ok(Some(Resolved {
            secret: Secret::from(value),
            source: Source::Env,
        }));
    }
    if let Some(variable) = kind.and_then(ProviderKind::api_key_env)
        && let Some(value) = nonempty(getenv(variable))
    {
        return Ok(Some(Resolved {
            secret: Secret::from(value),
            source: Source::Env,
        }));
    }
    // `resolve_with` checks one variable, then the store. The variable is one
    // we already know is blank, so this is the keychain lookup.
    let found = resolve_with(&endpoint, &account(id), store, |_| None)?;
    Ok(found.filter(|resolved| !resolved.secret.reveal().trim().is_empty()))
}

/// Stores `key` in the keychain under `id`. Surrounding whitespace is removed.
/// The key is not an argument of a log line.
pub fn store_key(id: &str, key: &str, store: &dyn SecretStore) -> Result<(), CredentialsError> {
    let id = validate_provider_id(id)?;
    let key = key.trim();
    if key.is_empty() {
        return Err(CredentialsError::Empty);
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(CredentialsError::TooLong);
    }
    store.set(&account(id), &Secret::from(key))?;
    Ok(())
}

/// Removes the keychain entry. An environment variable, if set, still wins.
pub fn delete_key(id: &str, store: &dyn SecretStore) -> Result<(), CredentialsError> {
    let id = validate_provider_id(id)?;
    store.delete(&account(id))?;
    Ok(())
}

/// Every configured endpoint, in id order, with its key state and not its key.
pub fn configured_keys(
    config: &Config,
    store: &dyn SecretStore,
) -> Result<Vec<ProviderKeyInfo>, CredentialsError> {
    configured_keys_with(config, store, |name| std::env::var(name).ok())
}

/// [`configured_keys`] with the environment injected.
pub fn configured_keys_with(
    config: &Config,
    store: &dyn SecretStore,
    getenv: impl Fn(&str) -> Option<String>,
) -> Result<Vec<ProviderKeyInfo>, CredentialsError> {
    let mut out = Vec::with_capacity(config.providers.endpoints.len());
    for (id, endpoint) in &config.providers.endpoints {
        out.push(ProviderKeyInfo {
            id: id.clone(),
            kind: endpoint.kind,
            state: key_state_with(id, Some(endpoint.kind), store, &getenv)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProviderConfig, ProviderKind};

    const SECRET: &str = "sk-live-from-env";
    const STORED: &str = "sk-live-from-keychain";

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn config_with(id: &str, kind: ProviderKind) -> Config {
        let mut config = Config::default();
        config.providers.endpoints.insert(
            id.to_owned(),
            ProviderConfig {
                kind,
                base_url: None,
            },
        );
        config
    }

    #[test]
    fn endpoint_env_wins_over_the_conventional_var_and_the_keychain() {
        let store = MemoryStore::default();
        store_key("openai", STORED, &store).unwrap();
        let getenv = env(&[
            ("AULO_OPENAI_API_KEY", SECRET),
            ("OPENAI_API_KEY", "sk-live-conventional"),
        ]);
        assert_eq!(
            key_state_with("openai", Some(ProviderKind::Openai), &store, &getenv).unwrap(),
            KeyState::Env {
                variable: "AULO_OPENAI_API_KEY".into()
            }
        );
        let resolved =
            resolve_provider_key_with("openai", Some(ProviderKind::Openai), &store, &getenv)
                .unwrap()
                .unwrap();
        assert_eq!(resolved.secret.reveal(), SECRET);
        assert_eq!(resolved.source, keychain_secret::Source::Env);
        let shown = format!(
            "{:?}",
            key_state_with("openai", Some(ProviderKind::Openai), &store, &getenv).unwrap()
        );
        assert!(!shown.contains(SECRET), "{shown}");
        assert!(!shown.contains(STORED), "{shown}");
    }

    #[test]
    fn a_blank_endpoint_var_falls_through_to_the_conventional_one() {
        let store = MemoryStore::default();
        let getenv = env(&[("AULO_OPENAI_API_KEY", "  "), ("OPENAI_API_KEY", SECRET)]);
        let resolved =
            resolve_provider_key_with("openai", Some(ProviderKind::Openai), &store, getenv)
                .unwrap()
                .unwrap();
        assert_eq!(resolved.secret.reveal(), SECRET);
    }

    #[test]
    fn the_keychain_is_used_when_no_variable_is_set() {
        let store = MemoryStore::default();
        let padded = format!("  {STORED}  ");
        store_key("openai", &padded, &store).unwrap();
        let state = key_state_with("openai", Some(ProviderKind::Openai), &store, env(&[])).unwrap();
        assert_eq!(state, KeyState::Keychain);
        let resolved =
            resolve_provider_key_with("openai", Some(ProviderKind::Openai), &store, env(&[]))
                .unwrap()
                .unwrap();
        assert_eq!(resolved.secret.reveal(), STORED);
        delete_key("openai", &store).unwrap();
        assert_eq!(
            key_state_with("openai", None, &store, env(&[])).unwrap(),
            KeyState::Unset
        );
    }

    #[test]
    fn deleting_the_keychain_entry_leaves_an_environment_variable_in_force() {
        let store = MemoryStore::default();
        store_key("openai", STORED, &store).unwrap();
        delete_key("openai", &store).unwrap();
        let getenv = env(&[("OPENAI_API_KEY", SECRET)]);
        assert_eq!(
            key_state_with("openai", Some(ProviderKind::Openai), &store, &getenv).unwrap(),
            KeyState::Env {
                variable: "OPENAI_API_KEY".into()
            }
        );
    }

    #[test]
    fn local_providers_have_no_conventional_variable() {
        let store = MemoryStore::default();
        let getenv = env(&[("OPENAI_API_KEY", SECRET)]);
        assert_eq!(
            key_state_with("local", Some(ProviderKind::Ollama), &store, getenv).unwrap(),
            KeyState::Unset
        );
    }

    #[test]
    fn configured_keys_omit_the_value() {
        let store = MemoryStore::default();
        store_key("local", STORED, &store).unwrap();
        let mut config = config_with("local", ProviderKind::Ollama);
        config.providers.endpoints.insert(
            "openai".into(),
            ProviderConfig {
                kind: ProviderKind::Openai,
                base_url: None,
            },
        );
        let getenv = env(&[("OPENAI_API_KEY", SECRET)]);
        let listed = configured_keys_with(&config, &store, getenv).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, "local");
        assert_eq!(listed[0].state, KeyState::Keychain);
        assert_eq!(
            listed[1].state,
            KeyState::Env {
                variable: "OPENAI_API_KEY".into()
            }
        );
        let shown = format!("{listed:?}");
        assert!(!shown.contains(SECRET), "{shown}");
        assert!(!shown.contains(STORED), "{shown}");
    }

    #[test]
    fn rejected_keys_and_ids_do_not_echo_the_value() {
        let store = MemoryStore::default();
        let err = store_key("openai", "   ", &store).unwrap_err();
        assert!(err.to_string().contains("no key"), "{err}");
        let err = store_key("bad id", STORED, &store).unwrap_err();
        assert!(!err.to_string().contains(STORED), "{err}");
        let err = store_key("openai", &"k".repeat(MAX_KEY_BYTES + 1), &store).unwrap_err();
        assert!(err.to_string().contains("4096"), "{err}");
        assert!(!err.to_string().contains(&"k".repeat(80)), "{err}");
    }

    #[test]
    fn provider_env_var_maps_punctuation_to_underscores() {
        assert_eq!(provider_env_var("my.proxy"), "AULO_MY_PROXY_API_KEY");
        assert_eq!(provider_env_var("My.Proxy"), "AULO_MY_PROXY_API_KEY");
    }
}
