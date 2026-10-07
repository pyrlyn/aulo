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

/// The catalog id of the only Kokoro bundle the speech engine loads, the
/// fp32 multi-language v1.0 bundle (int8 is about 3x slower on Apple silicon,
/// spike R3). Pinned for the same reason as [`PARAKEET_MODEL_ID`].
pub const KOKORO_MODEL_ID: &str = "kokoro-multi-lang-v1_0";

const KOKORO_ONNX: &str = "model.onnx";
const KOKORO_VOICES: &str = "voices.bin";
const KOKORO_DATA: &str = "espeak-ng-data";
const KOKORO_DICT: &str = "dict";
const KOKORO_LEXICONS: [&str; 2] = ["lexicon-us-en.txt", "lexicon-zh.txt"];

/// One speaker's style table in `voices.bin`: 510 x 1 x 256 `f32`
/// (`EXPECTED_STYLE_SHAPE` in sherpa-onnx's `generate_voices_bin.py`).
const KOKORO_STYLE_BYTES: u64 = 510 * 256 * 4;

/// How a token table numbers its lines.
#[derive(Debug, Clone, Copy)]
enum Numbering {
    /// 0, 1, 2, ...: the transducer's output index is the line number.
    Dense,
    /// Strictly increasing: Kokoro's phoneme table skips ids.
    Increasing,
}

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
        let bundle = Bundle::check(model, dir, PARAKEET_MODEL_ID, ModelKind::Stt)?;
        Ok(Self {
            encoder: bundle.onnx(ENCODER)?,
            decoder: bundle.onnx(DECODER)?,
            joiner: bundle.onnx(JOINER)?,
            tokens: bundle.tokens(TOKENS, Numbering::Dense)?,
        })
    }
}

/// Paths of a validated Kokoro bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroFiles {
    pub(crate) model: String,
    pub(crate) voices: String,
    pub(crate) tokens: String,
    pub(crate) data_dir: String,
    pub(crate) dict_dir: String,
    /// Comma-separated, which is how sherpa-onnx takes several lexicons.
    pub(crate) lexicon: String,
}

impl KokoroFiles {
    /// Like [`ParakeetFiles::locate`], plus a voice table that holds exactly
    /// `speakers` voices: sherpa-onnx quietly maps an out-of-range speaker id
    /// to 0, so a table of another size would put the wrong names on voices.
    pub fn locate(model: &Model, dir: &Path, speakers: usize) -> Result<Self, SpeechError> {
        let bundle = Bundle::check(model, dir, KOKORO_MODEL_ID, ModelKind::Tts)?;
        if bundle.size(KOKORO_VOICES) != Some(speakers as u64 * KOKORO_STYLE_BYTES) {
            return Err(SpeechError::unavailable(
                "voices.bin does not hold the voices this engine names",
            ));
        }
        let lexicons = KOKORO_LEXICONS
            .iter()
            .map(|name| bundle.path(name))
            .collect::<Result<Vec<_>, _>>()?;
        if lexicons.iter().any(|path| path.contains(',')) {
            return Err(SpeechError::unavailable(
                "model path contains a comma, which sherpa-onnx reads as a list separator",
            ));
        }
        Ok(Self {
            model: bundle.onnx(KOKORO_ONNX)?,
            voices: bundle.path(KOKORO_VOICES)?,
            tokens: bundle.tokens(TOKENS, Numbering::Increasing)?,
            data_dir: bundle.path(KOKORO_DATA)?,
            dict_dir: bundle.path(KOKORO_DICT)?,
            lexicon: lexicons.join(","),
        })
    }
}

/// A model directory whose files all have their pinned sizes.
struct Bundle<'a> {
    model: &'a Model,
    dir: &'a Path,
}

