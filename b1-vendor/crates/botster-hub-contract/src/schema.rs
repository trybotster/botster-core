//! The generated JSON Schemas (design section 8): the envelope, the pairing frames, the MCP tool data and one schema per kind.
//! `cargo xtask codegen` writes them to `schemas/hub-client/`.
//!
//! `null` is not valid for an optional field (A.1). A schema that marks itself with `x-null-allowed` keeps its `null` (a merge patch, VB-4).

use crate::prelude::*;
use schemars::generate::{SchemaGenerator, SchemaSettings};
use schemars::transform::{transform_subschemas, Transform};
use schemars::{JsonSchema, Schema};
use serde_json::{json, Map, Value};

/// The key that marks a schema whose `null` is stated by a clause. It is removed from the generated artifact.
pub const NULL_ALLOWED: &str = "x-null-allowed";

fn is_null_schema(v: &Value) -> bool {
    v.get("type").and_then(Value::as_str) == Some("null") && v.get(NULL_ALLOWED).is_none()
}

/// Removes `null` from `type`, `anyOf` and `oneOf` lists, except in a schema marked `NULL_ALLOWED`.
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

/// A JSON integer is within ±(2^53 − 1) (A.1). Every integer schema gets those bounds, unless its type has narrower ones.
///
/// Clause: HC A.1.
#[derive(Clone)]
struct SafeInts;

const MAX_SAFE: i64 = (1 << 53) - 1;

/// The range of an integer format that schemars writes, cut to the A.1 range.
fn format_range(format: &str) -> Option<(i64, i64)> {
    Some(match format {
        "uint8" => (0, u8::MAX as i64),
        "uint16" => (0, u16::MAX as i64),
        "uint32" => (0, u32::MAX as i64),
        "uint" | "uint64" => (0, MAX_SAFE),
        "int8" => (i8::MIN as i64, i8::MAX as i64),
        "int16" => (i16::MIN as i64, i16::MAX as i64),
        "int32" => (i32::MIN as i64, i32::MAX as i64),
        "int" | "int64" => (-MAX_SAFE, MAX_SAFE),
        _ => return None,
    })
}

impl Transform for SafeInts {
    fn transform(&mut self, schema: &mut Schema) {
        if let Some(o) = schema.as_object_mut() {
            if o.get("type").and_then(Value::as_str) == Some("integer") {
                let (low, high) = o
                    .get("format")
                    .and_then(Value::as_str)
                    .and_then(format_range)
                    .unwrap_or((-MAX_SAFE, MAX_SAFE));
                let have_max = o.get("maximum").and_then(Value::as_i64);
                if have_max.is_none_or(|m| m > high) {
                    o.insert("maximum".into(), json!(high));
                }
                let have_min = o.get("minimum").and_then(Value::as_i64);
                if have_min.is_none_or(|m| m < low) {
                    o.insert("minimum".into(), json!(low));
                }
            }
        }
        transform_subschemas(self, schema);
    }
}

fn generator() -> SchemaGenerator {
    SchemaSettings::draft2020_12()
        .with_transform(NoNull)
        .with_transform(SafeInts)
        .into_generator()
}

/// The codec sets that are non-exhaustive on the wire (Codec A.2): a receiver accepts a name that it does not know.
const OPEN_CODEC_SETS: [&str; 3] = ["ProtocolErrorCode", "RouteEndReason", "HistoryUnavailable"];

fn open_codec_sets(schema: &mut Value) {
    let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
        return;
    };
    for name in OPEN_CODEC_SETS {
        if let Some(def) = defs.get_mut(name) {
            let names: Vec<Value> = match def.get("enum").and_then(Value::as_array) {
                Some(e) => e.clone(),
                None => def
                    .get("oneOf")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|i| i.get("const").cloned())
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            let names: Vec<Value> = names.into_iter().filter(|n| n != "other").collect();
            *def = json!({ "anyOf": [{ "type": "string", "enum": names }, { "type": "string" }] });
        }
    }
}

