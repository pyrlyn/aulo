use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use figment::providers::{Env, Format, Serialized, Toml};
use figment::value::Value;
use figment::{Figment, Metadata, Profile, Provider, Source};

use crate::error::ConfigError;
use crate::model::Config;

const DEFAULTS: &str = "built-in defaults";
const OVERRIDES: &str = "command-line overrides";

/// Where a value came from, lowest layer first: defaults, global file,
/// project file, environment, caller overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Default,
    File(PathBuf),
    /// Name of the environment variable, e.g. `AULO_VOICE__LISTEN`.
    Env(String),
    Override,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str(DEFAULTS),
            Self::File(path) => write!(f, "{}", path.display()),
            Self::Env(name) => f.write_str(name),
            Self::Override => f.write_str(OVERRIDES),
        }
    }
}

/// Origin of every leaf value, keyed by dotted path (`voice.stt.engine`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provenance(BTreeMap<String, Origin>);

impl Provenance {
    pub fn get(&self, key: &str) -> Option<&Origin> {
        self.0.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Origin)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub config: Config,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// Directory holding `config.toml`. Unset: `$AULO_HOME`, then `~/.aulo`.
    pub home: Option<PathBuf>,
    /// Directory holding the project's `.aulo.toml`.
    pub project_dir: Option<PathBuf>,
    /// Highest layer, for command-line flags: a nested JSON object.
    pub overrides: Option<serde_json::Value>,
}

/// `$AULO_HOME`, else `~/.aulo`; `None` when neither can be determined.
pub fn aulo_home() -> Option<PathBuf> {
    std::env::var_os("AULO_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|h| h.join(".aulo")))
}

impl Config {
    /// Merges defaults < `config.toml` < `.aulo.toml` < `AULO_*` env < overrides.
    /// Missing files are skipped; a present but invalid layer is an error.
    pub fn load(options: &LoadOptions) -> Result<Loaded, ConfigError> {
        let mut figment = Figment::from(Named(Serialized::defaults(Config::default()), DEFAULTS));
        if let Some(home) = options.home.clone().or_else(aulo_home) {
            figment = figment.merge(Toml::file(home.join("config.toml")));
        }
        if let Some(dir) = &options.project_dir {
            figment = figment.merge(Toml::file(project_file(dir)));
        }
        // Nested keys use `__` (`AULO_VOICE__STT__ENGINE`) because field names
        // contain `_`. Requiring a nesting level (the filter runs after `split`) also keeps `AULO_HOME` and other
        // non-config variables out of the strict, unknown-key-rejecting parse.
        figment = figment.merge(
            Env::prefixed("AULO_")
                .split("__")
                .filter(|k| k.as_str().contains('.')),
        );
        if let Some(overrides) = &options.overrides {
            figment = figment.merge(Named(Serialized::defaults(overrides), OVERRIDES));
        }

        // Before figment: an unknown-field error can echo the value, and a
        // secret must not appear in the message. The file name still does.
        reject_inline_layers(options)?;

        let config = figment.extract().map_err(to_config_error)?;
        let provenance = provenance(&figment)?;
        Ok(Loaded { config, provenance })
    }
}

fn project_file(dir: &Path) -> PathBuf {
    dir.join(".aulo.toml")
}

/// Refuses a secret-named key in a config file, in `AULO_*` configuration
/// variables, or in caller overrides. Credential variables (`OPENAI_API_KEY`,
/// `AULO_<ID>_API_KEY`) have no `__` and are not configuration.
fn reject_inline_layers(options: &LoadOptions) -> Result<(), ConfigError> {
    if let Some(home) = options.home.clone().or_else(aulo_home) {
        reject_inline_file(&home.join("config.toml"))?;
    }
    if let Some(dir) = &options.project_dir {
        reject_inline_file(&project_file(dir))?;
    }
    reject_inline_env()?;
    if let Some(overrides) = &options.overrides {
        reject_inline_json(overrides, "")?;
    }
    Ok(())
}

fn reject_inline_file(path: &Path) -> Result<(), ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(ConfigError {
                origin: Some(Origin::File(path.to_owned())),
                message: format!("cannot read: {error}"),
            });
        }
    };
    match keychain_secret::reject_inline_secrets(&text, &path.display().to_string()) {
        Ok(()) => Ok(()),
        // Figment reports a syntax error and already names the file.
        Err(keychain_secret::Error::InvalidToml { .. }) => Ok(()),
        Err(keychain_secret::Error::Inline { key, .. }) => {
            Err(inline_rejected(key, Origin::File(path.to_owned())))
        }
        Err(error) => Err(ConfigError {
            origin: Some(Origin::File(path.to_owned())),
            message: error.to_string(),
        }),
    }
}

