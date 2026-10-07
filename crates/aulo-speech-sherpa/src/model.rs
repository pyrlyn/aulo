//! Model files checked before sherpa-onnx sees them. sherpa-onnx aborts the
//! whole process on a model it cannot run (spike T0.5, R5) instead of
//! returning an error, so everything that can be checked cheaply is checked
//! here, where a failure is an ordinary `Err` and the registry falls back.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use aulo_models::{Model, ModelKind};
use aulo_speech::SpeechError;

/// The catalog id of the only Parakeet bundle this engine loads. Pinning the
/// id pins the hashes too: aulo-models gives a file its final name only after
/// its SHA-256 matched, so a model that is not this exact bundle never reaches
/// sherpa-onnx. The spike crashed on a different bundle of the right shape.
pub const PARAKEET_MODEL_ID: &str = "parakeet-tdt-0.6b-v3-int8";

const ENCODER: &str = "encoder.int8.onnx";
const DECODER: &str = "decoder.int8.onnx";
const JOINER: &str = "joiner.int8.onnx";
const TOKENS: &str = "tokens.txt";

/// Every ONNX file is a protobuf `ModelProto` that starts with field 1,
/// `ir_version`, as a varint: tag byte `0x08`. An HTML error page or a
/// truncated download saved under the right name fails this.
const ONNX_FIRST_BYTE: u8 = 0x08;

/// Paths of a validated Parakeet bundle, as UTF-8 because the sherpa-onnx
/// config takes strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParakeetFiles {
    pub(crate) encoder: String,
    pub(crate) decoder: String,
    pub(crate) joiner: String,
    pub(crate) tokens: String,
}

impl ParakeetFiles {
    /// Checks `dir` (from `ModelManager::path`) against the catalog entry
    /// `model`: the right model, every file present with its pinned size, ONNX
    /// headers and a well-formed token table. Reads only the token table and
    /// one byte per ONNX file, so it is cheap enough to run on every load.
    pub fn locate(model: &Model, dir: &Path) -> Result<Self, SpeechError> {
        if model.id != PARAKEET_MODEL_ID || model.kind != ModelKind::Stt {
            return Err(SpeechError::unsupported(
                "not the Parakeet TDT v3 int8 model",
            ));
        }
        let checked = |name: &str| -> Result<String, SpeechError> {
            let Some(file) = model.files.iter().find(|f| f.path == name) else {
                return Err(SpeechError::unavailable(&format!(
                    "catalog entry has no {name}"
                )));
            };
            let path = dir.join(&file.path);
            let size = fs::metadata(&path)
                .ok()
                .filter(|m| m.is_file())
                .map(|m| m.len());
            if size != Some(file.size) {
                return Err(SpeechError::unavailable(&format!(
                    "{name} is missing or not the pinned size; run `aulo models pull {}`",
                    model.id
                )));
            }
            if name == TOKENS {
                check_tokens(&path, file.size)?;
            } else {
                check_onnx(&path)?;
            }
            // Interior NUL would make the sherpa-onnx crate panic in `CString::new`.
            path.to_str()
                .filter(|p| !p.contains('\0'))
                .map(str::to_owned)
                .ok_or_else(|| SpeechError::unavailable("model path is not valid UTF-8"))
        };
        Ok(Self {
            encoder: checked(ENCODER)?,
            decoder: checked(DECODER)?,
            joiner: checked(JOINER)?,
            tokens: checked(TOKENS)?,
        })
    }
}

fn check_onnx(path: &Path) -> Result<(), SpeechError> {
    let mut first = [0_u8; 1];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut first))
        .map_err(|_| SpeechError::unavailable("cannot read an ONNX model file"))?;
    if first[0] != ONNX_FIRST_BYTE {
        return Err(SpeechError::unavailable("model file is not ONNX"));
    }
    Ok(())
}

