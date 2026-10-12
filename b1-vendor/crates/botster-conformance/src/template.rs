//! Templates of a transcript (design 5.1: fixtures live in `conformance/<suite>/fixtures/`). Shared by the HP, Lua binding and CLI runners.
//!
//! A step or a setup may hold `{"$ref": "<name>", ..overrides}`. The runner replaces it before the engine runs, with the template of `fixtures/frames.json`,
//! deep-merged with the overrides. So a `Ready` or a `Hello` with every required field is one line in a transcript.

use serde_json::{Map, Value};

fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                // A present key merges: two objects merge key by key, and anything else is replaced by the override (the last arm).
                match b.get_mut(k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

/// Expands every `$ref` in `value`. `Err` names a template that does not exist.
pub fn expand(value: &Value, templates: &Value) -> Result<Value, String> {
    Ok(match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| expand(v, templates))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if k != "$ref" {
                    out.insert(k.clone(), expand(v, templates)?);
                }
            }
            match map.get("$ref").and_then(Value::as_str) {
                Some(name) => {
                    let mut base = templates
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("no template '{name}' in fixtures/frames.json"))?;
                    merge(&mut base, &Value::Object(out));
                    base
                }
                None => Value::Object(out),
            }
        }
        other => other.clone(),
    })
}
