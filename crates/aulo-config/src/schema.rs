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
    stable_pretty(&schema)
}

/// Pretty-print with object keys sorted. `llm-wire` enables `serde_json`'s
/// `preserve_order` for the whole workspace, which would otherwise rewrite
/// this committed file whenever that crate is in the build.
fn stable_pretty(value: &impl serde::Serialize) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string_pretty(&sorted(serde_json::to_value(value)?))?;
    json.push('\n');
    Ok(json)
}

fn sorted(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted_map = serde_json::Map::new();
            for (key, child) in entries {
                sorted_map.insert(key, sorted(child));
            }
            Value::Object(sorted_map)
        }
        other => other,
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
