use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{Client, redirect};

use crate::catalog::{Catalog, Model, ModelKind, is_safe_id};
use crate::download::{Expected, fetch_verified};
use crate::error::ModelError;

/// Install state of one catalog model, as `aulo models ls` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStatus {
    pub id: String,
    pub kind: ModelKind,
    pub name: String,
    pub license: String,
    pub size: u64,
    pub files_present: usize,
    pub files_total: usize,
}

impl ModelStatus {
    pub fn installed(&self) -> bool {
        self.files_present == self.files_total
    }
}

/// Download progress; `bytes` and `total_bytes` count the whole model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress<'a> {
    pub model: &'a str,
    pub file: &'a str,
    pub file_bytes: u64,
    pub file_size: u64,
    pub bytes: u64,
    pub total_bytes: u64,
}

/// Installs, lists and removes catalog models under `<AULO_HOME>/models`.
#[derive(Debug)]
pub struct ModelManager {
    models_dir: PathBuf,
    catalog: Catalog,
    client: Client,
}

impl ModelManager {
    pub fn new(home: &Path, catalog: Catalog) -> Result<Self, ModelError> {
        catalog.validate()?;
        // The hash, not the transport, is the trust anchor, so redirects to a
        // CDN are fine; the limit only bounds a redirect loop. There is no total
        // timeout because a model is hundreds of megabytes.
        let client = Client::builder()
            .user_agent(concat!("aulo-models/", env!("CARGO_PKG_VERSION")))
            .redirect(redirect::Policy::limited(5))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            models_dir: home.join("models"),
            catalog,
            client,
        })
    }

    pub fn list(&self) -> Vec<ModelStatus> {
        self.catalog
            .models
            .iter()
            .map(|m| ModelStatus {
                id: m.id.clone(),
                kind: m.kind,
                name: m.name.clone(),
                license: m.license.clone(),
                size: m.total_size(),
                files_present: m.files.iter().filter(|f| self.has(m, f)).count(),
                files_total: m.files.len(),
            })
            .collect()
    }

    /// Directory of an installed model, for the engine that loads it.
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        let model = self.catalog.get(id)?;
        model
            .files
            .iter()
            .all(|f| self.has(model, f))
            .then(|| self.models_dir.join(id))
    }

    /// Downloads every missing file of `id`, verifying each against its pinned
    /// SHA-256, and returns the model directory. Files that are already
    /// installed are kept: they only reach their final name after a hash check.
    pub async fn pull(
        &self,
        id: &str,
        mut on_progress: impl FnMut(&Progress<'_>),
    ) -> Result<PathBuf, ModelError> {
        let model = self
            .catalog
            .get(id)
            .ok_or_else(|| ModelError::UnknownModel(id.into()))?;
        let dir = self.models_dir.join(&model.id);
        let total_bytes = model.total_size();
        let mut finished = 0;
        for file in &model.files {
            let dest = dir.join(&file.path);
            let mut report = |file_bytes| {
                on_progress(&Progress {
                    model: &model.id,
                    file: &file.path,
                    file_bytes,
                    file_size: file.size,
                    bytes: finished + file_bytes,
                    total_bytes,
                });
            };
            if self.has(model, file) {
                report(file.size);
            } else {
                if let Some(parent) = dest.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(ModelError::io(parent))?;
                }
                let url = format!("{}{}", model.base_url, file.path);
                let want = Expected {
                    url: &url,
                    name: &file.path,
                    size: file.size,
                    sha256: &file.sha256,
                };
                fetch_verified(&self.client, &want, &dest, &mut report).await?;
            }
            finished += file.size;
        }
        Ok(dir)
    }

    /// Deletes a model directory, partial downloads included. `false` when
    /// there was nothing to delete.
    pub fn remove(&self, id: &str) -> Result<bool, ModelError> {
        if !is_safe_id(id) {
            return Err(ModelError::UnsafeName(id.into()));
        }
        let dir = self.models_dir.join(id);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ModelError::Io {
                path: dir,
                source: e,
            }),
        }
    }

    /// Files only get their final name after verification, so the pinned size
    /// is enough to tell an installed file from damage; hashing hundreds of
    /// megabytes on every `ls` would not be.
    fn has(&self, model: &Model, file: &crate::catalog::ModelFile) -> bool {
        std::fs::metadata(self.models_dir.join(&model.id).join(&file.path))
            .is_ok_and(|m| m.is_file() && m.len() == file.size)
    }
}