impl<'a> Bundle<'a> {
    /// Stats every catalog file, which also covers data directories such as
    /// `espeak-ng-data` that sherpa-onnx reads without naming each file.
    fn check(
        model: &'a Model,
        dir: &'a Path,
        id: &str,
        kind: ModelKind,
    ) -> Result<Self, SpeechError> {
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
                    "{} is missing or not the pinned size; run `aulo models pull {}`",
                    file.path, model.id
                )));
            }
        }
        Ok(Self { model, dir })
    }

    fn size(&self, name: &str) -> Option<u64> {
        self.model
            .files
            .iter()
            .find(|f| f.path == name)
            .map(|f| f.size)
    }

    /// A file the catalog lists, or a directory that listed files sit in.
    fn path(&self, name: &str) -> Result<String, SpeechError> {
        let inside = format!("{name}/");
        if !self
            .model
            .files
            .iter()
            .any(|f| f.path == name || f.path.starts_with(&inside))
        {
            return Err(SpeechError::unavailable(&format!(
                "catalog entry has no {name}"
            )));
        }
        // Interior NUL would make the sherpa-onnx crate panic in `CString::new`.
        self.dir
            .join(name)
            .to_str()
            .filter(|p| !p.contains('\0'))
            .map(str::to_owned)
            .ok_or_else(|| SpeechError::unavailable("model path is not valid UTF-8"))
    }

    fn onnx(&self, name: &str) -> Result<String, SpeechError> {
        let path = self.path(name)?;
        check_onnx(Path::new(&path))?;
        Ok(path)
    }

    fn tokens(&self, name: &str, numbering: Numbering) -> Result<String, SpeechError> {
        let path = self.path(name)?;
        let size = self
            .size(name)
            .ok_or_else(|| SpeechError::unavailable("token table is not a file"))?;
        check_tokens(Path::new(&path), size, numbering)?;
        Ok(path)
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

/// One `<token> <id>` pair per line, numbered as sherpa-onnx's symbol table
/// expects. The read is capped at the pinned size, which `check` matched.
fn check_tokens(path: &Path, size: u64, numbering: Numbering) -> Result<(), SpeechError> {
    let bad = || SpeechError::unavailable("token table is malformed");
    let mut text = String::new();
    File::open(path)
        .map_err(|_| bad())?
        .take(size)
        .read_to_string(&mut text)
        .map_err(|_| bad())?;
    let mut previous: Option<usize> = None;
    for (line, entry) in text.lines().enumerate() {
        let id = entry
            .rsplit_once(' ')
            .and_then(|(token, id)| (!token.is_empty()).then_some(id))
            .and_then(|id| id.parse::<usize>().ok())
            .ok_or_else(bad)?;
        let in_order = match numbering {
            Numbering::Dense => id == line,
            Numbering::Increasing => previous.is_none_or(|p| id > p),
        };
        if !in_order {
            return Err(bad());
        }
        previous = Some(id);
    }
    previous.map(drop).ok_or_else(bad)
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

    fn write(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, bytes) in files {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        dir
    }

    /// A Kokoro bundle with a voice table of `speakers` voices.
    pub(crate) fn kokoro_bundle(speakers: usize) -> (tempfile::TempDir, Model) {
        let voices = vec![0_u8; speakers * KOKORO_STYLE_BYTES as usize];
        let files: [(&str, &[u8]); 7] = [
            (KOKORO_ONNX, ONNX),
            (KOKORO_VOICES, &voices),
            (TOKENS, b"; 1\n  16\n\xc9\x99 83\n"),
            ("espeak-ng-data/phontab", b"x"),
            ("dict/jieba.dict.utf8", b"x"),
            (KOKORO_LEXICONS[0], b"kokoro k O\n"),
            (KOKORO_LEXICONS[1], b"x"),
        ];
        let mut model = model(&files);
        model.id = KOKORO_MODEL_ID.into();
        model.kind = ModelKind::Tts;
        (write(&files), model)
    }

    pub(crate) fn bundle(tokens: &str) -> (tempfile::TempDir, Model) {
        let files: [(&str, &[u8]); 4] = [
            (ENCODER, ONNX),
            (DECODER, ONNX),
            (JOINER, ONNX),
            (TOKENS, tokens.as_bytes()),
        ];
        (write(&files), model(&files))
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

    #[test]
    fn a_kokoro_bundle_is_located_with_its_data_directories() {
        let (dir, model) = kokoro_bundle(2);
        let files = KokoroFiles::locate(&model, dir.path(), 2).unwrap();
        assert!(files.model.ends_with(KOKORO_ONNX));
        assert!(files.data_dir.ends_with(KOKORO_DATA));
        assert!(files.dict_dir.ends_with(KOKORO_DICT));
        let lexicons: Vec<_> = files.lexicon.split(',').collect();
        assert_eq!(lexicons.len(), 2);
        assert!(lexicons[1].ends_with(KOKORO_LEXICONS[1]));
    }

    #[test]
    fn a_voice_table_of_another_size_is_refused() {
        let (dir, model) = kokoro_bundle(2);
        let error = KokoroFiles::locate(&model, dir.path(), 3).unwrap_err();
        assert!(error.to_string().contains("voices.bin"), "{error}");
        assert!(KokoroFiles::locate(&model, dir.path(), 1).is_err());
    }

    #[test]
    fn kokoro_checks_every_data_file_and_its_token_table() {
        let (dir, model) = kokoro_bundle(1);
        fs::remove_file(dir.path().join("espeak-ng-data/phontab")).unwrap();
        let error = KokoroFiles::locate(&model, dir.path(), 1).unwrap_err();
        assert!(error.to_string().contains("phontab"), "{error}");

        let (dir, model) = kokoro_bundle(1);
        // Same size, ids out of order.
        fs::write(dir.path().join(TOKENS), b"; 1\n  16\n\xc9\x99 13\n").unwrap();
        let error = KokoroFiles::locate(&model, dir.path(), 1).unwrap_err();
        assert!(error.to_string().contains("token table"), "{error}");

        let (dir, mut model) = kokoro_bundle(1);
        model.kind = ModelKind::Stt;
        let error = KokoroFiles::locate(&model, dir.path(), 1).unwrap_err();
        assert!(matches!(error, SpeechError::Unsupported(_)));
    }

    #[test]
    fn a_comma_in_the_model_path_is_refused() {
        let (dir, model) = kokoro_bundle(1);
        let odd = dir.path().join("a,b");
        fs::create_dir(&odd).unwrap();
        for file in &model.files {
            let from = dir.path().join(&file.path);
            let to = odd.join(&file.path);
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            fs::copy(from, to).unwrap();
        }
        let error = KokoroFiles::locate(&model, &odd, 1).unwrap_err();
        assert!(error.to_string().contains("comma"), "{error}");
    }

    #[test]
    fn the_catalog_kokoro_voice_table_matches_the_engine() {
        let catalog = aulo_models::Catalog::embedded().unwrap();
        let model = catalog.get(KOKORO_MODEL_ID).unwrap();
        let voices = model
            .files
            .iter()
            .find(|f| f.path == KOKORO_VOICES)
            .unwrap();
        assert_eq!(
            voices.size,
            crate::tts::SPEAKERS.len() as u64 * KOKORO_STYLE_BYTES
        );
        for name in [KOKORO_ONNX, TOKENS, KOKORO_LEXICONS[0], KOKORO_LEXICONS[1]] {
            assert!(model.files.iter().any(|f| f.path == name), "{name}");
        }
    }
}
