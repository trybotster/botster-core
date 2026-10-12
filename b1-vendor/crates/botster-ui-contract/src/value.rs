//! Value rules: truthiness, display text and the order compare.
//!
//! Clause: UI-BD-1, UI-BD-4, UI-RZ-4, UI-RZ-4b (compare function), UI-FD-2 (map key and raw text).

use serde_json::Value;
use std::cmp::Ordering;

/// `truthy` is false for `null`, absent, `false`, `0`, `""` and `[]` (UI-BD-1, UI-BD-4).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(_)) => true,
    }
}

/// `eq` compares JSON values exactly. Two absent operands are equal. One absent operand is not equal (UI-BD-1, UI-BD-4).
pub fn eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// The text that a value shows: absent is empty, a string is itself, anything else is its RFC 8785 text (UI-BD-4, UI-FD-2).
pub fn display_text(v: Option<&Value>) -> String {
    match v {
        None => String::new(),
        Some(v) => raw_text(v),
    }
}

fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        _ => 4,
    }
}

/// The value order of `order` (UI-RZ-4b): absent first, then null, bool, number, string, other.
/// Numbers compare as doubles. Strings compare bytewise. Objects and arrays compare equal to each other.
pub fn compare(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(x), Some(y)) => match type_rank(x).cmp(&type_rank(y)) {
            Ordering::Equal => match (x, y) {
                (Value::Bool(p), Value::Bool(q)) => p.cmp(q),
                (Value::Number(p), Value::Number(q)) => {
                    let (p, q) = (p.as_f64().unwrap_or(0.0), q.as_f64().unwrap_or(0.0));
                    p.partial_cmp(&q).unwrap_or(Ordering::Equal)
                }
                (Value::String(p), Value::String(q)) => p.as_bytes().cmp(q.as_bytes()),
                _ => Ordering::Equal,
            },
            other => other,
        },
    }
}

/// The ECMAScript text of a finite double (RFC 8785 number serialization, UI-FD-2): `1.0` and `1` are `1`, `-0` is `0`,
/// and `1e21` has one spelling. The digits come from `ryu-js`, which implements the ECMAScript `Number::toString`
/// rules, including the tie rule for the shortest digits.
pub fn es_number(f: f64) -> String {
    if f == 0.0 {
        return "0".to_string();
    }
    ryu_js::Buffer::new().format_finite(f).to_string()
}

/// The RFC 8785 (JCS) text of a value (UI-FD-2): members in UTF-16 code unit order, ES number text.
pub fn jcs(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.as_f64().map(es_number).unwrap_or_else(|| "null".into()),
        Value::String(s) => serde_json::to_string(s).unwrap_or_default(),
        Value::Array(a) => format!("[{}]", a.iter().map(jcs).collect::<Vec<_>>().join(",")),
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            // UTF-16 code unit order, not UTF-8 byte order (UI-FD-2).
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        jcs(&m[*k])
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// The lookup key of a value in `Field.map` (UI-FD-2): a string is itself; `true`, `false`, `null` and a finite number
/// are their RFC 8785 text. An object or an array is never a key.
pub fn map_key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) | Value::Array(_) => None,
        Value::Number(n) => n.as_f64().filter(|f| f.is_finite()).map(es_number),
        other => Some(jcs(other)),
    }
}

/// The raw display text of a value (UI-FD-2): a string shows as itself, any other value as its RFC 8785 text.
pub fn raw_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => jcs(other),
    }
}
