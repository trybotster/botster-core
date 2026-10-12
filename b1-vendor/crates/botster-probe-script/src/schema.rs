//! The generated JSON Schema of a probe script. `cargo xtask codegen` writes it to `schemas/core/probe-script.schema.json`.

use crate::Script;
use serde_json::Value;

/// The schema of [`Script`] with the opaque `fake` part (Core A5-1).
pub fn schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Script)).expect("a schema serializes")
}

/// The text of a schema file: pretty JSON and a newline.
pub fn render(schema: &Value) -> String {
    let mut text = serde_json::to_string_pretty(schema).expect("a schema serializes");
    text.push('\n');
    text
}
