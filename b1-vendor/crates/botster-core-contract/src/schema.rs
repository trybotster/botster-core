//! The generated JSON Schemas of the types that a transcript writes or matches (design section 8).
//! `cargo xtask codegen` writes them to `schemas/core/`.

use crate::prelude::*;
use serde_json::{json, Value};

fn of<T: schemars::JsonSchema>() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).expect("a schema serializes");
    open_sets(&mut schema);
    schema
}

/// The sets that are non-exhaustive: a receiver accepts a value that it does not know (Core 3, A2-3).
const OPEN_SETS: [&str; 11] = [
    "ExitCause",
    "LostReason",
    "StartFailReason",
    "UnsupportedReason",
    "UnsupportedWhat",
    "RouteCloseReason",
    "ServiceExitCause",
    "LaneEndReason",
    "SendError",
    "LostKind",
    "ProtocolErrorCode",
];

/// The open sets whose reader takes a bare name only (the codec's `ProtocolErrorCode`, Codec A.1).
const STRING_ONLY: [&str; 1] = ["ProtocolErrorCode"];

/// The variant names that a schema def of an enum lists: string constants and the keys of one-key object variants.
fn known_names(def: &Value, out: &mut Vec<String>) {
    if let Value::Object(map) = def {
        if let Some(Value::String(c)) = map.get("const") {
            out.push(c.clone());
        }
        if let Some(Value::Array(e)) = map.get("enum") {
            out.extend(e.iter().filter_map(|v| v.as_str().map(String::from)));
        }
        if let Some(Value::Object(props)) = map.get("properties") {
            if props.len() == 1 {
                out.extend(props.keys().cloned());
            }
        }
        for key in ["oneOf", "anyOf"] {
            if let Some(Value::Array(items)) = map.get(key) {
                items.iter().for_each(|i| known_names(i, out));
            }
        }
    }
}

/// An open set accepts a value of a name that it does not know, in the forms that its reader accepts, and nothing else.
fn open_sets(schema: &mut Value) {
    let Some(defs) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
        return;
    };
    for name in OPEN_SETS {
        if let Some(def) = defs.get_mut(name) {
            let known = def.take();
            let mut names = Vec::new();
            known_names(&known, &mut names);
            let mut options = vec![known, json!({ "type": "string", "not": { "enum": names } })];
            if !STRING_ONLY.contains(&name) {
                options.push(
                    json!({ "type": "object", "minProperties": 1, "maxProperties": 1,
                  "propertyNames": { "not": { "enum": names } },
                  "additionalProperties": { "type": "object" } }),
                );
            }
            *def = json!({ "anyOf": options });
        }
    }
}

/// Every Core schema as (file stem, schema).
///
/// Clause: Core 3, Core A2-1, Core 6.2, Core 9B.
pub fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("op", of::<Op>()),
        ("op-result", of::<OpResult>()),
        ("event", of::<Event>()),
        ("core-error", of::<CoreError>()),
        ("core-limits", of::<CoreLimits>()),
        ("open-config", of::<OpenConfig>()),
        ("spawn-request", of::<SpawnRequest>()),
        ("session-record", of::<SessionRecord>()),
        ("terminal-state", of::<TerminalState>()),
        ("input-result", of::<InputResult>()),
        ("cancel-result", of::<CancelResult>()),
        ("attach-options", of::<AttachOptions>()),
        ("attach-result", of::<AttachResult>()),
        ("service-limits", of::<ServiceLimits>()),
        ("features", of::<Features>()),
        ("facts", of::<Facts>()),
        ("pump-report", of::<PumpReport>()),
        ("wake", of::<Wake>()),
    ]
}

/// The text of one schema file: pretty JSON and a final newline.
///
/// Clause: Core 3.
pub fn render(schema: &Value) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("a schema serializes");
    text.push('\n');
    text
}
