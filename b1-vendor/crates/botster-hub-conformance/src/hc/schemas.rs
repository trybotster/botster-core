//! The schema resolver of the HC suite: the generated envelope and kind schemas, compiled when first used (design 5.4, 8).

use botster_conformance::{Direction, FrameSchema, SchemaResolver};
use botster_hub_contract::schema;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// The generated schemas, by name. The envelope defs are `hc/<Name>`; a kind's defs are `kind/<kind>/<Name>`; the whole kind row is `kind/<kind>`.
fn sources() -> &'static BTreeMap<String, (Value, Option<String>)> {
    static SOURCES: OnceLock<BTreeMap<String, (Value, Option<String>)>> = OnceLock::new();
    SOURCES.get_or_init(|| {
        let mut m = BTreeMap::new();
        fn add_defs(m: &mut BTreeMap<String, (Value, Option<String>)>, prefix: &str, root: &Value) {
            let defs = root["$defs"].clone();
            if let Some(d) = defs.as_object() {
                for name in d.keys() {
                    m.insert(
                        format!("{prefix}/{name}"),
                        (
                            json!({"$ref": format!("#/$defs/{name}"), "$defs": defs}),
                            None,
                        ),
                    );
                }
            }
        }
        add_defs(&mut m, "hc", &schema::envelope());
        for (path, v) in schema::all() {
            if let Some(kind) = path
                .strip_prefix("kinds/")
                .and_then(|p| p.strip_suffix(".schema.json"))
            {
                add_defs(&mut m, &format!("kind/{kind}"), &v);
                m.insert(format!("kind/{kind}"), (v, None));
            }
        }
        m
    })
}

/// Validates control-stream frames, and (for a route frame) the route schemas of the codec.
pub struct HcSchemas {
    compiled: Mutex<BTreeMap<String, std::sync::Arc<jsonschema::Validator>>>,
    route: crate::route::RouteSchemas,
}

impl Default for HcSchemas {
    fn default() -> Self {
        HcSchemas {
            compiled: Mutex::new(BTreeMap::new()),
            route: crate::route::RouteSchemas,
        }
    }
}

impl HcSchemas {
    fn get(&self, name: &str) -> Result<std::sync::Arc<jsonschema::Validator>, String> {
        if let Some(v) = self.compiled.lock().expect("lock").get(name) {
            return Ok(v.clone());
        }
        let schema = sources()
            .get(name)
            .ok_or_else(|| format!("no schema registered for '{name}'"))?
            .0
            .clone();
        let v = std::sync::Arc::new(jsonschema::validator_for(&schema).map_err(|e| e.to_string())?);
        self.compiled
            .lock()
            .expect("lock")
            .insert(name.to_string(), v.clone());
        Ok(v)
    }

    fn is_route(name: &str) -> bool {
        matches!(
            name,
            "to_worker"
                | "to-worker"
                | "to_client"
                | "to-client"
                | "route/to_client"
                | "route/to_worker"
        )
    }
}

impl SchemaResolver for HcSchemas {
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String> {
        if Self::is_route(schema_ref) {
            return self.route.validate(schema_ref, value);
        }
        self.get(schema_ref)?
            .validate(value)
            .map_err(|e| e.to_string())
    }

    fn violations(&self, schema_ref: &str, value: &Value) -> Result<Vec<String>, String> {
        if Self::is_route(schema_ref) {
            return self.route.violations(schema_ref, value);
        }
        Ok(self
            .get(schema_ref)?
            .iter_errors(value)
            .map(|e| e.schema_path().to_string())
            .collect())
    }

    /// A control frame has a `type`; a route frame in its decoded form has none (design 5.4). The `type` also tells a Hub frame from a client frame, so a
    /// `client` transcript, where the runner plays the Hub, sends server frames.
    fn frame_schema(&self, direction: Direction, frame: &Value) -> FrameSchema {
        let Some(ty) = frame.get("type").and_then(Value::as_str) else {
            return self.route.frame_schema(direction, frame);
        };
        let server = matches!(ty, "welcome" | "res" | "watch" | "event" | "close");
        let client = matches!(ty, "hello" | "req" | "cancel");
        let _ = direction;
        match (server, client) {
            (true, _) => FrameSchema::Ref("hc/ServerFrame".into()),
            (_, true) => FrameSchema::Ref("hc/ClientFrame".into()),
            // An unknown type: the schema of the direction decides (a received unknown type fails ServerFrame).
            _ => FrameSchema::Ref(
                if direction == Direction::Sent {
                    "hc/ClientFrame"
                } else {
                    "hc/ServerFrame"
                }
                .into(),
            ),
        }
    }
}
