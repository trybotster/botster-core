//! The generated JSON Schemas of the route frames (design section 8). `cargo xtask codegen` writes them to `schemas/hub-client/route/`.
//!
//! `to-client` and `to-worker` describe the decoded form of every frame (a `frame` tag, the body fields, and binary payloads as `{"$bytes_hex": ..}`).
//! The other schemas describe one JSON body, as it travels on the wire.

use crate::frames::*;
use crate::input::Input;
use crate::types::*;
use serde_json::{json, Value};

fn of<T: schemars::JsonSchema>() -> Value {
    let schema = serde_json::to_value(schemars::schema_for!(T)).expect("a schema serializes");
    tighten(schema)
}

/// The reason sets that are non-exhaustive on the wire: a receiver accepts a value that it does not know (Codec A.2).
const OPEN_SETS: [&str; 11] = [
    "MouseTracking",
    "MouseEncoding",
    "HistoryUnavailable",
    "HistoryIncomplete",
    "AttachFailedReason",
    "ResyncReason",
    "CloseReason",
    "RouteEndReason",
    "RefusalReason",
    "QueryKind",
    "ProtocolErrorCode",
];

/// Applies the rules that the derive cannot state: no null in a route body (A.1), the open reason sets (A.2),
/// and the field constraints of Core 5.1A (A.5).
fn tighten(mut schema: Value) -> Value {
    strip_null(&mut schema);
    let defs = schema.get_mut("$defs").and_then(Value::as_object_mut);
    if let Some(defs) = defs {
        for name in OPEN_SETS {
            if let Some(def) = defs.get_mut(name) {
                let known = def.take();
                *def = json!({ "anyOf": [known, { "type": "string" }, { "type": "object", "minProperties": 1, "maxProperties": 1 }] });
            }
        }
        if let Some(key) = defs.get_mut("Key") {
            if let Some(c) = key.pointer_mut("/oneOf/1/properties/char") {
                *c = json!({ "type": "string", "minLength": 1, "maxLength": 1 });
            }
        }
        // A.5: the position of a window is two u16 xterm wire values.
        if let Some(variants) = defs
            .get_mut("QueryReply")
            .and_then(|q| q.get_mut("oneOf"))
            .and_then(Value::as_array_mut)
        {
            for v in variants {
                for axis in ["x", "y"] {
                    if let Some(a) =
                        v.pointer_mut(&format!("/properties/position/properties/{axis}"))
                    {
                        a["maximum"] = json!(65535);
                    }
                }
            }
        }
        if let Some(input) = defs.get_mut("Input") {
            constrain_input(input);
        }
    }
    if schema.get("title").and_then(Value::as_str) == Some("Input") {
        constrain_input(&mut schema);
    }
    schema
}

fn strip_null(v: &mut Value) {
    match v {
        Value::Object(map) => {
            if let Some(Value::Array(options)) = map.get("anyOf") {
                if options.len() == 2 && options.contains(&json!({ "type": "null" })) {
                    let keep = options
                        .iter()
                        .find(|o| **o != json!({ "type": "null" }))
                        .cloned()
                        .unwrap_or(Value::Null);
                    map.remove("anyOf");
                    if let Value::Object(inner) = keep {
                        map.extend(inner);
                    }
                }
            }
            if let Some(Value::Array(types)) = map.get_mut("type") {
                types.retain(|t| t != "null");
                if types.len() == 1 {
                    let only = types.remove(0);
                    map.insert("type".into(), only);
                }
            }
            map.values_mut().for_each(strip_null);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_null),
        _ => {}
    }
}

/// Core 5.1A field constraints that need more than a type (Codec A.5).
fn constrain_input(input: &mut Value) {
    let Some(variants) = input.get_mut("oneOf").and_then(Value::as_array_mut) else {
        return;
    };
    let wheel = ["wheel_up", "wheel_down", "wheel_left", "wheel_right"];
    for v in variants {
        let kind = v
            .pointer("/properties/kind/const")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let Some(props) = v.get_mut("properties").and_then(Value::as_object_mut) else {
            continue;
        };
        let one_char = json!({ "type": "object", "properties": { "char": { "type": "string", "minLength": 1, "maxLength": 1 } }, "required": ["char"], "additionalProperties": false });
        match kind.as_str() {
            "key" => {
                props.insert("shifted_key".into(), one_char.clone());
                props.insert("base_layout_key".into(), one_char);
                if let Some(mods) = props.get_mut("mods").and_then(Value::as_object_mut) {
                    mods.insert("uniqueItems".into(), json!(true));
                }
                v["allOf"] = json!([{ "if": { "required": ["shifted_key"] }, "then": { "required": ["mods"], "properties": { "mods": { "contains": { "const": "shift" } } } } }]);
            }
            "mouse" => {
                props.insert("mods".into(), json!({ "type": "array", "items": { "enum": ["shift", "alt", "ctrl"] }, "uniqueItems": true, "default": [] }));
                v["allOf"] = json!([
                    { "if": { "properties": { "button": { "enum": wheel } }, "required": ["button"] },
                      "then": { "properties": { "action": { "const": "wheel" } } },
                      "else": { "properties": { "action": { "not": { "const": "wheel" } } } } },
                    { "if": { "required": ["notches"] },
                      "then": { "required": ["action"], "properties": { "action": { "const": "wheel" }, "notches": { "minimum": 1 } } } }
                ]);
            }
            _ => {}
        }
    }
}

/// Every route schema as (file stem, schema).
///
/// Clause: Codec A.1, A.4, A.5.
pub fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("to-client", of::<ToClient>()),
        ("to-worker", of::<ToWorker>()),
        ("attached", of::<Attached>()),
        ("baseline_begin", of::<BaselineBegin>()),
        ("baseline_end", of::<BaselineEnd>()),
        ("modes", of::<Modes>()),
        ("resync", of::<Resync>()),
        ("input_refused", of::<InputRefused>()),
        ("input_done", of::<InputDone>()),
        ("terminal_query", of::<TerminalQuery>()),
        ("output_dropped", of::<OutputDropped>()),
        ("route_closed", of::<RouteClosed>()),
        ("input", of::<Input>()),
        ("mode_flags", of::<ModeFlags>()),
        ("route_limits", of::<RouteLimits>()),
    ]
}

/// The text of one schema file: pretty JSON and a final newline.
///
/// Clause: Codec A.1, A.4, A.5.
pub fn render(schema: &Value) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("a schema serializes");
    text.push('\n');
    text
}

/// The decoded-form schema of the frames that the worker sends.
///
/// Clause: Codec A.4.
pub fn to_client() -> Value {
    of::<ToClient>()
}

/// The decoded-form schema of the frames that the client sends.
///
/// Clause: Codec A.5.
pub fn to_worker() -> Value {
    of::<ToWorker>()
}
