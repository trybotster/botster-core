//! The matcher engine (design 5.2).
//!
//! A matcher is a JSON value that a received value must match.
//! - A literal matches an equal value. An object matcher matches when every key matches, and extra keys in the received object are allowed.
//! - `"$name"` binds a variable at its first use. A later use must be equal.
//! - `{"$any": true}`, `{"$absent": true}`, `{"$type": "<name>"}`, `{"$one_of": [..]}`, `{"$bytes_hex": "<hex>"}` and `{"$schema": "<ref>"}` are the control matchers.
//! - Predicates (HP hp-calls-a, design 5.2): `{"$not": m}` matches when `m` does not match (it binds nothing); `{"$ne": "$var"}` matches a present value that differs from the
//!   bound variable (an unbound variable is a transcript error); `{"$starts_with": "text"}` and `{"$contains": "text"}` match a string; `{"$between": [lo, hi]}` matches an integer with `lo <= v <= hi`;
//!   `{"$b64": m}` decodes a base64 string and matches `m` against the decoded text (invalid UTF-8 becomes U+FFFD), so `{"$b64": {"$contains": "x"}}` reads the bytes that a tail carries;
//!   `{"$array": {"each": m, "min": n, "max": n}}` matches an array of any length in `min..=max` (both optional) whose every element matches `m`.
//!
//! A binary value is carried in received JSON as `{"$bytes_hex": "<lowercase hex>"}`: see [`bytes_value`].
//! A control matcher is an object with exactly one key, and that key starts with `$`.
//! The closed list of `$type` names: `string`, `number`, `integer`, `u64`, `boolean`, `null`, `object`, `array`, `u64_decimal` (a string of decimal digits without a leading zero, at most `u64::MAX`).

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// Variable bindings of one transcript run. The key has no leading `$`.
pub type Bindings = BTreeMap<String, Value>;

/// Why a match did not succeed.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchFailure {
    /// The value does not match. `path` is a JSON pointer into the received value.
    Mismatch {
        path: String,
        expected: Value,
        received: Option<Value>,
        reason: String,
    },
    /// The matcher itself is malformed. This is a transcript error, never a subject failure.
    BadMatcher { path: String, reason: String },
}

impl MatchFailure {
    /// The matcher part that failed, as JSON, for the report.
    pub fn expected(&self) -> Value {
        match self {
            MatchFailure::Mismatch { expected, .. } => expected.clone(),
            MatchFailure::BadMatcher { .. } => Value::Null,
        }
    }

    /// A one-line description, for the report.
    pub fn describe(&self) -> String {
        match self {
            MatchFailure::Mismatch { path, reason, .. } => format!("at '{path}': {reason}"),
            MatchFailure::BadMatcher { path, reason } => {
                format!("bad matcher at '{path}': {reason}")
            }
        }
    }
}

/// Which way a frame travels (design 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    /// A frame that the transcript sends to the subject.
    Sent,
    /// A frame that the subject sends back.
    Received,
}

/// The schema that governs a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameSchema {
    /// The frame must validate against this schema reference.
    Ref(String),
    /// The value is not a schema-checked frame (for example a Core event). The engine does not check it.
    NotFramed,
}

/// Resolves schema references (design 5.2, 5.4). A suite supplies one, built from the generated schemas.
pub trait SchemaResolver {
    /// `Err` carries the reason that the value does not validate, or that the reference is unknown.
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String>;

    /// The schema paths at which the value fails, for example `/required` or `/properties/outer/required`. Empty means that the value validates.
    ///
    /// `Err` means that the resolver cannot say (an unknown schema, or no support). It never means that the value violates the schema.
    /// The default supports nothing, so a resolver that cannot list violations never lets a negative frame pass as checked.
    fn violations(&self, schema_ref: &str, _value: &Value) -> Result<Vec<String>, String> {
        Err(format!(
            "this resolver cannot list violations of '{schema_ref}'"
        ))
    }