/// An internally tagged variant is `{ type: "object", properties: { <tag> }, required, $ref }`. A `$ref` with siblings is read differently by tools,
/// so the sibling keywords move into an `allOf` beside the `$ref`.
fn split_ref_siblings(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if map.contains_key("$ref") && map.keys().any(|k| k == "properties" || k == "required")
            {
                let reference = map.remove("$ref").expect("checked");
                let mut rest = Map::new();
                for key in ["type", "properties", "required"] {
                    if let Some(value) = map.remove(key) {
                        rest.insert(key.into(), value);
                    }
                }
                map.insert(
                    "allOf".into(),
                    json!([{ "$ref": reference }, Value::Object(rest)]),
                );
            }
            map.values_mut().for_each(split_ref_siblings);
        }
        Value::Array(items) => items.iter_mut().for_each(split_ref_siblings),
        _ => {}
    }
}

/// The data-carrying sets of this crate that are non-exhaustive: a reader takes an unknown string, or an unknown one-key object with an object value,
/// as `Other` (A.1b, EN-6). The schema says the same, and keeps the checks of the names that it knows.
const OPEN_DATA_SETS: [(&str, &[&str]); 3] = [
    ("CloseReason", CloseReason::NAMES),
    ("PackageStatus", PackageStatus::NAMES),
    ("UpdateState", UpdateState::NAMES),
];

fn open_data_sets(schema: &mut Value) {
    let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
        return;
    };
    for (name, names) in OPEN_DATA_SETS {
        if let Some(def) = defs.get_mut(name) {
            let known = def.take();
            *def = json!({ "anyOf": [
                known,
                { "type": "string", "not": { "enum": names } },
                { "type": "object", "minProperties": 1, "maxProperties": 1,
                  "propertyNames": { "not": { "enum": names } },
                  "additionalProperties": { "type": "object" } }
            ] });
        }
    }
}

fn finish(mut schema: Value) -> Value {
    open_codec_sets(&mut schema);
    open_data_sets(&mut schema);
    split_ref_siblings(&mut schema);
    schema
}

/// The envelope schema: every frame of the control stream, the handshake, `describe`, the watch and event frames, and the verb arguments.
///
/// Clause: HC EN-1 to EN-8, A4 EN-1a, section 2 to 5, 8.
pub fn envelope() -> Value {
    let mut g = generator();
    let _ = g.subschema_for::<ClientFrame>();
    let _ = g.subschema_for::<ServerFrame>();
    let _ = g.subschema_for::<DescribeResult>();
    let _ = g.subschema_for::<ListArgs>();
    let _ = g.subschema_for::<ListResult>();
    let _ = g.subschema_for::<UpdateArgs>();
    let _ = g.subschema_for::<WatchArgs>();
    let _ = g.subschema_for::<WatchOpened>();
    let _ = g.subschema_for::<EventFilter>();
    let _ = g.subschema_for::<NoticePayload>();
    let _ = g.subschema_for::<SessionNotification>();
    let _ = g.subschema_for::<RouteEndedSubject>();
    let _ = g.subschema_for::<RouteEndedPayload>();
    let _ = g.subschema_for::<Limits>();
    let _ = g.subschema_for::<Error>();
    finish(serde_json::to_value(g.root_schema_for::<Frame>()).expect("a schema serializes"))
}

/// The pairing and route-handshake frames, and the ICE answer.
///
/// Clause: HC TB-L2, PR-4 to PR-7, A2 G-30.
pub fn pairing() -> Value {
    let mut g = generator();
    let _ = g.subschema_for::<AuthFrame>();
    let _ = g.subschema_for::<Signal>();
    let _ = g.subschema_for::<IceServers>();
    finish(serde_json::to_value(g.root_schema_for::<AuthFrame>()).expect("a schema serializes"))
}

/// The MCP tool arguments and the `whoami` result.
///
/// Clause: HC MC-2, A2 H-8.
pub fn mcp() -> Value {
    let mut g = generator();
    let _ = g.subschema_for::<ListTool>();
    let _ = g.subschema_for::<GetTool>();
    let _ = g.subschema_for::<CreateTool>();
    let _ = g.subschema_for::<UpdateTool>();
    let _ = g.subschema_for::<DeleteTool>();
    let _ = g.subschema_for::<InvokeTool>();
    finish(serde_json::to_value(g.root_schema_for::<WhoamiResult>()).expect("a schema serializes"))
}

fn def_name(schema: &Schema) -> Option<String> {
    let r = schema.as_object()?.get("$ref")?.as_str()?;
    Some(r.rsplit('/').next()?.to_string())
}

