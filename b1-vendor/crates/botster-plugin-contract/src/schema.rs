//! Schema helpers: self-contained (inlined) schemas for tables that embed many schemas in one artifact.
//!
//! `null` is not valid for an optional field: the contract allows `null` only where it says so (HP WF-4). A schema that marks itself with
//! `NULL_ALLOWED` keeps its `null`; the marker is removed from the artifact.

use schemars::generate::{SchemaGenerator, SchemaSettings};
use schemars::transform::{transform_subschemas, Transform};
use schemars::{JsonSchema, Schema};
use serde_json::Value;

/// The key that marks a schema whose `null` is stated by a clause (HP WF-4). It is removed from the generated artifact.
pub const NULL_ALLOWED: &str = "x-null-allowed";

fn is_null_schema(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("null") && v.get(NULL_ALLOWED).is_none()
}

/// Removes `null` from `type` lists and from `anyOf` and `oneOf` lists, except in a schema marked `NULL_ALLOWED` (HP WF-4).
#[derive(Clone)]
struct NoNull;

impl Transform for NoNull {
    fn transform(&mut self, schema: &mut Schema) {
        if let Some(o) = schema.as_object_mut() {
            if o.remove(NULL_ALLOWED).is_some() {
                transform_subschemas(self, schema);
                return;
            }
            if let Some(Value::Array(types)) = o.get_mut("type") {
                types.retain(|t| t != "null");
                if types.len() == 1 {
                    let only = types.remove(0);
                    o.insert("type".into(), only);
                }
            }
            for key in ["anyOf", "oneOf"] {
                if let Some(Value::Array(items)) = o.get_mut(key) {
                    items.retain(|i| !is_null_schema(i));
                }
                if let Some(Value::Array(items)) = o.get(key) {
                    if items.len() == 1 {
                        let only = items[0].clone();
                        o.remove(key);
                        if let Value::Object(m) = only {
                            for (k, v) in m {
                                o.entry(k).or_insert(v);
                            }
                        }
                    }
                }
            }
        }
        transform_subschemas(self, schema);
    }
}

/// The generator settings of every artifact (draft 2020-12; an optional field is not nullable, HP WF-4).
pub fn settings() -> SchemaSettings {
    SchemaSettings::draft2020_12().with_transform(NoNull)
}

/// A generator with the artifact settings.
pub fn generator(inline: bool) -> SchemaGenerator {
    settings()
        .with(|s| s.inline_subschemas = inline)
        .into_generator()
}

/// The JSON Schema of `T` with subschemas inlined and no `$schema` key, so it can sit inside a larger artifact (HP Appendix B).
pub fn inline<T: JsonSchema>() -> Value {
    let mut v = serde_json::to_value(generator(true).into_root_schema_for::<T>())
        .expect("a schema serializes");
    if let Some(o) = v.as_object_mut() {
        o.remove("$schema");
    }
    v
}

/// The root schema of `T` with `$defs` (for a type that recurses).
pub fn rooted<T: JsonSchema>() -> Value {
    serde_json::to_value(generator(false).into_root_schema_for::<T>()).expect("a schema serializes")
}

/// `schema_with` for a field that is an object or `null` by clause (HP A2-21 `metadata`).
pub fn object_or_null(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({"type": ["object", "null"], "additionalProperties": true, NULL_ALLOWED: true})
}

/// `schema_with` for a field that is a string or `null` by clause (HP HS-1 `provider: null`).
pub fn string_or_null(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({"type": ["string", "null"], NULL_ALLOWED: true})
}
