//! Argument and result types of every call of HP Appendix A (section 24), with the shapes and enums of A2.
//!
//! Clause: HP section 24, A2-1 to A2-38. Arguments refuse unknown fields (HP CF-1, CA-1). Results accept unknown fields (HP section 18).

use crate::common::const_str;
use crate::common::{present_or_null, ConstTrue, OpenObject, SessionRef, WireId, WireU64};
use crate::entities::{PublishStatus, PublishType};
use crate::operations::OperationRecord;
use crate::rows::{SessionRow, SessionTypeRow, SpawnTargetRow, WorktreeRow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// `{}`: a call with no arguments (HP CF-1: any argument is `invalid_request`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// `{}`: an empty result value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NoValue {}

/// `{session}` (HP SA-1, SO-1, SO-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionArgs {
    pub session: SessionRef,
}

/// The reference `{target_id | worktree_id, relative?}` that replaces an absolute path (HP PX-1, CA-19). Exactly one of the two ids is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CwdRef {
    Target(CwdTarget),
    Worktree(CwdWorktree),
}

/// A `cwd` inside a spawn target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CwdTarget {
    pub target_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub relative: Option<String>,
}

/// A `cwd` inside a worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CwdWorktree {
    pub worktree_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub relative: Option<String>,
}

/// A terminal cursor (HP SA-1, SO-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
}

// ---- admission answers (HP CA-14, CA-16, A2-32) ----

const_str!(
    /// `kind: "spawn"` (HP A2-32).
    KindSpawn, "spawn"
);
const_str!(
    /// `kind: "safe_write"` (HP A2-32).
    KindSafeWrite, "safe_write"
);
const_str!(
    /// `kind: "http"` (HP NT-3).
    KindHttp, "http"
);
const_str!(
    /// `kind: "ws"` (HP NT-4).
    KindWs, "ws"
);
const_str!(
    /// `kind: "process"` (HP PX-1).
    KindProcess, "process"
);
const_str!(
    /// `kind: "plugin_call"` (HP PC-1).
    KindPluginCall, "plugin_call"
);
const_str!(
    /// `state: "starting"` (HP CA-14).
    StateStarting, "starting"
);
const_str!(
    /// `state: "queued"` (HP SW-1, NT-3, NT-4, PX-1, PC-1).
    StateQueued, "queued"
);

/// `terminal: false`: every admission answer is not terminal (HP CA-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotTerminal;

impl Serialize for NotTerminal {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(false)
    }
}

impl<'de> Deserialize<'de> for NotTerminal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? {
            Err(serde::de::Error::custom(
                "an admission answer carries terminal: false",
            ))
        } else {
            Ok(NotTerminal)
        }
    }
}

impl JsonSchema for NotTerminal {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "NotTerminal".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"const": false})
    }
}

/// The answer of a call that launches a session: `session_types.spawn`, `ensure_worktree_and_spawn`, `sessions.restart` and `sessions.create_command` (HP CA-14, CA-19, A2-32). `worktree` is present when the Hub creates or reuses one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnAdmission {
    pub operation_id: String,
    pub kind: KindSpawn,
    pub session: SessionRef,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub worktree: Option<crate::rows::WorktreeRow>,
    pub state: StateStarting,
    pub terminal: NotTerminal,
}

macro_rules! queued_admission {
    ($(#[$m:meta])* $name:ident, $kind:ident) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        pub struct $name {
            pub operation_id: String,
            pub kind: $kind,
            pub state: StateQueued,
            pub terminal: NotTerminal,
        }
    };
}

queued_admission!(
    /// The answer of `sessions.safe_write` (HP SW-1).
    SafeWriteAdmission, KindSafeWrite
);
queued_admission!(
    /// The answer of `http.request` (HP NT-3).
    HttpAdmission, KindHttp
);
queued_admission!(
    /// The answer of `ws.open` (HP NT-4).
    WsAdmission, KindWs
);
queued_admission!(
    /// The answer of `process.run` (HP PX-1).
    ProcessAdmission, KindProcess
);
queued_admission!(
    /// The answer of `plugins.call` (HP PC-1).
    PluginCallAdmission, KindPluginCall
);

// ---- log, identity, config ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// `log.write` (HP CA-9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogWriteArgs {
    pub level: LogLevel,
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub fields: Option<OpenObject>,
}

