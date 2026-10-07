use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::ModelError;

/// The committed catalog, pinned to Hugging Face commits.
const EMBEDDED: &str = include_str!("../data/models.json");

/// Suffix of in-progress downloads; a catalog path may not use it, or a
/// finished file could be mistaken for a partial one.
pub(crate) const PART_SUFFIX: &str = ".part";

/// Which speech role a model fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    Stt,
    Tts,
    Vad,
    Kws,
}

/// One downloadable file. The size is also the download cap.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// Path relative to the model directory and to `base_url`; no `..`.
    pub path: String,
    /// Exact size in bytes.
    #[schemars(range(min = 1))]
    pub size: u64,
    /// Lowercase hex SHA-256 of the file.
    #[schemars(regex(pattern = r"^[0-9a-f]{64}$"))]
    pub sha256: String,
}

/// A model: a directory of files that are installed together.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Directory name under `<AULO_HOME>/models/`.
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9._-]{0,63}$"))]
    pub id: String,
    pub kind: ModelKind,
    pub name: String,
    /// SPDX identifier; shown before a download because some licences need attribution.
    pub license: String,
    /// Immutable location (a Hugging Face commit or a release tag) ending in `/`.
    pub base_url: String,
    #[schemars(length(min = 1))]
    pub files: Vec<ModelFile>,
}

impl Model {
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// The set of models aulo knows how to download.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub models: Vec<Model>,
}

impl Catalog {
    /// The catalog shipped in the binary.
    pub fn embedded() -> Result<Self, ModelError> {
        Self::parse(EMBEDDED)
    }

    pub fn parse(json: &str) -> Result<Self, ModelError> {
        let catalog: Self =
            serde_json::from_str(json).map_err(|e| ModelError::Catalog(e.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn get(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Checks what the schema cannot express. Names become paths on disk, so
    /// they are rejected here even though the catalog is ours.
    pub fn validate(&self) -> Result<(), ModelError> {
        let mut ids = BTreeSet::new();
        for model in &self.models {
            let id = &model.id;
            let bad = |msg: String| Err(ModelError::Catalog(format!("{id}: {msg}")));
            if !is_safe_id(id) {
                return bad("not a safe directory name".into());
            }
            if !ids.insert(id.as_str()) {
                return bad("duplicate model id".into());
            }
            let http =
                model.base_url.starts_with("https://") || model.base_url.starts_with("http://");
            if !http || !model.base_url.ends_with('/') {
                return bad("base_url must be an http(s) URL ending in `/`".into());
            }
            if model.files.is_empty() {
                return bad("no files".into());
            }
            let mut paths = BTreeSet::new();
            for file in &model.files {
                if !is_safe_relative(&file.path) {
                    return bad(format!("unsafe file path `{}`", file.path));
                }
                if !paths.insert(file.path.as_str()) {
                    return bad(format!("duplicate file `{}`", file.path));
                }
                if file.size == 0 || !is_sha256(&file.sha256) {
                    return bad(format!(
                        "`{}` needs a size and a lowercase SHA-256",
                        file.path
                    ));
                }
            }
            // `a` next to `a/b` cannot both exist on disk.
            if let Some(file) = model
                .files
                .iter()
                .find(|f| paths.iter().any(|p| p.starts_with(&format!("{}/", f.path))))
            {
                return bad(format!("`{}` is both a file and a directory", file.path));
            }
        }
        Ok(())
    }
}

/// The JSON Schema of [`Catalog`], pretty-printed with a trailing newline.
pub fn schema_json() -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(&schemars::schema_for!(Catalog))?;
    json.push('\n');
    Ok(json)
}

pub(crate) fn is_safe_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.len() <= 64
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// A relative path made of plain components. Backslashes and colons are out
/// because Windows reads them as separators and drive letters.
fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 200
        && !path.ends_with(PART_SUFFIX)
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
