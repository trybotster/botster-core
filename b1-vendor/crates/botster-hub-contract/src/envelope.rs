//! The envelope (HC section 3): client frames `hello`, `req`, `cancel` and server frames `welcome`, `res`, `watch`, `event`, `close`.

use crate::error::Error;
use crate::events::EventFrame;
use crate::handshake::{CloseReason, Hello, Welcome};
use crate::watch::WatchFrame;
use crate::wire::{open_enum, opt_no_null, RequestId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

open_enum! {
    /// The request verbs, a closed set (P2). An unknown verb gets `unsupported` and the connection stays open (EN-6).
    ///
    /// Clause: HC P2, EN-6, VB-7.
    pub enum Verb {
        Describe = "describe",
        List = "list",
        Get = "get",
        Watch = "watch",
        Unwatch = "unwatch",
        Create = "create",
        Update = "update",
        Delete = "delete",
        Invoke = "invoke",
    }
}

/// A request frame. `action` is required when `verb` is `invoke` and absent for every other verb (A4 EN-1a); that rule is checked by `Req::check`.
///
/// Clause: HC EN-1, A4 EN-1a, EN-8a.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(extend("allOf" = [{
    "if": { "properties": { "verb": { "const": "invoke" } }, "required": ["verb"] },
    "then": { "required": ["action"] },
    "else": { "not": { "required": ["action"] } }
}]))]
pub struct Req {
    pub id: RequestId,
    pub verb: Verb,
    pub kind: String,
    /// A resource id where the verb needs one (for `unwatch`, the watch id).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub target: Option<String>,
    /// The action name, the key of `describe` `actions` (DS-1). Only with `verb: invoke`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub action: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub args: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub idempotency_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub operation_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub descriptor_id: Option<String>,
}

/// Why a request frame breaks a rule that the frame types cannot state.
///
/// Clause: A4 EN-1a.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReqViolation {
    /// `verb` is `invoke` and `action` is absent.
    ActionRequired,
    /// `action` is present on a verb other than `invoke`.
    ActionForbidden,
}

impl Req {
    /// The EN-1a rule: `action` exactly when `verb` is `invoke`. A violation answers `invalid_arguments`.
    ///
    /// Clause: A4 EN-1a.
    pub fn check(&self) -> Result<(), ReqViolation> {
        match (self.verb == Verb::Invoke, self.action.is_some()) {
            (true, false) => Err(ReqViolation::ActionRequired),
            (false, true) => Err(ReqViolation::ActionForbidden),
            _ => Ok(()),
        }
    }
}

/// `cancel { id }`: best effort, and the request still gets one `res`.
///
/// Clause: HC EN-1, EN-3b.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cancel {
    pub id: RequestId,
}

/// The frames of a client. An unknown `type` is a protocol error and closes the connection (EN-6).
///
/// Clause: HC EN-1, EN-6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientFrame {
    Hello(Hello),
    Req(Req),
    Cancel(Cancel),
}

/// The outcome of a `res`: `ok` or `err`, exactly one.
///
/// Clause: HC EN-2, EN-3.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Ok { ok: Value },
    Err { err: Error },
}

/// The wire form of `Res`: `ok` and `err` are optional members, and exactly one is present.
#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Res", extend("oneOf" = [
    { "required": ["ok"], "properties": { "ok": { "not": { "type": "null" } } }, "not": { "required": ["err"] } },
    { "required": ["err"], "not": { "required": ["ok"] } }
]))]
struct ResWire {
    id: RequestId,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    ok: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    err: Option<Error>,
}

/// `res { id, ok } | res { id, err }`. A frame with both or neither is refused.
///
/// Clause: HC EN-2, EN-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ResWire", into = "ResWire")]
#[schemars(with = "ResWire")]
pub struct Res {
    pub id: RequestId,
    pub outcome: Outcome,
}

impl TryFrom<ResWire> for Res {
    type Error = String;
    fn try_from(w: ResWire) -> Result<Self, String> {
        let outcome = match (w.ok, w.err) {
            (Some(ok), None) if !ok.is_null() => Outcome::Ok { ok },
            (None, Some(err)) => Outcome::Err { err },
            _ => return Err("a res has exactly one of ok and err".into()),
        };
        Ok(Res { id: w.id, outcome })
    }
}

impl From<Res> for ResWire {
    fn from(r: Res) -> Self {
        match r.outcome {
            Outcome::Ok { ok } => ResWire {
                id: r.id,
                ok: Some(ok),
                err: None,
            },
            Outcome::Err { err } => ResWire {
                id: r.id,
                ok: None,
                err: Some(err),
            },
        }
    }
}

/// `close { reason }`.
///
/// Clause: HC HS-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Close {
    pub reason: CloseReason,
}

/// The frames of the Hub. An unknown `type` is a protocol error (EN-6).
///
/// Clause: HC EN-2, EN-6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ServerFrame {
    Welcome(Welcome),
    Res(Res),
    Watch(WatchFrame),
    Event(EventFrame),
    Close(Close),
}

/// Any frame of the control stream, in either direction.
///
/// Clause: HC EN-1, EN-2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum Frame {
    Client(ClientFrame),
    Server(ServerFrame),
}

/// The arguments of `list`.
///
/// Clause: HC VB-1, A2 VB-1 (H-32).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub filter: Option<crate::wire::OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub cursor: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub limit: Option<u64>,
}

/// The `ok` value of `list`.
///
/// Clause: HC VB-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListResult {
    pub items: Vec<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub next_cursor: Option<String>,
}

/// The arguments of `update`: an RFC 7396 merge patch and the optional `if_rev`.
/// `req` has no field for `if_rev`, so it travels in `args` (pending steward ruling H2).
///
/// Clause: HC VB-4, A.1.
// pending steward ruling H2: `if_rev` travels inside `args`, beside `patch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateArgs {
    pub patch: crate::wire::OpenObject,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub if_rev: Option<String>,
}

/// The arguments of `watch` on a kind: the optional filter. For `watch(event, …)` the filter is an `EventFilter`.
///
/// Clause: HC WT-1, EV-2.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct WatchArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub filter: Option<crate::wire::OpenObject>,
}

/// The `ok` value of `watch`. For an event stream the id is the `stream_id` (H-14).
///
/// Clause: HC WT-1, A2 H-14.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchOpened {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub watch_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub stream_id: Option<String>,
}