/// `{seq, dropped_before?}` (HP CA-9, A2-20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LogWriteResult {
    /// A counter: a JSON number up to 2^53 and a decimal string above (HP WF-3).
    pub seq: WireU64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub dropped_before: Option<WireU64>,
}

/// `{hub_id, display_name}` (HP Appendix A `hub.identity`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HubIdentity {
    pub hub_id: String,
    pub display_name: String,
}

/// A `config.get` diagnostic (HP A2-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigDiagnostic {
    pub name: String,
    pub code: String,
    pub message: String,
}

/// `config.get` result (HP CF-1): `values` maps each name to the plain value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConfigValues {
    pub values: OpenObject,
    pub missing_required: Vec<String>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

// ---- store ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreGetArgs {
    pub collection: String,
    pub id: String,
}

/// `{doc, revision}`; `revision` is an opaque string for equality only (HP ST-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoreDoc {
    pub doc: Value,
    pub revision: String,
}

/// A half-open range `[from, to)` on the next index field (HP ST-4, A2-14).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreRange {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub from: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub to: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Asc,
    Desc,
}

/// `store.query` (HP ST-4, A2-14). `limit` is required; `equals` is an array for the first N index fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreQueryArgs {
    pub collection: String,
    pub index: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub equals: Option<Vec<Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub range: Option<StoreRange>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub order: Option<Order>,
    pub limit: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub max_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoreRow {
    pub id: String,
    pub doc: Value,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StoreQueryResult {
    pub docs: Vec<StoreRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cursor: Option<String>,
}

/// `store.put` (HP ST-2): create with `expected_revision` absent, update with the current one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorePutArgs {
    pub collection: String,
    pub id: String,
    pub doc: Value,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoreRevision {
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreDeleteArgs {
    pub collection: String,
    pub id: String,
    pub expected_revision: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchOpKind {
    Put,
    Delete,
}

/// One op of `store.batch` (HP ST-3, A2-9): a `delete` needs `expected_revision`; a `put` without it creates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreBatchOp {
    pub op: BatchOpKind,
    pub collection: String,
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub doc: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreBatchArgs {
    pub ops: Vec<StoreBatchOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoreBatchEntry {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoreBatchResult {
    pub results: Vec<StoreBatchEntry>,
}

/// `store.watch` (HP ST-6, A2-14, A2-53).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StoreWatchArgs {
    pub collection: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub index: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub equals: Option<Vec<Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub handler_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchId {
    pub watch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnwatchArgs {
    pub watch_id: String,
}

// ---- events and timers ----

/// `events.emit` (HP EV-1). `owner`, `source`, `instance` and `at` are Hub-stamped and refused if supplied: `deny_unknown_fields` refuses them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventsEmitArgs {
    pub name: String,
    pub payload: Value,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subject: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmitResult {
    /// `"accepted"` (HP Appendix A). Other ingress statuses travel in `error.detail.status` (EV-3).
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimersArmArgs {
    pub timer_id: String,
    pub delay_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub handler_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimersCancelArgs {
    pub timer_id: String,
}

// ---- sessions ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionsListArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub hub_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub owner: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionsListResult {
    pub sessions: Vec<SessionRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StopDecision {
    pub decision: String,
}

/// `sessions.update` (HP SA-1, A2-21): `metadata` replaces `metadata[<plugin_key>]` whole, and `null` removes the namespace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionsUpdateArgs {
    pub session: SessionRef,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub task: Option<String>,
    #[serde(
        default,
        deserialize_with = "present_or_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(schema_with = "crate::schema::object_or_null")]
    pub metadata: Option<Option<OpenObject>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReadScreenResult {
    pub text: String,
    pub cursor: Cursor,
    /// Core's opaque token, for equality only (HP SO-1).
    pub model_rev: WireId,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub history_unavailable: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalSize {
    pub rows: u32,
    pub cols: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Interaction {
    Interactive,
    Noninteractive,
}

/// `sessions.create_command` (HP CA-19, section 24).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCommandArgs {
    pub argv: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cwd: Option<CwdRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub size: Option<TerminalSize>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<Interaction>,
}

/// `sessions.safe_write` (HP SW-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SafeWriteArgs {
    pub session: SessionRef,
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub submit: Option<bool>,
}

/// `sessions.write` (HP RW-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawWriteArgs {
    pub session: SessionRef,
    pub bytes_b64: String,
}

/// The `state` of a `sessions.write` result (HP RW-2, A2-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WriteState {
    Written,
    Partial,
    Cancelled,
    Unknown,
    Failed,
}

/// `sessions.write` result (HP RW-2, A2-3). `reason` is one of `os_write_failed`, `cancelled`, `link_lost`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WriteResult {
    pub state: WriteState,
    pub accepted_bytes: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub written_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub max_written_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
}

/// The modes of `sessions.terminal_state` (HP SO-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalModesValue {
    pub bracketed_paste: bool,
    pub kitty_flags: u64,
    pub alt_screen: bool,
}

/// `sessions.terminal_state` result (HP SO-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalStateResult {
    pub cursor: Cursor,
    pub modes: TerminalModesValue,
    pub size: TerminalSize,
    /// Unix milliseconds (HP WF-5).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub last_client_input_at: Option<u64>,
    pub model_rev: WireId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReadCursorResult {
    pub cursor: Cursor,
    pub model_rev: WireId,
}

// ---- net, secrets, process ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

/// `http.request` (HP NT-3, A2-17). `headers` is a list of `[name, value]` pairs that keeps repeats and order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HttpRequestArgs {
    pub method: HttpMethod,
    pub url: String,
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
    pub timeout_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub max_response_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsOpenArgs {
    pub url: String,
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
    pub subprotocols: Option<Vec<String>>,
}

/// `ws.send` (HP NT-4): exactly one of `text` and `data_b64`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WsSendArgs {
    Text(WsSendText),
    Data(WsSendData),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsSendText {
    pub ws_id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsSendData {
    pub ws_id: String,
    pub data_b64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Accepted {
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WsCloseArgs {
    pub ws_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub code: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reason: Option<String>,
}

/// The `value` of `secrets.put`: a string, or `null` to delete (HP NT-5, A2-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretValue {
    Delete,
    Set(String),
}

impl JsonSchema for SecretValue {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SecretValue".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"anyOf": [{"type": "null", crate::schema::NULL_ALLOWED: true}, {"type": "string"}]})
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecretsPutArgs {
    pub name: String,
    pub value: SecretValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecretsGetArgs {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SecretValueResult {
    pub value: String,
}

/// `process.run` (HP PX-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessRunArgs {
    pub argv: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cwd: Option<CwdRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub env: Option<BTreeMap<String, String>>,
    pub timeout_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub stdout_tail_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub stderr_tail_bytes: Option<u64>,
}

// ---- fs ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FsListArgs {
    pub root: String,
    pub path: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub after: Option<String>,
}

/// The `kind` of a filesystem entry (HP A2-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FsKind {
    File,
    Dir,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsEntry {
    pub name: String,
    pub kind: FsKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsListResult {
    pub entries: Vec<FsEntry>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FsStatArgs {
    pub root: String,
    pub path: String,
}

/// `fs.stat` result (HP Appendix A). `modified_at` is Unix milliseconds (HP WF-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsStatResult {
    pub kind: FsKind,
    pub size: u64,
    pub modified_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FsReadArgs {
    pub root: String,
    pub path: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub offset: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub length: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsReadResult {
    pub bytes_b64: String,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FsWatchArgs {
    pub root: String,
    pub path: String,
    pub handler_id: String,
}

// ---- plugins ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BindingState {
    Available,
    Unavailable,
    Unmet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BindingStatus {
    pub interface: String,
    pub interface_version: String,
    /// `null` for an unmet optional binding (HP HS-1, PD-5, PD-8); an absent key is accepted as well (A2-25).
    #[serde(default)]
    #[schemars(schema_with = "crate::schema::string_or_null")]
    pub provider: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub provider_version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interface_hash: Option<String>,
    pub state: BindingState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BindingsResult {
    pub bindings: Vec<BindingStatus>,
}

/// `plugins.call` (HP PC-1, PD-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginsCallArgs {
    pub interface: String,
    pub tool: String,
    pub args: Value,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub idempotency_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub timeout_ms: Option<u64>,
}

// ---- web, relay, assets ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub username: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetIceServersArgs {
    pub servers: Vec<IceServer>,
}

/// `{served: true}`: the plugin's own served origin (HP WP-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServedOrigin {
    pub served: ConstTrue,
}

/// The `origin` of `web.set_origin`: an origin string, `{served: true}` or `null` to clear (HP WP-1, A2-8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebOrigin {
    Clear,
    Url(String),
    Served(ServedOrigin),
}

impl JsonSchema for WebOrigin {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "WebOrigin".into()
    }
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let served = g.subschema_for::<ServedOrigin>();
        schemars::json_schema!({"anyOf": [{"type": "null", crate::schema::NULL_ALLOWED: true}, {"type": "string"}, served]})
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetOriginArgs {
    pub origin: WebOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelayId {
    pub relay_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelayDeliverArgs {
    pub relay_id: String,
    pub envelope_b64: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelayStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelayDeliverResult {
    pub status: RelayStatus,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The allowed `content_type` values of `assets.publish` and of staged assets (HP A2-19). Any other value is `invalid_request{content_type_not_allowed}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum AssetContentType {
    #[serde(rename = "text/html")]
    TextHtml,
    #[serde(rename = "text/plain")]
    TextPlain,
    #[serde(rename = "text/css")]
    TextCss,
    #[serde(rename = "application/javascript")]
    ApplicationJavascript,
    #[serde(rename = "application/json")]
    ApplicationJson,
    #[serde(rename = "image/png")]
    ImagePng,
    #[serde(rename = "image/jpeg")]
    ImageJpeg,
    #[serde(rename = "image/gif")]
    ImageGif,
    #[serde(rename = "image/webp")]
    ImageWebp,
    #[serde(rename = "image/svg+xml")]
    ImageSvgXml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetsPublishArgs {
    pub name: String,
    pub content_type: AssetContentType,
    pub bytes_b64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssetId {
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetsLookupArgs {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AssetLookupResult {
    pub asset_id: String,
    pub content_type: AssetContentType,
}

// ---- session types, targets, worktrees ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionTypesListArgs {
    pub target_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypesListResult {
    pub session_types: Vec<SessionTypeRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionTypesShowArgs {
    pub target_id: String,
    pub session_type_id: String,
}

/// `{definition}`: the Hub-client `session_type` resource with `context_keys`, `allowed_environment_overrides`, `interaction` and `presentation` (HP A2-16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeDefinition {
    pub definition: SessionTypeRow,
}

/// `session_types.spawn` (HP CA-14, section 24). `environment` names only `allowed_environment_overrides`; `context` keys are the session type's `context_keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpawnArgs {
    pub session_type_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub target_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub worktree_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cwd: Option<CwdRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub environment: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub prompt: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub context: Option<BTreeMap<String, String>>,
}

/// The worktree request of `ensure_worktree_and_spawn` (HP CA-14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreeRequest {
    pub branch: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub base_ref: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub reuse_worktree_id: Option<String>,
}

/// `session_types.ensure_worktree_and_spawn`: the `spawn` arguments plus `worktree` (HP section 24).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnsureWorktreeAndSpawnArgs {
    pub session_type_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub target_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub worktree_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cwd: Option<CwdRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub environment: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub prompt: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub context: Option<BTreeMap<String, String>>,
    pub worktree: WorktreeRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationArgs {
    pub operation_id: String,
}

/// `after?` of the collection reads that gained a cursor (HP A2-15).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AfterArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnTargetsResult {
    pub targets: Vec<SpawnTargetRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetIdArgs {
    pub target_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    Ok,
    Disabled,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidateResult {
    pub ok: bool,
    pub status: TargetStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreesResult {
    pub worktrees: Vec<WorktreeRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreeIdArgs {
    pub worktree_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeResult {
    pub worktree: WorktreeRow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorktreesDeleteArgs {
    pub worktree_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub delete_branch: Option<bool>,
}

// ---- entity ----

/// `entity.publish` (HP EN-4, A2-7): `patch` is a JSON Merge Patch (RFC 7396) that applies to `entity`.
/// `id` and `entity` are the record pair of HP A5-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EntityPublishArgs {
    pub entity_type: String,
    #[serde(rename = "type")]
    pub change: PublishType,
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub entity: Option<crate::common::OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub patch: Option<Value>,
    /// An opaque application value that the Hub carries and never interprets (HP EN-4).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub revision: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PublishResult {
    pub status: PublishStatus,
}

/// The record `operations.get` and `operations.cancel` answer (HP CA-16, CA-17).
pub type OperationAnswer = OperationRecord;
