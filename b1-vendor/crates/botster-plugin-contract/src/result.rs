//! `Result`, `ErrorKind` and the error body.
//!
//! Clause: HP section 3, section 14, ER-1 to ER-5, IV-2, A2-2.

use crate::common::{Effect, OpenObject};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use std::borrow::Cow;

/// The closed set of error kinds (HP section 14, ER-1). A kind a reader does not know is `Unknown` and acts as `Failed` (ER-1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    InvalidRequest,
    CapabilityDenied,
    NotFound,
    Conflict,
    QuotaExceeded,
    ResponseTooLarge,
    Backpressured,
    TimedOut,
    Cancelled,
    Unavailable,
    Failed,
    /// A kind this version does not know (ER-1: treated as `Failed`).
    Unknown(String),
}

impl ErrorKind {
    /// The published kinds, in the order of HP section 14.
    pub const ALL: [ErrorKind; 11] = [
        ErrorKind::InvalidRequest,
        ErrorKind::CapabilityDenied,
        ErrorKind::NotFound,
        ErrorKind::Conflict,
        ErrorKind::QuotaExceeded,
        ErrorKind::ResponseTooLarge,
        ErrorKind::Backpressured,
        ErrorKind::TimedOut,
        ErrorKind::Cancelled,
        ErrorKind::Unavailable,
        ErrorKind::Failed,
    ];

    pub fn as_str(&self) -> &str {
        match self {
            ErrorKind::InvalidRequest => "invalid_request",
            ErrorKind::CapabilityDenied => "capability_denied",
            ErrorKind::NotFound => "not_found",
            ErrorKind::Conflict => "conflict",
            ErrorKind::QuotaExceeded => "quota_exceeded",
            ErrorKind::ResponseTooLarge => "response_too_large",
            ErrorKind::Backpressured => "backpressured",
            ErrorKind::TimedOut => "timed_out",
            ErrorKind::Cancelled => "cancelled",
            ErrorKind::Unavailable => "unavailable",
            ErrorKind::Failed => "failed",
            ErrorKind::Unknown(s) => s,
        }
    }

    pub fn parse(s: &str) -> ErrorKind {
        ErrorKind::ALL
            .iter()
            .find(|k| k.as_str() == s)
            .cloned()
            .unwrap_or_else(|| ErrorKind::Unknown(s.to_string()))
    }

    /// The kind a host acts on (ER-1): an unknown kind is `Failed`.
    pub fn effective(&self) -> ErrorKind {
        match self {
            ErrorKind::Unknown(_) => ErrorKind::Failed,
            k => k.clone(),
        }
    }
}

impl Serialize for ErrorKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ErrorKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(ErrorKind::parse(&String::deserialize(d)?))
    }
}

impl JsonSchema for ErrorKind {
    fn schema_name() -> Cow<'static, str> {
        "ErrorKind".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let kinds: Vec<&str> = ErrorKind::ALL.iter().map(|k| k.as_str()).collect();
        json_schema!({
            "description": "HP section 14, ER-1: the closed set of kinds. A reader treats an unknown string as `failed`.",
            "type": "string",
            "enum": kinds
        })
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The `detail` of an error (HP section 14, ER-3). `code` is the stable specific code; `reason` is the closed reason of a state refusal (A2-2).
/// Other keys carry the descriptive fields that individual clauses name (for example `names`, `op_index`, `status`); the contract states them per clause.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Detail {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub code: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

/// One provider of an interface that PD-6 resolution considered: its package key and the `interface_version` that its `provides`
/// declares. The names are the lockfile's (PM-3).
///
/// Clause: HP PD-6, PM-3, ruling R-40.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FoundProvider {
    pub provider: String,
    pub interface_version: String,
}

/// The `detail` of a PD-6 resolution failure: its `code` and the fields that PD-6 names for that code. [`ResolutionFailure::kind`]
/// gives the kind that it travels under. Install and update refuse with it before any plugin code runs.
///
/// Clause: HP PD-6, PD-9, ruling R-40.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResolutionFailure {
    /// No installed or chosen package provides the required interface: `failed`.
    RequirementMissing { interface: String, range: String },
    /// No provider of the interface is in `range`: `conflict`. `found` lists each provider that resolution considered, none in
    /// range, so it is never empty (R-40). Its order is not fixed.
    RequirementConflict {
        interface: String,
        range: String,
        #[serde(deserialize_with = "at_least::<1, _, _>")]
        #[schemars(length(min = 1))]
        found: Vec<FoundProvider>,
    },
    /// Several providers are in range and the caller chose none (`bindings`): `conflict`. "Several" means at least two.
    AmbiguousProvider {
        interface: String,
        #[serde(deserialize_with = "at_least::<2, _, _>")]
        #[schemars(length(min = 2))]
        candidates: Vec<String>,
    },
}

