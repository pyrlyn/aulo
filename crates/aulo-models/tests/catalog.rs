use std::path::PathBuf;

use aulo_models::{Catalog, ModelError};
use serde_json::{Value, json};

fn committed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/aulo-models.schema.json")
}

#[test]
fn committed_schema_is_current() {
    let generated = aulo_models::schema_json().unwrap();
    let path = committed_path();
    if std::env::var_os("AULO_UPDATE_SCHEMA").is_some_and(|v| v == "1") {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "{} is stale; regenerate with AULO_UPDATE_SCHEMA=1 cargo test -p aulo-models --test catalog",
        path.display()
    );
}

#[test]
fn the_embedded_catalog_is_valid_and_pinned() {
    let catalog = Catalog::embedded().unwrap();
    assert!(!catalog.models.is_empty());
    for model in &catalog.models {
        assert!(model.base_url.starts_with("https://"), "{}", model.id);
        // Hugging Face URLs must name a commit, not a branch that can move.
        if let Some(rest) = model.base_url.split("/resolve/").nth(1) {
            let rev = rest.trim_end_matches('/');
            assert!(
                rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()),
                "{}: {rev}",
                model.id
            );
        }
    }
}

fn catalog_with(file_path: &str) -> Value {
    json!({"models": [{
        "id": "m", "kind": "stt", "name": "M", "license": "MIT",
        "base_url": "https://example.com/m/",
        "files": [{"path": file_path, "size": 1, "sha256": "0".repeat(64)}],
    }]})
}

#[test]
fn unsafe_paths_ids_and_hashes_are_rejected() {
    for bad in [
        "../x", "/abs", "a/../b", "a//b", "", ".", "a\\b", "C:x", "x.part",
    ] {
        let err = Catalog::parse(&catalog_with(bad).to_string()).unwrap_err();
        assert!(matches!(err, ModelError::Catalog(_)), "{bad}");
    }
    assert!(Catalog::parse(&catalog_with("sub/ok.bin").to_string()).is_ok());

    let mut value = catalog_with("a");
    value["models"][0]["id"] = json!("../evil");
    assert!(Catalog::parse(&value.to_string()).is_err());

    let mut value = catalog_with("a");
    value["models"][0]["files"][0]["sha256"] = json!("ABC");
    assert!(Catalog::parse(&value.to_string()).is_err());

    let mut value = catalog_with("a");
    value["models"][0]["unknown"] = json!(true);
    assert!(Catalog::parse(&value.to_string()).is_err());
}

#[test]
fn duplicates_and_file_directory_clashes_are_rejected() {
    let mut value = catalog_with("a");
    let file = value["models"][0]["files"][0].clone();
    value["models"][0]["files"] = json!([file, file]);
    assert!(Catalog::parse(&value.to_string()).is_err());

    let mut value = catalog_with("a");
    let nested = json!({"path": "a/b", "size": 1, "sha256": "0".repeat(64)});
    value["models"][0]["files"]
        .as_array_mut()
        .unwrap()
        .push(nested);
    assert!(Catalog::parse(&value.to_string()).is_err());

    let mut value = catalog_with("a");
    let model = value["models"][0].clone();
    value["models"] = json!([model, model]);
    assert!(Catalog::parse(&value.to_string()).is_err());
}