/// One `<token> <id>` pair per line with ids 0, 1, 2, ... in order, which is
/// what sherpa-onnx's symbol table expects. The read is capped at the pinned
/// size, which `locate` already matched.
fn check_tokens(path: &Path, size: u64) -> Result<(), SpeechError> {
    let bad = || SpeechError::unavailable("token table is malformed");
    let mut text = String::new();
    File::open(path)
        .map_err(|_| bad())?
        .take(size)
        .read_to_string(&mut text)
        .map_err(|_| bad())?;
    let mut count = 0_usize;
    for (expected, line) in text.lines().enumerate() {
        let id = line
            .rsplit_once(' ')
            .and_then(|(token, id)| (!token.is_empty()).then_some(id))
            .and_then(|id| id.parse::<usize>().ok());
        if id != Some(expected) {
            return Err(bad());
        }
        count += 1;
    }
    if count == 0 {
        return Err(bad());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use aulo_models::ModelFile;

    const ONNX: &[u8] = &[ONNX_FIRST_BYTE, 0x07, 0x12];
    pub(crate) const TOKEN_TABLE: &str = "<unk> 0\n\u{2581}the 1\n\u{2581} 2\n";

    fn model(files: &[(&str, &[u8])]) -> Model {
        Model {
            id: PARAKEET_MODEL_ID.into(),
            kind: ModelKind::Stt,
            name: "Parakeet".into(),
            license: "CC-BY-4.0".into(),
            base_url: "https://example.invalid/".into(),
            files: files
                .iter()
                .map(|(path, bytes)| ModelFile {
                    path: (*path).into(),
                    size: bytes.len() as u64,
                    sha256: "0".repeat(64),
                })
                .collect(),
        }
    }

    pub(crate) fn bundle(tokens: &str) -> (tempfile::TempDir, Model) {
        let files: [(&str, &[u8]); 4] = [
            (ENCODER, ONNX),
            (DECODER, ONNX),
            (JOINER, ONNX),
            (TOKENS, tokens.as_bytes()),
        ];
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in files {
            fs::write(dir.path().join(name), bytes).unwrap();
        }
        (dir, model(&files))
    }

    #[test]
    fn a_complete_bundle_is_located() {
        let (dir, model) = bundle(TOKEN_TABLE);
        let files = ParakeetFiles::locate(&model, dir.path()).unwrap();
        assert!(files.encoder.ends_with(ENCODER));
        assert!(files.tokens.ends_with(TOKENS));
    }

    #[test]
    fn another_catalog_model_is_refused() {
        let (dir, mut model) = bundle(TOKEN_TABLE);
        model.id = "parakeet-mobile".into();
        let error = ParakeetFiles::locate(&model, dir.path()).unwrap_err();
        assert!(matches!(error, SpeechError::Unsupported(_)));
        model.id = PARAKEET_MODEL_ID.into();
        model.kind = ModelKind::Tts;
        assert!(ParakeetFiles::locate(&model, dir.path()).is_err());
    }

    #[test]
    fn missing_or_resized_files_are_unavailable() {
        let (dir, model) = bundle(TOKEN_TABLE);
        fs::write(dir.path().join(JOINER), [ONNX_FIRST_BYTE]).unwrap();
        let error = ParakeetFiles::locate(&model, dir.path()).unwrap_err();
        assert!(matches!(error, SpeechError::Unavailable(_)), "{error}");
        fs::remove_file(dir.path().join(JOINER)).unwrap();
        assert!(ParakeetFiles::locate(&model, dir.path()).is_err());
        let empty = tempfile::tempdir().unwrap();
        assert!(ParakeetFiles::locate(&model, empty.path()).is_err());
    }

    #[test]
    fn a_non_onnx_file_of_the_right_size_is_refused() {
        let (dir, model) = bundle(TOKEN_TABLE);
        fs::write(dir.path().join(ENCODER), b"<ht").unwrap();
        let error = ParakeetFiles::locate(&model, dir.path()).unwrap_err();
        assert!(error.to_string().contains("not ONNX"), "{error}");
    }

    #[test]
    fn malformed_token_tables_are_refused() {
        for tokens in [
            "",
            "<unk> 1\n",
            "<unk> 0\nthe 2\n",
            " 0\n",
            "<unk>0\n",
            "\u{0}\u{ff}",
        ] {
            let (dir, model) = bundle(tokens);
            assert!(
                ParakeetFiles::locate(&model, dir.path()).is_err(),
                "{tokens:?} was accepted"
            );
        }
    }
}
