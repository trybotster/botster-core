//! The operation record and the state tables per kind.
//!
//! Clause: HP CA-16, CA-17, SW-2, SW-4, NT-3, NT-4, PX-2, PC-1, A2-4, A2-22, A2-32.

use crate::common::{Effect, SessionRef};
use crate::result::ErrorBody;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The record kinds this contract defines (HP CA-16). `kind` is non-exhaustive: a reader treats an unknown kind as opaque and waits for `terminal: true`.
pub const OPERATION_KINDS: &[&str] = &[
    "spawn",
    "safe_write",
    "http",
    "ws",
    "process",
    "plugin_call",
];

/// The states this contract defines per kind (HP A2-4) with the states that end the operation.
/// `state` stays non-exhaustive: every record carries `terminal`.
pub struct StateTable {
    pub kind: &'static str,
    pub states: &'static [&'static str],
    pub terminal: &'static [&'static str],
}

/// The closed state tables of A2-4 (spawn, http, ws, process, plugin_call) and SW-2 (safe_write).
pub const STATE_TABLES: &[StateTable] = &[
    StateTable {
        kind: "spawn",
        states: &["starting", "installed", "failed", "cancelled", "unknown"],
        terminal: &["installed", "failed", "cancelled", "unknown"],
    },
    StateTable {
        kind: "safe_write",
        states: &[
            "queued",
            "typing",
            "delivered",
            "abandoned",
            "replaced",
            "cancelled",
            "failed",
            "unknown",
        ],
        terminal: &[
            "delivered",
            "abandoned",
            "replaced",
            "cancelled",
            "failed",
            "unknown",
        ],
    },
    StateTable {
        kind: "http",
        states: &["queued", "done", "failed", "cancelled", "unknown"],
        terminal: &["done", "failed", "cancelled", "unknown"],
    },
    StateTable {
        kind: "ws",
        states: &["queued", "open", "failed", "cancelled", "unknown"],
        terminal: &["open", "failed", "cancelled", "unknown"],
    },
    StateTable {
        kind: "process",
        states: &[
            "queued",
            "running",
            "done",
            "failed",
            "cancelled",
            "unknown",
        ],
        terminal: &["done", "failed", "cancelled", "unknown"],
    },
    StateTable {
        kind: "plugin_call",
        states: &[
            "queued",
            "dispatched",
            "done",
            "failed",
            "cancelled",
            "timed_out",
            "unknown",
        ],
        terminal: &["done", "failed", "cancelled", "timed_out", "unknown"],
    },
];

/// `spawn` `failed.reason` values (HP A2-4).
pub const SPAWN_FAILED_REASONS: &[&str] = &[
    "start_failed",
    "worker_lost",
    "worktree_failed",
    "session_type_invalid",
    "target_disabled",
];

/// `safe_write` `abandoned.reason` values (HP SW-2).
pub const SAFE_WRITE_ABANDONED_REASONS: &[&str] = &[
    "human_input",
    "session_ended",
    "not_safe_in_time",
    "no_echo",
    "caller_cancelled",
    "guard_stale",
];

/// `safe_write` `cancelled.reason` values (HP SW-2).
pub const SAFE_WRITE_CANCELLED_REASONS: &[&str] = &["caller_cancelled", "plugin_unloaded"];

/// The probe text that `sessions.safe_write` types first and erases once its echo is confirmed (HP SW-1 to SW-9, HC MC-4).
pub const SAFE_WRITE_PROBE: &str = "zx";

/// `safe_write` `failed.reason` values (HP A2-22).
pub const SAFE_WRITE_FAILED_REASONS: &[&str] = &["core_refused", "session_removed"];

/// `safe_write` `unknown.reason` values (HP A2-22). `hub_restarted` is also the answer for a lost record of every kind (A2-4).
pub const SAFE_WRITE_UNKNOWN_REASONS: &[&str] =
    &["hub_restarted", "link_lost", "cancel_after_text"];

/// Whether a safe write leaves residue (HP SW-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Residue {
    None,
    Possible,
}

/// Which output streams were truncated (HP PX-2, A2-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Truncated {
    pub stdout: bool,
    pub stderr: bool,
}

/// The fields of an operation record (HP CA-16, `operations.get` in Appendix A, A2-4): `{operation_id, kind, state, terminal, ...}` with the per-kind fields.
/// `kind` and `state` are strings and are non-exhaustive (CA-16); see `STATE_TABLES`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperationFields {
    pub operation_id: String,
    pub kind: String,
    pub state: String,
    pub terminal: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub session: Option<SessionRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub worktree: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub probe_written: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub residue: Option<Residue>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub text_written_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub status: Option<u16>,
    /// A list of `[name, value]` pairs (HP A2-17).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub headers: Option<Vec<[String; 2]>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub body_b64: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub ws_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub exit_code: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub signal: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub stdout_tail_b64: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub stderr_tail_b64: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub truncated: Option<Truncated>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub duration_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub result: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub error: Option<ErrorBody>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub effect: Option<Effect>,
}

