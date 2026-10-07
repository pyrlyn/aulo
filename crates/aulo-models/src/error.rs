use std::path::PathBuf;

/// Why a catalog could not be loaded or a model could not be installed.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("invalid model catalog: {0}")]
    Catalog(String),
    #[error("unknown model `{0}`")]
    UnknownModel(String),
    #[error("unsafe name `{0}`")]
    UnsafeName(String),
    #[error("{url} answered HTTP {status}")]
    Http { url: String, status: u16 },
    #[error("{url} ignored the requested range or answered with the wrong one")]
    BadRange { url: String },
    #[error("download failed: {0}")]
    Network(#[from] reqwest::Error),
    #[error("{file}: SHA-256 is {actual}, the catalog pins {expected}; nothing was kept")]
    HashMismatch {
        file: String,
        expected: String,
        actual: String,
    },
    #[error("{file}: the server sent more than the pinned {limit} bytes; nothing was kept")]
    TooLarge { file: String, limit: u64 },
    #[error(
        "{file}: the server stopped after {got} of {expected} bytes; run the pull again to resume"
    )]
    Truncated {
        file: String,
        got: u64,
        expected: u64,
    },
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ModelError {
    /// `map_err` adapter that names the file the I/O error is about.
    pub(crate) fn io(path: &std::path::Path) -> impl Fn(std::io::Error) -> Self {
        let path = path.to_owned();
        move |source| Self::Io {
            path: path.clone(),
            source,
        }
    }
}
