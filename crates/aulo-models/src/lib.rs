//! Speech model manager: a committed catalog with pinned SHA-256 hashes and
//! a downloader that refuses anything that does not match.
//!
//! Models live in `<AULO_HOME>/models/<id>/`. The CLI wiring (`aulo models
//! pull/ls/rm`) calls [`ModelManager`]; this crate holds no CLI or UI code.

mod catalog;
mod download;
mod error;
mod manager;

pub use catalog::{Catalog, Model, ModelFile, ModelKind, schema_json};
pub use error::ModelError;
pub use manager::{ModelManager, ModelStatus, Progress};
