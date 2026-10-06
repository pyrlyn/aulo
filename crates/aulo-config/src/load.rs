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

        let config = figment.extract().map_err(to_config_error)?;
        let provenance = provenance(&figment)?;
        Ok(Loaded { config, provenance })
    }
}

fn project_file(dir: &Path) -> PathBuf {
    dir.join(".aulo.toml")
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