    /// The schema of a frame that is sent or received. The default says that nothing is a frame.
    fn frame_schema(&self, _direction: Direction, _frame: &Value) -> FrameSchema {
        FrameSchema::NotFramed
    }
}

/// A resolver with no schemas. Every `$schema` matcher fails.
pub struct NoSchemas;

impl SchemaResolver for NoSchemas {
    fn validate(&self, schema_ref: &str, _value: &Value) -> Result<(), String> {
        Err(format!("no schema registered for '{schema_ref}'"))
    }
}

/// A resolver over JSON Schemas compiled with the `jsonschema` crate.
#[derive(Default)]
pub struct SchemaRegistry {
    validators: BTreeMap<String, jsonschema::Validator>,
    frames: BTreeMap<Direction, String>,
}

impl SchemaRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a schema under a reference name. `Err` carries the compile error.
    pub fn add(&mut self, schema_ref: &str, schema: &Value) -> Result<(), String> {
        let validator = jsonschema::validator_for(schema).map_err(|e| e.to_string())?;
        self.validators.insert(schema_ref.to_string(), validator);
        Ok(())
    }

    /// Declares that every frame in `direction` is checked against the registered schema `schema_ref`.
    pub fn set_frame_schema(&mut self, direction: Direction, schema_ref: &str) {
        self.frames.insert(direction, schema_ref.to_string());
    }
}

impl SchemaResolver for SchemaRegistry {
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String> {
        let validator = self
            .validators
            .get(schema_ref)
            .ok_or_else(|| format!("no schema registered for '{schema_ref}'"))?;
        validator.validate(value).map_err(|e| e.to_string())
    }

    fn violations(&self, schema_ref: &str, value: &Value) -> Result<Vec<String>, String> {
        let validator = self
            .validators
            .get(schema_ref)
            .ok_or_else(|| format!("no schema registered for '{schema_ref}'"))?;
        Ok(validator
            .iter_errors(value)
            .map(|e| e.schema_path().to_string())
            .collect())
    }

    fn frame_schema(&self, direction: Direction, _frame: &Value) -> FrameSchema {
        match self.frames.get(&direction) {
            Some(r) => FrameSchema::Ref(r.clone()),
            None => FrameSchema::NotFramed,
        }
    }
}

/// The received form of binary data: `{"$bytes_hex": "<lowercase hex>"}`.
pub fn bytes_value(bytes: &[u8]) -> Value {
    // Two table lookups per byte, not one `format!`: received data can be large.
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        hex.push(char::from(DIGITS[usize::from(b >> 4)]));
        hex.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    json!({ "$bytes_hex": hex })
}

/// True when the string is a variable reference: `$` and an identifier.
pub fn is_variable(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('$') else {
        return false;
    };
    let mut chars = rest.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Replaces every string that is a bound variable (design 5.4: variables are substituted before a send).
///
/// `Err` carries the name of the first unbound variable. Object keys are not substituted.
pub fn substitute(value: &Value, bindings: &Bindings) -> Result<Value, String> {
    Ok(match value {
        Value::String(s) if is_variable(s) => {
            bindings.get(&s[1..]).cloned().ok_or_else(|| s.clone())?
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| substitute(v, bindings))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), substitute(v, bindings)?);
            }
            Value::Object(out)
        }
        other => other.clone(),
    })
}

/// Matches `received` against `matcher`. `None` means that the value is missing (an absent object key).
///
/// On success the new bindings are committed to `bindings`. On failure `bindings` is unchanged.
pub fn matches(
    matcher: &Value,
    received: Option<&Value>,
    bindings: &mut Bindings,
    schemas: &dyn SchemaResolver,
) -> Result<(), MatchFailure> {
    let mut scratch = bindings.clone();
    walk(matcher, received, &mut scratch, schemas, "")?;
    *bindings = scratch;
    Ok(())
}