/// The schema of one kind: the row is the root, and the arguments and results of its verbs and actions are in `$defs`.
/// The keywords `x-kind`, `x-schema-version` (section 12, Versioning), `x-verbs` and `x-actions` carry the registry entry; they name the `$defs` entries.
///
/// Clause: HC section 6, A.7, A.8, A.9, A.9b, DS-1.
pub fn kind_schema(spec: &KindSpec) -> Value {
    let mut g = generator();
    let mut verbs = Map::new();
    for verb in spec.verbs {
        let mut o = Map::new();
        o.insert("effect".into(), json!(verb.effect.as_str()));
        if let Some(p) = verb.permission {
            o.insert("permission".into(), json!(p));
        }
        if let Some(f) = verb.args {
            if let Some(n) = def_name(&f(&mut g)) {
                o.insert("args".into(), json!(n));
            }
        }
        if let Some(f) = verb.result {
            if let Some(n) = def_name(&f(&mut g)) {
                o.insert("result".into(), json!(n));
            }
        }
        o.insert("reasons".into(), json!(verb.reasons));
        o.insert("clause".into(), json!(verb.clause));
        verbs.insert(verb.verb.into(), Value::Object(o));
    }
    let mut actions = Map::new();
    for a in spec.actions {
        let mut o = Map::new();
        o.insert("effect".into(), json!(a.effect.as_str()));
        o.insert("durable_operation".into(), json!(a.durable_operation));
        if let Some(p) = a.permission {
            o.insert("permission".into(), json!(p));
        }
        o.insert("reasons".into(), json!(a.reasons));
        if let Some(n) = def_name(&(a.args)(&mut g)) {
            o.insert("args".into(), json!(n));
        }
        if let Some(n) = def_name(&(a.result)(&mut g)) {
            o.insert("result".into(), json!(n));
        }
        o.insert("clause".into(), json!(a.clause));
        actions.insert(a.name.into(), Value::Object(o));
    }
    // The operation kinds of A.7b: the result types are reachable from the schema, and the table names each kind's states and result.
    let mut operation_kinds = Map::new();
    if spec.kind == "operation" {
        for k in OPERATION_KINDS {
            let mut o = Map::new();
            o.insert("states".into(), json!(k.states));
            o.insert("failed_reasons".into(), json!(k.failed_reasons));
            if let Some(f) = k.result {
                let reference = f(&mut g);
                o.insert(
                    "result".into(),
                    json!(def_name(&reference).unwrap_or_default()),
                );
            }
            operation_kinds.insert(k.kind.into(), Value::Object(o));
        }
    }
    let row = (spec.row)(&mut g);
    let row_name = def_name(&row);
    // The generator holds every definition. The root is the row's definition, with the definitions beside it.
    let defs = g.take_definitions(true);
    let mut out = Map::new();
    out.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    if let Some(name) = &row_name {
        if let Some(Value::Object(def)) = defs.get(name).cloned() {
            out.extend(def);
        }
    } else if let Some(o) = row.as_object() {
        out.extend(o.clone());
    }
    out.insert("x-kind".into(), json!(spec.kind));
    out.insert("x-schema-version".into(), json!(spec.schema_version));
    out.insert("x-verbs".into(), Value::Object(verbs));
    out.insert("x-actions".into(), Value::Object(actions));
    if spec.kind == "operation" {
        out.insert("x-operation-kinds".into(), Value::Object(operation_kinds));
    }
    out.insert(
        "$defs".into(),
        Value::Object(
            defs.into_iter()
                .filter(|(k, _)| Some(k) != row_name.as_ref())
                .collect(),
        ),
    );
    finish(Value::Object(out))
}

/// Every schema file as (path under `schemas/hub-client/`, schema).
///
/// Clause: HC section 12, design section 8.
pub fn all() -> Vec<(String, Value)> {
    let mut out = vec![
        ("envelope.schema.json".to_string(), envelope()),
        ("pairing.schema.json".to_string(), pairing()),
        ("mcp.schema.json".to_string(), mcp()),
    ];
    out.extend(
        KINDS
            .iter()
            .map(|k| (format!("kinds/{}.schema.json", k.kind), kind_schema(k))),
    );
    out
}

/// The text of one schema file: pretty JSON and a final newline.
///
/// Clause: design section 8.
pub fn render(schema: &Value) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("a schema serializes");
    text.push('\n');
    text
}

#[allow(dead_code)]
fn _assert_json_schema<T: JsonSchema>() {}
