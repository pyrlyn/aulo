//! The check every local engine runs on a catalog model before loading it.

use std::fs;
use std::path::Path;

use aulo_models::{Model, ModelKind};
use aulo_speech::SpeechError;

/// Checks that `model` is the catalog entry `id` of `kind` and that every file
/// of it sits in `dir` with its pinned size. Pinning the id pins the hashes
/// too: aulo-models gives a file its final name only after its SHA-256
/// matched, so a size check is enough to catch a missing or truncated file
/// without hashing hundreds of megabytes on every load.
pub fn check_installed(
    model: &Model,
    dir: &Path,
    id: &str,
    kind: ModelKind,
) -> Result<(), SpeechError> {
    if model.id != id || model.kind != kind {
        return Err(SpeechError::unsupported(&format!("not the {id} model")));
    }
    for file in &model.files {
        let size = fs::metadata(dir.join(&file.path))
            .ok()
            .filter(|m| m.is_file())
            .map(|m| m.len());
        if size != Some(file.size) {
            return Err(SpeechError::unavailable(&format!(
                "{} under {} is missing or not the pinned size for model {}",
                file.path,
                dir.display(),
                model.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use aulo_models::Catalog;

    use super::*;

    fn model() -> Model {
        let json = format!(
            r#"{{"models": [{{"id": "m", "kind": "stt", "name": "M", "license": "MIT",
            "base_url": "https://example.com/m/",
            "files": [{{"path": "a.bin", "size": 3, "sha256": "{}"}}]}}]}}"#,
            "0".repeat(64)
        );
        Catalog::parse(&json).unwrap().models.remove(0)
    }

    #[test]
    fn a_complete_install_passes_and_a_damaged_one_does_not() {
        let model = model();
        let dir = tempfile::tempdir().unwrap();
        let missing = check_installed(&model, dir.path(), "m", ModelKind::Stt).unwrap_err();
        assert!(matches!(missing, SpeechError::Unavailable(_)));
        fs::write(dir.path().join("a.bin"), b"abcd").unwrap();
        let wrong_size = check_installed(&model, dir.path(), "m", ModelKind::Stt).unwrap_err();
        assert!(matches!(wrong_size, SpeechError::Unavailable(_)));
        fs::write(dir.path().join("a.bin"), b"abc").unwrap();
        check_installed(&model, dir.path(), "m", ModelKind::Stt).unwrap();
    }

    #[test]
    fn another_model_or_kind_is_unsupported() {
        let model = model();
        let dir = tempfile::tempdir().unwrap();
        for (id, kind) in [("other", ModelKind::Stt), ("m", ModelKind::Tts)] {
            let error = check_installed(&model, dir.path(), id, kind).unwrap_err();
            assert!(matches!(error, SpeechError::Unsupported(_)));
        }
    }
}
