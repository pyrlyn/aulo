use std::path::PathBuf;

fn committed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/aulo.schema.json")
}

#[test]
fn committed_schema_is_current() {
    let generated = aulo_config::schema_json().unwrap();
    let path = committed_path();
    if std::env::var_os("AULO_UPDATE_SCHEMA").is_some_and(|v| v == "1") {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "{} is stale; regenerate with AULO_UPDATE_SCHEMA=1 cargo test -p aulo-config --test schema",
        path.display()
    );
}

#[test]
fn schema_rejects_unknown_keys_and_has_no_null() {
    let json = aulo_config::schema_json().unwrap();
    assert!(json.contains("\"additionalProperties\": false"));
    assert!(!json.contains("null"));
}