/// The fields that a known kind and state require (HP CA-14, CA-16, SW-2, SW-4, NT-3, NT-4, PX-2, PC-1, A2-4).
/// A row names (kind, state, required fields). A kind or state outside the table is opaque: a reader waits for `terminal: true` (CA-16).
pub const REQUIRED_FIELDS: &[(&str, &str, &[&str])] = &[
    ("spawn", "failed", &["reason"]),
    ("safe_write", "abandoned", &["reason"]),
    ("safe_write", "cancelled", &["reason"]),
    ("safe_write", "failed", &["reason"]),
    ("safe_write", "unknown", &["reason"]),
    ("http", "done", &["status", "headers", "body_b64", "effect"]),
    ("http", "failed", &["error", "effect"]),
    ("ws", "open", &["ws_id"]),
    ("ws", "failed", &["error"]),
    (
        "process",
        "done",
        &[
            "stdout_tail_b64",
            "stderr_tail_b64",
            "truncated",
            "duration_ms",
        ],
    ),
    ("process", "failed", &["error"]),
    ("plugin_call", "done", &["result"]),
    ("plugin_call", "failed", &["error"]),
];

impl OperationFields {
    fn has(&self, field: &str) -> bool {
        match field {
            "reason" => self.reason.is_some(),
            "status" => self.status.is_some(),
            "headers" => self.headers.is_some(),
            "body_b64" => self.body_b64.is_some(),
            "error" => self.error.is_some(),
            "ws_id" => self.ws_id.is_some(),
            "stdout_tail_b64" => self.stdout_tail_b64.is_some(),
            "stderr_tail_b64" => self.stderr_tail_b64.is_some(),
            "truncated" => self.truncated.is_some(),
            "duration_ms" => self.duration_ms.is_some(),
            "result" => self.result.is_some(),
            "effect" => self.effect.is_some(),
            _ => true,
        }
    }

    /// Checks the rules of a known kind and state. An unknown kind or state passes (CA-16).
    pub fn check(&self) -> Result<(), String> {
        if let Some(t) = STATE_TABLES.iter().find(|t| t.kind == self.kind) {
            if t.states.contains(&self.state.as_str()) {
                let want = t.terminal.contains(&self.state.as_str());
                if self.terminal != want {
                    return Err(format!(
                        "a {} record in state {} has terminal: {want}",
                        self.kind, self.state
                    ));
                }
            }
        }
        for (kind, state, fields) in REQUIRED_FIELDS {
            if self.kind == *kind && self.state == *state {
                if let Some(f) = fields.iter().find(|f| !self.has(f)) {
                    return Err(format!("a {kind} record in state {state} needs `{f}`"));
                }
            }
        }
        if self.kind == "safe_write" && self.probe_written == Some(true) {
            if self.residue.is_none() {
                return Err(
                    "a safe_write record that wrote the probe needs `residue` (SW-4)".into(),
                );
            }
            if self.text_written_bytes.is_none() {
                return Err(
                    "a safe_write record that wrote the probe needs `text_written_bytes` (SW-4)"
                        .into(),
                );
            }
        }
        Ok(())
    }
}

/// The operation record (HP CA-16, CA-17). A record of a known kind and state has the fields that `REQUIRED_FIELDS` names; an unknown kind or state is accepted as opaque (CA-16).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OperationRecord(pub OperationFields);

impl std::ops::Deref for OperationRecord {
    type Target = OperationFields;
    fn deref(&self) -> &OperationFields {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OperationRecord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let f = OperationFields::deserialize(d)?;
        f.check().map_err(serde::de::Error::custom)?;
        Ok(OperationRecord(f))
    }
}

impl JsonSchema for OperationRecord {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "OperationRecord".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut v = serde_json::to_value(g.subschema_for::<OperationFields>())
            .expect("a schema serializes");
        let mut rules: Vec<Value> = Vec::new();
        for t in STATE_TABLES {
            let terminal: Vec<&str> = t.terminal.to_vec();
            let open: Vec<&str> = t
                .states
                .iter()
                .copied()
                .filter(|s| !t.terminal.contains(s))
                .collect();
            rules.push(serde_json::json!({"if": {"properties": {"kind": {"const": t.kind}, "state": {"enum": terminal}}, "required": ["kind", "state"]}, "then": {"properties": {"terminal": {"const": true}}}}));
            rules.push(serde_json::json!({"if": {"properties": {"kind": {"const": t.kind}, "state": {"enum": open}}, "required": ["kind", "state"]}, "then": {"properties": {"terminal": {"const": false}}}}));
        }
        for (kind, state, fields) in REQUIRED_FIELDS {
            rules.push(serde_json::json!({"if": {"properties": {"kind": {"const": kind}, "state": {"const": state}}, "required": ["kind", "state"]}, "then": {"required": fields}}));
        }
        rules.push(serde_json::json!({"if": {"properties": {"kind": {"const": "safe_write"}, "probe_written": {"const": true}}, "required": ["kind", "probe_written"]}, "then": {"required": ["residue", "text_written_bytes"]}}));
        if let Some(o) = v.as_object_mut() {
            if o.contains_key("$ref") {
                // A referenced subschema: wrap it so the rules can sit beside it.
                let r = o.remove("$ref").expect("checked");
                o.insert(
                    "allOf".into(),
                    Value::Array(vec![serde_json::json!({"$ref": r})]),
                );
            }
            match o.get_mut("allOf") {
                Some(Value::Array(a)) => a.extend(rules),
                _ => {
                    o.insert("allOf".into(), Value::Array(rules));
                }
            }
        }
        schemars::Schema::try_from(v).expect("an object schema")
    }
}
