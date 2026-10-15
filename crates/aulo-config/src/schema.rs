use schemars::Schema;
use schemars::generate::SchemaSettings;
use schemars::transform::RecursiveTransform;
use serde_json::Value;

use crate::model::Config;

/// The JSON Schema of [`Config`], pretty-printed with a trailing newline.
pub fn schema_json() -> Result<String, serde_json::Error> {
    let settings = SchemaSettings::draft2020_12()
        .with_transform(RecursiveTransform(drop_null as fn(&mut Schema)));
    let schema = settings.into_generator().into_root_schema_for::<Config>();
    stable_schema_json(schema)
}

/// Pretty-print `schema` with a trailing newline and lexicographic object keys.
///
/// `serde_json`'s `preserve_order` (pulled in by llm-wire) keeps object keys in
/// insertion order. schemars emits unpinned keywords in that order, and leaves
/// `properties` and `$defs` in map order entirely, so a schema would change
/// whenever another crate enables the feature. Sorting every map first makes
/// those keys lexicographic, which is what the committed file records.
/// Serializing the [`Schema`] (not the raw [`Value`]) keeps schemars' pinned
/// keyword order, so `$schema` stays ahead of `$defs`.
fn stable_schema_json(schema: Schema) -> Result<String, serde_json::Error> {
    let mut value = schema.to_value();
    sort_maps(&mut value);
    let schema = Schema::try_from(value)?;
    let mut json = serde_json::to_string_pretty(&schema)?;
    json.push('\n');
    Ok(json)
}

/// Object keys in alphabetical order, at every level. Arrays keep their order.
fn sort_maps(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                sort_maps(item);
            }
        }
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, mut child) in entries {
                sort_maps(&mut child);
                map.insert(key, child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// TOML has no null: an absent key is how a file says `None`, so `Option<T>`
/// is described as plain `T`. Handles both shapes schemars emits, `type:
/// [T, "null"]` and `anyOf: [T, {type: null}]`; nulls in `default`s go too.
fn drop_null(schema: &mut Schema) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    if let Some(default) = obj.get_mut("default") {
        strip_nulls(default);
        if default.is_null() {
            obj.remove("default");
        }
    }
    if let Some(Value::Array(types)) = obj.get_mut("type") {
        types.retain(|t| t != "null");
        if types.len() == 1 {
            let only = types.remove(0);
            obj.insert("type".into(), only);
        }
    }
    let Some(Value::Array(variants)) = obj.get_mut("anyOf") else {
        return;
    };
    let is_null = |v: &Value| v.get("type").is_some_and(|t| t == "null");
    if !variants.iter().any(is_null) {
        return;
    }
    variants.retain(|v| !is_null(v));
    if variants.len() == 1 {
        let only = variants.remove(0);
        obj.remove("anyOf");
        if let Value::Object(inner) = only {
            obj.extend(inner);
        }
    }
}

fn strip_nulls(value: &mut Value) {
    if let Value::Object(map) = value {
        map.retain(|_, v| !v.is_null());
        map.values_mut().for_each(strip_nulls);
    }
}