fn reject_inline_env() -> Result<(), ConfigError> {
    for (name, _) in std::env::vars() {
        let Some(rest) = name.strip_prefix("AULO_") else {
            continue;
        };
        // A dedicated credential variable is one segment (`AULO_OPENAI_API_KEY`).
        // Configuration uses `__` between levels.
        if !rest.contains("__") {
            continue;
        }
        let last = rest.rsplit("__").next().unwrap_or(rest);
        if keychain_secret::is_secret_key(last) {
            return Err(inline_rejected(last.to_owned(), Origin::Env(name)));
        }
    }
    Ok(())
}

fn reject_inline_json(value: &serde_json::Value, path: &str) -> Result<(), ConfigError> {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if keychain_secret::is_secret_key(key) {
                    return Err(inline_rejected(child_path, Origin::Override));
                }
                reject_inline_json(child, &child_path)?;
            }
            Ok(())
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                reject_inline_json(child, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn inline_rejected(key: String, origin: Origin) -> ConfigError {
    ConfigError {
        origin: Some(origin),
        message: format!(
            "the secret at `{key}` is rejected; set it with `aulo auth set` or an environment variable, not in configuration"
        ),
    }
}

/// Gives a provider a stable name so its layer can be told apart from env.
struct Named<P>(P, &'static str);

impl<P: Provider> Provider for Named<P> {
    fn metadata(&self) -> Metadata {
        Metadata::named(self.1)
    }

    fn data(&self) -> Result<figment::value::Map<Profile, figment::value::Dict>, figment::Error> {
        self.0.data()
    }
}

fn origin_of(metadata: &Metadata, keys: &[&str]) -> Origin {
    match &metadata.source {
        Some(Source::File(path)) => Origin::File(path.clone()),
        _ if metadata.name == DEFAULTS => Origin::Default,
        _ if metadata.name == OVERRIDES => Origin::Override,
        // Rebuilt from the key path because figment's own name for the
        // variable drops the prefix and the `__` separators.
        _ => Origin::Env(format!("AULO_{}", keys.join("__").to_uppercase())),
    }
}

fn provenance(figment: &Figment) -> Result<Provenance, ConfigError> {
    fn walk(
        figment: &Figment,
        value: &Value,
        path: &mut Vec<String>,
        out: &mut BTreeMap<String, Origin>,
    ) {
        if let Value::Dict(_, dict) = value {
            for (key, child) in dict {
                path.push(key.clone());
                walk(figment, child, path, out);
                path.pop();
            }
        } else if let Some(metadata) = figment.get_metadata(value.tag()) {
            let keys: Vec<&str> = path.iter().map(String::as_str).collect();
            out.insert(path.join("."), origin_of(metadata, &keys));
        }
    }

    let root: Value = figment.extract().map_err(to_config_error)?;
    let mut out = BTreeMap::new();
    walk(figment, &root, &mut Vec::new(), &mut out);
    Ok(Provenance(out))
}

fn to_config_error(error: figment::Error) -> ConfigError {
    let keys: Vec<&str> = error.path.iter().map(String::as_str).collect();
    let origin = error.metadata.as_ref().map(|m| origin_of(m, &keys));
    let message = if error.path.is_empty() {
        error.kind.to_string()
    } else {
        format!("{} for key `{}`", error.kind, error.path.join("."))
    };
    ConfigError { origin, message }
}