fn at_least<'de, const N: usize, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v = Vec::<T>::deserialize(d)?;
    if v.len() < N {
        return Err(serde::de::Error::invalid_length(
            v.len(),
            &format!("at least {N} entries").as_str(),
        ));
    }
    Ok(v)
}

impl ResolutionFailure {
    /// The error kind of the refusal (PD-6).
    pub fn kind(&self) -> ErrorKind {
        match self {
            ResolutionFailure::RequirementMissing { .. } => ErrorKind::Failed,
            ResolutionFailure::RequirementConflict { .. }
            | ResolutionFailure::AmbiguousProvider { .. } => ErrorKind::Conflict,
        }
    }

    /// The error `detail` that carries this failure.
    pub fn to_detail(&self) -> Detail {
        serde_json::from_value(serde_json::to_value(self).expect("a resolution failure serializes"))
            .expect("a resolution failure is a detail")
    }

    /// The resolution failure in `detail`, or an error if `detail` is not one of the three PD-6 forms.
    pub fn from_detail(detail: &Detail) -> Result<Self, serde_json::Error> {
        serde_json::from_value(serde_json::to_value(detail)?)
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The body of a failure (HP section 14, ER-2, ER-5). `effect` is set where the caller sees the failure; a plugin's own `Result` may omit it, because the Hub adds it (HP section 2, ER-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorBody {
    pub kind: ErrorKind,
    pub message: String,
    pub retryable: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub effect: Option<Effect>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub detail: Option<Detail>,
}

impl ErrorBody {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        ErrorBody {
            kind,
            message: message.into(),
            retryable: false,
            effect: None,
            detail: None,
        }
    }
    pub fn with_code(mut self, code: &str) -> Self {
        self.detail.get_or_insert_with(Detail::default).code = Some(code.to_string());
        self
    }
    pub fn with_effect(mut self, effect: Effect) -> Self {
        self.effect = Some(effect);
        self
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `Result = { ok: true, value } | { ok: false, error }` (HP section 3, IV-2). Every call and every handler returns it.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginResult {
    Ok(#[cfg_attr(test, strategy(crate::props::value()))] Value),
    Err(ErrorBody),
}

impl Serialize for PluginResult {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PluginResult::Ok(v) => json!({"ok": true, "value": v}).serialize(s),
            PluginResult::Err(e) => {
                let mut m = serde_json::Map::new();
                m.insert("ok".into(), Value::Bool(false));
                m.insert(
                    "error".into(),
                    serde_json::to_value(e).map_err(serde::ser::Error::custom)?,
                );
                Value::Object(m).serialize(s)
            }
        }
    }
}

impl<'de> Deserialize<'de> for PluginResult {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut m = serde_json::Map::<String, Value>::deserialize(d)?;
        let ok = match m.remove("ok") {
            Some(Value::Bool(b)) => b,
            _ => return Err(Error::custom("result needs a boolean `ok`")),
        };
        if ok {
            if m.contains_key("error") {
                return Err(Error::custom("ok result must not carry `error`"));
            }
            match m.remove("value") {
                Some(v) => Ok(PluginResult::Ok(v)),
                None => Err(Error::custom("ok result needs `value`")),
            }
        } else {
            if m.contains_key("value") {
                return Err(Error::custom("failed result must not carry `value`"));
            }
            let e = m
                .remove("error")
                .ok_or_else(|| Error::custom("failed result needs `error`"))?;
            serde_json::from_value(e)
                .map(PluginResult::Err)
                .map_err(Error::custom)
        }
    }
}

impl JsonSchema for PluginResult {
    fn schema_name() -> Cow<'static, str> {
        "PluginResult".into()
    }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let err = g.subschema_for::<ErrorBody>();
        json_schema!({
            "description": "HP section 3, section 14. Descriptive extension fields are accepted and ignored (HP section 18, VR-3); an `error` in a successful result and a `value` in a failed result are refused.",
            "oneOf": [
                {"type": "object", "required": ["ok", "value"], "properties": {"ok": {"const": true}, "value": true}, "not": {"required": ["error"]}},
                {"type": "object", "required": ["ok", "error"], "properties": {"ok": {"const": false}, "error": err}, "not": {"required": ["value"]}}
            ]
        })
    }
}