fn mismatch(path: &str, matcher: &Value, received: Option<&Value>, reason: &str) -> MatchFailure {
    MatchFailure::Mismatch {
        path: path.to_string(),
        expected: matcher.clone(),
        received: received.cloned(),
        reason: reason.to_string(),
    }
}

fn bad(path: &str, reason: &str) -> MatchFailure {
    MatchFailure::BadMatcher {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

fn pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn walk(
    matcher: &Value,
    received: Option<&Value>,
    binds: &mut Bindings,
    schemas: &dyn SchemaResolver,
    path: &str,
) -> Result<(), MatchFailure> {
    if let Value::Object(map) = matcher {
        if map.keys().any(|k| k.starts_with('$')) {
            return control(map, matcher, received, binds, schemas, path);
        }
    }
    let Some(value) = received else {
        return Err(mismatch(path, matcher, received, "value is missing"));
    };
    match matcher {
        Value::String(s) if is_variable(s) => {
            let name = &s[1..];
            match binds.get(name) {
                Some(bound) if bound == value => Ok(()),
                Some(_) => Err(mismatch(
                    path,
                    matcher,
                    received,
                    "differs from the bound variable",
                )),
                None => {
                    binds.insert(name.to_string(), value.clone());
                    Ok(())
                }
            }
        }
        Value::Object(map) => {
            let Value::Object(got) = value else {
                return Err(mismatch(path, matcher, received, "expected an object"));
            };
            for (key, sub) in map {
                let child = format!("{path}/{}", pointer_token(key));
                walk(sub, got.get(key), binds, schemas, &child)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            let Value::Array(got) = value else {
                return Err(mismatch(path, matcher, received, "expected an array"));
            };
            if items.len() != got.len() {
                return Err(mismatch(path, matcher, received, "array length differs"));
            }
            for (i, (sub, item)) in items.iter().zip(got).enumerate() {
                walk(sub, Some(item), binds, schemas, &format!("{path}/{i}"))?;
            }
            Ok(())
        }
        literal => {
            if literal == value {
                Ok(())
            } else {
                Err(mismatch(path, matcher, received, "literal differs"))
            }
        }
    }
}

/// `$set`: gives each matcher its own element, in any order. A binding that a matcher makes holds for the matchers after it.
fn assign_set(
    matchers: &[Value],
    items: &[Value],
    at: usize,
    used: &mut [bool],
    binds: &mut Bindings,
    schemas: &dyn SchemaResolver,
    path: &str,
) -> Result<bool, MatchFailure> {
    if at == matchers.len() {
        return Ok(true);
    }
    for (j, item) in items.iter().enumerate() {
        if used[j] {
            continue;
        }
        let mut attempt = binds.clone();
        match walk(&matchers[at], Some(item), &mut attempt, schemas, path) {
            Ok(()) => {}
            Err(bad @ MatchFailure::BadMatcher { .. }) => return Err(bad),
            Err(MatchFailure::Mismatch { .. }) => continue,
        }
        used[j] = true;
        if assign_set(matchers, items, at + 1, used, &mut attempt, schemas, path)? {
            *binds = attempt;
            return Ok(true);
        }
        used[j] = false;
    }
    Ok(false)
}

fn control(
    map: &Map<String, Value>,
    matcher: &Value,
    received: Option<&Value>,
    binds: &mut Bindings,
    schemas: &dyn SchemaResolver,
    path: &str,
) -> Result<(), MatchFailure> {
    if map.len() != 1 {
        return Err(bad(path, "a control matcher has exactly one key"));
    }
    let (key, arg) = map.iter().next().expect("one key");
    match key.as_str() {
        "$any" => {
            if arg != &Value::Bool(true) {
                return Err(bad(path, "$any takes true"));
            }
            received
                .map(|_| ())
                .ok_or_else(|| mismatch(path, matcher, received, "value is missing"))
        }
        "$absent" => {
            if arg != &Value::Bool(true) {
                return Err(bad(path, "$absent takes true"));
            }
            match received {
                None => Ok(()),
                Some(_) => Err(mismatch(path, matcher, received, "value is present")),
            }
        }
        "$type" => {
            let Value::String(name) = arg else {
                return Err(bad(path, "$type takes a type name"));
            };
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            match type_matches(name, value) {
                Some(true) => Ok(()),
                Some(false) => Err(mismatch(path, matcher, received, "wrong type")),
                None => Err(bad(path, "unknown $type name")),
            }
        }
        "$one_of" => {
            let Value::Array(options) = arg else {
                return Err(bad(path, "$one_of takes an array"));
            };
            if options.is_empty() {
                return Err(bad(path, "$one_of takes at least one matcher"));
            }
            for option in options {
                let mut attempt = binds.clone();
                match walk(option, received, &mut attempt, schemas, path) {
                    Ok(()) => {
                        *binds = attempt;
                        return Ok(());
                    }
                    Err(bad @ MatchFailure::BadMatcher { .. }) => return Err(bad),
                    Err(MatchFailure::Mismatch { .. }) => {}
                }
            }
            Err(mismatch(path, matcher, received, "no alternative matches"))
        }
        "$not" => {
            let mut attempt = binds.clone();
            match walk(arg, received, &mut attempt, schemas, path) {
                Ok(()) => Err(mismatch(
                    path,
                    matcher,
                    received,
                    "the negated matcher matches",
                )),
                Err(bad @ MatchFailure::BadMatcher { .. }) => Err(bad),
                Err(MatchFailure::Mismatch { .. }) => Ok(()),
            }
        }
        "$ne" => {
            let Value::String(name) = arg else {
                return Err(bad(path, "$ne takes a variable name"));
            };
            if !is_variable(name) {
                return Err(bad(path, "$ne takes a variable name"));
            }
            let bound = binds
                .get(&name[1..])
                .ok_or_else(|| bad(path, "$ne names an unbound variable"))?;
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            if value == bound {
                Err(mismatch(
                    path,
                    matcher,
                    received,
                    "equals the bound variable",
                ))
            } else {
                Ok(())
            }
        }
        "$starts_with" | "$contains" => {
            let Value::String(text) = arg else {
                return Err(bad(path, "the string predicates take a string"));
            };
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            let Some(got) = value.as_str() else {
                return Err(mismatch(path, matcher, received, "not a string"));
            };
            let hit = if key == "$starts_with" {
                got.starts_with(text.as_str())
            } else {
                got.contains(text.as_str())
            };
            if hit {
                Ok(())
            } else {
                Err(mismatch(
                    path,
                    matcher,
                    received,
                    "the string does not match",
                ))
            }
        }
        "$b64" => {
            use base64::Engine;
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            let Some(text) = value.as_str() else {
                return Err(mismatch(path, matcher, received, "not a string"));
            };
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(text) else {
                return Err(mismatch(path, matcher, received, "not base64"));
            };
            let decoded = Value::String(String::from_utf8_lossy(&bytes).into_owned());
            walk(arg, Some(&decoded), binds, schemas, path)
        }
        "$array" => {
            let Value::Object(spec) = arg else {
                return Err(bad(path, "$array takes {each, min?, max?}"));
            };
            let each = spec.get("each");
            let bound = |k: &str| spec.get(k).map(|v| v.as_u64());
            if spec
                .keys()
                .any(|k| !matches!(k.as_str(), "each" | "min" | "max"))
                || matches!(bound("min"), Some(None))
                || matches!(bound("max"), Some(None))
            {
                return Err(bad(
                    path,
                    "$array takes {each, min?, max?} with integer bounds",
                ));
            }
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            let Value::Array(items) = value else {
                return Err(mismatch(path, matcher, received, "expected an array"));
            };
            let n = items.len() as u64;
            if bound("min").flatten().is_some_and(|m| n < m)
                || bound("max").flatten().is_some_and(|m| n > m)
            {
                return Err(mismatch(
                    path,
                    matcher,
                    received,
                    "array length is out of range",
                ));
            }
            if let Some(each) = each {
                for (i, item) in items.iter().enumerate() {
                    walk(each, Some(item), binds, schemas, &format!("{path}/{i}"))?;
                }
            }
            Ok(())
        }
        "$set" => {
            let Value::Array(matchers) = arg else {
                return Err(bad(path, "$set takes an array of matchers"));
            };
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            let Value::Array(items) = value else {
                return Err(mismatch(path, matcher, received, "expected an array"));
            };
            if items.len() != matchers.len() {
                return Err(mismatch(
                    path,
                    matcher,
                    received,
                    "the array has another length than the set",
                ));
            }
            let mut used = vec![false; items.len()];
            if assign_set(matchers, items, 0, &mut used, binds, schemas, path)? {
                Ok(())
            } else {
                Err(mismatch(
                    path,
                    matcher,
                    received,
                    "the elements are not the set, each once, in any order",
                ))
            }
        }
        "$between" => {
            let range = arg
                .as_array()
                .filter(|a| a.len() == 2)
                .and_then(|a| Some((a[0].as_i64()?, a[1].as_i64()?)));
            let Some((lo, hi)) = range else {
                return Err(bad(path, "$between takes [lo, hi] integers"));
            };
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            match value.as_i64() {
                Some(v) if lo <= v && v <= hi => Ok(()),
                Some(_) => Err(mismatch(path, matcher, received, "outside the range")),
                None => Err(mismatch(path, matcher, received, "not an integer")),
            }
        }
        "$bytes_hex" => {
            let Value::String(hex) = arg else {
                return Err(bad(path, "$bytes_hex takes a hex string"));
            };
            if hex.len() % 2 != 0 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(bad(path, "$bytes_hex takes an even number of hex digits"));
            }
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            let got = value
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("$bytes_hex"))
                .and_then(Value::as_str);
            match got {
                Some(got) if got.eq_ignore_ascii_case(hex) => Ok(()),
                Some(_) => Err(mismatch(path, matcher, received, "bytes differ")),
                None => Err(mismatch(path, matcher, received, "not a binary value")),
            }
        }
        "$schema" => {
            let Value::String(name) = arg else {
                return Err(bad(path, "$schema takes a schema reference"));
            };
            let value =
                received.ok_or_else(|| mismatch(path, matcher, received, "value is missing"))?;
            schemas
                .validate(name, value)
                .map_err(|reason| mismatch(path, matcher, received, &reason))
        }
        _ => Err(bad(path, "unknown control matcher")),
    }
}

fn type_matches(name: &str, value: &Value) -> Option<bool> {
    Some(match name {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "u64" => value.is_u64(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "u64_decimal" => value.as_str().is_some_and(|s| {
            !s.is_empty()
                && s.bytes().all(|b| b.is_ascii_digit())
                && (s == "0" || !s.starts_with('0'))
                && s.parse::<u64>().is_ok()
        }),
        _ => return None,
    })
}

#[cfg(test)]
mod hex_tests {
    use super::*;

    /// The received form's hex is byte-identical to the `format!` encoding for each of the 256 byte values, alone and together.
    #[test]
    fn bytes_value_is_the_format_encoding_for_every_byte() {
        let format =
            |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
        let all: Vec<u8> = (0..=255).collect();
        for b in &all {
            assert_eq!(bytes_value(&[*b]), json!({ "$bytes_hex": format(&[*b]) }));
        }
        assert_eq!(bytes_value(&all), json!({ "$bytes_hex": format(&all) }));
        assert_eq!(bytes_value(&[]), json!({ "$bytes_hex": "" }));
    }
}
