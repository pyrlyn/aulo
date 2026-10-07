use std::path::PathBuf;

fn committed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/aulo-plugin.schema.json")
}

#[test]
fn committed_schema_is_current() {
    let generated = aulo_plugin::manifest::schema_json().unwrap();
    let path = committed_path();
    if std::env::var_os("AULO_UPDATE_SCHEMA").is_some_and(|v| v == "1") {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "{} is stale; regenerate with AULO_UPDATE_SCHEMA=1 cargo test -p aulo-plugin --test schema",
        path.display()
    );
}

#[test]
fn schema_rejects_unknown_keys_and_has_no_null() {
    let json = aulo_plugin::manifest::schema_json().unwrap();
    assert!(json.contains("\"additionalProperties\": false"));
    assert!(!json.contains("null"));
}

#[test]
fn schema_requires_id_version_and_kinds_but_not_entry() {
    let schema: serde_json::Value =
        serde_json::from_str(&aulo_plugin::manifest::schema_json().unwrap()).unwrap();
    let required = schema["required"].as_array().unwrap();
    let names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert_eq!(names, ["id", "version", "kinds"]);
    assert_eq!(schema["properties"]["kinds"]["minItems"], 1);
}
