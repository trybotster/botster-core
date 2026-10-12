//! The frame schemas of the HP runner: one schema per frame type, from the generated `plugin-frames.schema.json` (HP Appendix B, design 5.4).

use botster_conformance::{Direction, FrameSchema, SchemaRegistry, SchemaResolver};
use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};

/// A transcript frame is `{"lane": "<name>", "type": "<name>", "payload": {..}}`. This resolver checks the payload against the schema of its type and the lane against the
/// lanes of that type (HP section 2, R-4).
///
/// A frame schema compiles on its first use. A test runs in its own process and uses a few frame types, so compiling every schema up front
/// would cost each test the cost of all of them. A schema that is used compiles to the same validator as before.
pub struct HpSchemas {
    doc: std::sync::OnceLock<Value>,
    /// One slot per frame type, in the order of `FRAMES`.
    slots: Vec<std::sync::OnceLock<SchemaRegistry>>,
}

impl Default for HpSchemas {
    fn default() -> Self {
        Self::new()
    }
}

impl HpSchemas {
    pub fn new() -> Self {
        HpSchemas {
            doc: std::sync::OnceLock::new(),
            slots: FRAMES.iter().map(|_| std::sync::OnceLock::new()).collect(),
        }
    }

    /// The registry of `name`; for a name that is not a frame type, an empty one, whose answer is the error of an unregistered schema.
    fn registry_or_empty(&self, name: &str) -> &SchemaRegistry {
        static EMPTY: std::sync::OnceLock<SchemaRegistry> = std::sync::OnceLock::new();
        self.registry(name)
            .unwrap_or_else(|| EMPTY.get_or_init(SchemaRegistry::new))
    }

    /// The registry that holds the schema of `name`, compiled now if it was not before. `None` for a name that is not a frame type.
    fn registry(&self, name: &str) -> Option<&SchemaRegistry> {
        let i = FRAMES.iter().position(|f| f.name == name)?;
        Some(self.slots[i].get_or_init(|| {
            let doc = self.doc.get_or_init(|| {
                let text = artifact_files()
                    .into_iter()
                    .find(|(name, _)| *name == "plugin-frames.schema.json")
                    .map(|(_, t)| t)
                    .expect("the frames artifact exists");
                serde_json::from_str(&text).expect("the frames artifact parses")
            });
            // The defs name is the Rust type: `Result` is `ResultFrame` (HP Appendix B).
            let def = if name == "Result" { "ResultFrame" } else { name };
            let schema = json!({"$schema": doc["$schema"], "$ref": format!("#/$defs/{def}"), "$defs": doc["$defs"]});
            let mut registry = SchemaRegistry::new();
            registry
                .add(name, &schema)
                .expect("a frame schema compiles");
            registry
        }))
    }
}

fn lane_ok(frame_type: &str, lane: &str) -> bool {
    FRAMES
        .iter()
        .find(|f| f.name == frame_type)
        .is_some_and(|f| f.lanes.iter().any(|l| crate::lane_name(*l) == lane))
}

impl SchemaResolver for HpSchemas {
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String> {
        let lane = value.get("lane").and_then(Value::as_str).unwrap_or("");
        if !lane_ok(schema_ref, lane) {
            return Err(format!(
                "a {schema_ref} frame does not travel on lane '{lane}'"
            ));
        }
        self.registry_or_empty(schema_ref)
            .validate(schema_ref, value.get("payload").unwrap_or(&Value::Null))
    }

    fn violations(&self, schema_ref: &str, value: &Value) -> Result<Vec<String>, String> {
        self.registry_or_empty(schema_ref)
            .violations(schema_ref, value.get("payload").unwrap_or(value))
    }

    fn frame_schema(&self, _direction: Direction, frame: &Value) -> FrameSchema {
        match frame.get("type").and_then(Value::as_str) {
            Some(name) if frame.get("payload").is_some() => FrameSchema::Ref(name.to_string()),
            _ => FrameSchema::NotFramed,
        }
    }
}
