//! Author ids, node ids and draft keys.
//!
//! Clause: UI-ID-1, UI-ID-2, UI-ID-3, UI-ID-4, UI-ID-5, UI-RZ-2.
//!
//! A node id is an opaque string. One function, [`child_id`], builds every node id from a parent string and a
//! segment. A length prefix on both parts means that no separator can collide with a key (UI-ID-3).
//! The length is the number of UTF-8 bytes. [`split_id`] is the inverse.

use serde_json::Value;

/// The fixed prefix of a built node id (UI-ID-3).
pub const ID_PREFIX: &str = "botster-ui-descendant-v1:";

/// The role segments of generated nodes, and the words that an author id never equals (UI-ID-1, UI-RZ-2, UI-SA-5).
pub const ROLE_WORDS: [&str; 11] = [
    "label",
    "value",
    "widget",
    "help",
    "error",
    "container",
    "item",
    "cell",
    "action",
    "empty",
    "warning",
];

/// Builds the node id of a child from the id of its parent and one segment (UI-ID-2, UI-ID-3).
pub fn child_id(parent: &str, segment: &str) -> String {
    format!(
        "{ID_PREFIX}{}:{parent}{}:{segment}",
        parent.len(),
        segment.len()
    )
}

/// The inverse of [`child_id`] (UI-ID-3). `None` when the text is not a built id.
pub fn split_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix(ID_PREFIX)?;
    let (parent, rest) = take_chunk(rest)?;
    let (segment, rest) = take_chunk(rest)?;
    rest.is_empty().then_some((parent, segment))
}

/// Reads `<len>:<bytes>` from the front of `text`.
fn take_chunk(text: &str) -> Option<(&str, &str)> {
    let colon = text.find(':')?;
    let digits = &text[..colon];
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    let len: usize = digits.parse().ok()?;
    let body = &text[colon + 1..];
    if len > body.len() || !body.is_char_boundary(len) {
        return None;
    }
    Some(body.split_at(len))
}

/// The draft key of a form field: the form's node id plus the `field_id` (UI-ID-5).
pub fn draft_key(form_node_id: &str, field_id: &str) -> String {
    child_id(form_node_id, field_id)
}

/// The node id of the field that a form error names, inside a form (UI-ID-4, UI-ID-5).
pub fn field_error_target(form_node_id: &str, field_id: &str) -> String {
    draft_key(form_node_id, field_id)
}

/// True when `id` is a well-formed author id: not empty, only `[a-z0-9_.-]` (UI-ID-1).
pub fn is_author_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-')
        })
}

/// True when `id` is one of the reserved role words (UI-ID-1).
pub fn is_role_word(id: &str) -> bool {
    ROLE_WORDS.contains(&id)
}

/// The key of a row: its own `id` field (UI-ID-3). A string is the key itself. Another JSON value is its JSON text.
pub fn row_key(row: &Value) -> Option<String> {
    match row.get("id")? {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}
