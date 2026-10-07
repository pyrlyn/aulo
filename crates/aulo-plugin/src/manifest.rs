//! `aulo-plugin.toml`: reading, parsing and validating the manifest of a plugin
//! package, and exporting its JSON Schema. This is the one place that touches
//! the manifest's TOML.

mod limits;
mod model;
mod validate;

use std::io::Read;
use std::path::{Path, PathBuf};

use schemars::Schema;
use schemars::generate::SchemaSettings;

pub use limits::MAX_MANIFEST_BYTES;
pub use model::{Audio, Capabilities, Entry, Kind, PluginManifest, Process};

/// File name inside a plugin directory.
pub const MANIFEST_FILE: &str = "aulo-plugin.toml";

/// A manifest that was refused. Every variant names the file so the host can
/// warn about it and skip that one plugin.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("cannot read plugin manifest {}: {source}", file.display())]
    Read {
        file: PathBuf,
        source: std::io::Error,
    },
    #[error("plugin manifest {} is larger than {limit} bytes", file.display())]
    TooLarge { file: PathBuf, limit: u64 },
    #[error("invalid plugin manifest {}: {source}", file.display())]
    Parse {
        file: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid plugin manifest {}: {message}", file.display())]
    Invalid { file: PathBuf, message: String },
}

impl PluginManifest {
    /// Reads and validates the manifest at `file`.
    pub fn load(file: &Path) -> Result<Self, ManifestError> {
        let read_err = |source| ManifestError::Read {
            file: file.to_owned(),
            source,
        };
        let mut text = String::new();
        // Read one byte past the cap so an oversized or growing file is caught
        // without trusting a length checked before the read.
        std::fs::File::open(file)
            .and_then(|f| f.take(MAX_MANIFEST_BYTES + 1).read_to_string(&mut text))
            .map_err(read_err)?;
        Self::from_toml(&text, file)
    }

    /// Parses and validates `text`; `file` only labels errors.
    pub fn from_toml(text: &str, file: &Path) -> Result<Self, ManifestError> {
        if text.len() as u64 > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge {
                file: file.to_owned(),
                limit: MAX_MANIFEST_BYTES,
            });
        }
        let manifest: Self = toml::from_str(text).map_err(|source| ManifestError::Parse {
            file: file.to_owned(),
            source,
        })?;
        validate::check(&manifest).map_err(|message| ManifestError::Invalid {
            file: file.to_owned(),
            message,
        })?;
        Ok(manifest)
    }
}

/// The JSON Schema of [`PluginManifest`], pretty-printed with a trailing newline.
///
/// No `Option` reaches the schema (the one optional field is declared as its
/// inner type), so unlike `aulo-config` no null-stripping transform is needed.
pub fn schema_json() -> Result<String, serde_json::Error> {
    let schema: Schema = SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<PluginManifest>();
    let mut json = serde_json::to_string_pretty(&schema)?;
    json.push('\n');
    Ok(json)
}
