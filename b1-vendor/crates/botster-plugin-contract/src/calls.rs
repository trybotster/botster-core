//! The call list of HP Appendix A (section 24) as data: for each call its contexts, grant, argument and result schemas, commit point and failures.
//!
//! Clause: HP section 24, CA-1, CA-3, CA-5, LN-1, LN-2, A2-2, A2-10 to A2-12, A2-31.
//!
//! Calls not listed here do not exist in v1 (HP section 24). Section 9.1 is informative; this table is the list (A2-31).

use crate::call_types::*;
use schemars::JsonSchema;
use serde_json::Value;

/// A context a call may be made in (HP CA-3, section 24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    /// Inside a live request.
    Req,
    /// The bootstrap phase: reads only (HS-7).
    Boot,
    /// Autonomous: a closed set, `log.write` only.
    Auto,
}

/// A failure a call lists: the kind and its specific `detail.code`s.
pub struct Failure {
    pub kind: &'static str,
    pub codes: &'static [&'static str],
}

/// One call of Appendix A.
pub struct CallSpec {
    /// `<area>.<name>` (CA-1).
    pub name: &'static str,
    pub contexts: &'static [Ctx],
    /// The bulk class travels on lane 3 (HP section 2, R-4 P1b).
    pub bulk: bool,
    /// The grant, as Appendix A and MN-2 write it.
    pub grant: &'static str,
    pub args: fn() -> Value,
    pub result: fn() -> Value,
    /// Where the call commits (Appendix A "Effect and commit point").
    pub commit: &'static str,
    /// Whether the `HostResult` carries a meaningful `effect` (CA-5).
    pub mutating: bool,
    /// Failures beyond `COMMON_FAILURES`.
    pub failures: &'static [Failure],
    pub clauses: &'static str,
}

impl CallSpec {
    /// The Lua name `botster.<area>.<name>` (HP LN-1).
    pub fn lua_name(&self) -> String {
        format!("botster.{}", self.name)
    }
}

/// The failures of every call (HP section 24, A2-11).
pub const COMMON_FAILURES: &[Failure] = &[
    Failure {
        kind: "invalid_request",
        codes: &[],
    },
    Failure {
        kind: "capability_denied",
        codes: &[],
    },
    Failure {
        kind: "cancelled",
        codes: &[],
    },
    Failure {
        kind: "timed_out",
        codes: &[],
    },
    Failure {
        kind: "unavailable",
        codes: &["instance_fenced"],
    },
    Failure {
        kind: "response_too_large",
        codes: &[],
    },
    Failure {
        kind: "failed",
        codes: &["wrong_context"],
    },
];

fn sch<T: JsonSchema>() -> Value {
    crate::schema::inline::<T>()
}

const REQ: &[Ctx] = &[Ctx::Req];
const REQ_BOOT: &[Ctx] = &[Ctx::Req, Ctx::Boot];

macro_rules! f {
    ($k:literal) => { Failure { kind: $k, codes: &[] } };
    ($k:literal : $($c:literal),+) => { Failure { kind: $k, codes: &[$($c),+] } };
}

macro_rules! call {
    ($name:literal, $ctx:expr, $grant:literal, $args:ty, $res:ty, $commit:literal, $mut:expr, [$($fail:expr),*], $cl:literal) => {
        CallSpec { name: $name, contexts: $ctx, bulk: false, grant: $grant, args: sch::<$args>, result: sch::<$res>, commit: $commit, mutating: $mut, failures: &[$($fail),*], clauses: $cl }
    };
}

const SCOPED_WRONG_STATE: Failure = f!("failed": "wrong_state");
const NO_RECEIPT: Failure = f!("backpressured": "no_receipt_capacity");
const NO_PORT: Failure = f!("quota_exceeded": "no_session_port");

/// Every call of Appendix A, in the order of the table.
pub const CALLS: &[CallSpec] = &[
    CallSpec { name: "log.write", contexts: &[Ctx::Req, Ctx::Auto, Ctx::Boot], bulk: true, grant: "none", args: sch::<LogWriteArgs>, result: sch::<LogWriteResult>, commit: "none (bounded ring)", mutating: false, failures: &[f!("backpressured"), f!("quota_exceeded": "record_too_large")], clauses: "CA-9, A2-20" },
    call!("hub.identity", REQ_BOOT, "none", NoArgs, HubIdentity, "none", false, [f!("unavailable": "not_ready")], "A24, A2-38"),
    call!("config.get", REQ_BOOT, "none", NoArgs, ConfigValues, "none", false, [], "CF-1"),
    call!("store.get", REQ_BOOT, "plugin_db:<own>", StoreGetArgs, StoreDoc, "none", false, [f!("not_found"), f!("failed": "incompatible_document")], "ST-2, ST-11"),
    call!("store.query", REQ_BOOT, "plugin_db:<own>", StoreQueryArgs, StoreQueryResult, "none", false, [f!("invalid_request": "cursor_invalid", "no_such_index", "limit_required", "index_value_type"), f!("failed": "document_exceeds_page", "incompatible_document")], "ST-4, ST-5, A2-14"),
    call!("store.put", REQ, "plugin_db:<own>", StorePutArgs, StoreRevision, "commit: the durable write returns", true, [f!("conflict": "revision_conflict"), f!("quota_exceeded": "document_bytes", "plugin_bytes", "documents")], "ST-2, ST-8, ST-9"),
    call!("store.delete", REQ, "plugin_db:<own>", StoreDeleteArgs, NoValue, "commit: the durable write returns", true, [f!("conflict": "revision_conflict"), f!("not_found")], "ST-2, ST-9"),
    call!("store.batch", REQ, "plugin_db:<own>", StoreBatchArgs, StoreBatchResult, "one commit for all ops and indexes", true, [f!("conflict": "revision_conflict"), f!("quota_exceeded": "document_bytes", "plugin_bytes", "documents"), f!("invalid_request": "duplicate_key", "too_many_ops", "expected_revision_required")], "ST-3, A2-9"),
    call!("store.watch", REQ, "plugin_db:<own>", StoreWatchArgs, WatchId, "commit: the watch is registered (not durable)", true, [f!("quota_exceeded": "max_watches"), f!("invalid_request": "unknown_handler", "handler_id_required")], "ST-6, A2-33, A2-53"),
    call!("store.unwatch", REQ, "plugin_db:<own>", UnwatchArgs, NoValue, "none", false, [f!("not_found")], "ST-6"),
    call!("events.emit", REQ, "declared event", EventsEmitArgs, EmitResult, "commit: ingress admitted (not delivery)", true, [f!("backpressured")], "EV-1, EV-3"),
    call!("timers.arm", REQ, "timers", TimersArmArgs, NoValue, "commit: the Hub records the timer; re-arming a timer_id replaces it", true, [f!("quota_exceeded": "max_timers"), f!("invalid_request": "unknown_handler", "handler_id_required")], "CA-12, A2-53"),
    call!("timers.cancel", REQ, "timers", TimersCancelArgs, NoValue, "commit: the Hub drops it (a fire already queued still delivers)", true, [f!("not_found")], "CA-12"),
    call!("sessions.list", REQ, "session_read[:any]", SessionsListArgs, SessionsListResult, "none", false, [f!("unavailable": "not_ready"), f!("invalid_request": "remote_hub_unsupported")], "SS-1, SS-3"),
    call!("sessions.get", REQ, "session_read[:any]", SessionArgs, crate::rows::SessionRow, "none", false, [f!("not_found"), f!("invalid_request": "remote_hub_unsupported"), f!("unavailable": "not_ready")], "SS-2, SS-3"),
    call!("sessions.stop", REQ, "session_stop[:any]", SessionArgs, StopDecision, "commit: Core accepted the stop", true, [f!("not_found"), SCOPED_WRONG_STATE], "SA-1, A2-2, A2-13"),
    call!("sessions.delete", REQ, "session_delete[:any]", SessionArgs, NoValue, "commit: the removal (and restart record)", true, [f!("not_found"), SCOPED_WRONG_STATE], "SA-1, A2-2"),
    call!("sessions.update", REQ, "session_update[:any]", SessionsUpdateArgs, crate::rows::SessionRow, "commit: the write returns", true, [f!("not_found"), f!("capability_denied")], "SA-1, A2-21"),
    call!("sessions.read_screen", REQ, "session_screen[:any]", SessionArgs, ReadScreenResult, "none", false, [f!("not_found"), SCOPED_WRONG_STATE], "SA-1, SO-2, A2-2, A2-30"),
    call!("sessions.create_command", REQ, "session_create_command", CreateCommandArgs, SpawnAdmission, "commit: the launch is durably recorded", true, [f!("invalid_request": "argv_empty", "argv_too_long", "env_too_long", "cwd_not_allowed"), NO_RECEIPT, NO_PORT], "CA-19, A2-12"),
    call!("http.request", REQ, "net (allow)", HttpRequestArgs, HttpAdmission, "commit: queued (effect committed); the request's own effect is none, committed or unknown per NT-9 on the terminal record", true, [f!("capability_denied": "net_denied"), NO_RECEIPT, f!("invalid_request": "net_userinfo", "net_header_forbidden", "net_scheme")], "NT-3, A2-17"),
    call!("ws.open", REQ, "net (allow)", WsOpenArgs, WsAdmission, "as http.request", true, [f!("capability_denied": "net_denied"), NO_RECEIPT, f!("invalid_request": "net_userinfo", "net_header_forbidden", "net_scheme")], "NT-4"),
    call!("ws.send", REQ, "net", WsSendArgs, Accepted, "commit: queued for send", true, [f!("not_found"), f!("backpressured"), f!("failed": "net_closed")], "NT-4"),
    call!("ws.close", REQ, "net", WsCloseArgs, NoValue, "commit: close requested", true, [f!("not_found")], "NT-4"),
    call!("secrets.put", REQ, "secrets", SecretsPutArgs, NoValue, "commit: durable", true, [f!("capability_denied": "not_plugin_writable")], "NT-5, A2-8"),
    call!("secrets.get", REQ_BOOT, "secrets", SecretsGetArgs, SecretValueResult, "none", false, [f!("not_found"), f!("capability_denied")], "NT-5, HS-7"),
    call!("process.run", REQ, "process_run", ProcessRunArgs, ProcessAdmission, "commit: queued; the command's effect is unknown after a timeout or cancel that follows the start", true, [f!("capability_denied"), NO_RECEIPT, f!("invalid_request": "cwd_not_allowed", "argv_empty")], "PX-1 to PX-4"),
    call!("fs.list", REQ, "fs:<root>", FsListArgs, FsListResult, "none", false, [f!("not_found"), f!("invalid_request"), f!("capability_denied": "fs_outside_root")], "FS-2, A2-15"),
    call!("fs.stat", REQ, "fs:<root>", FsStatArgs, FsStatResult, "none", false, [f!("not_found"), f!("invalid_request"), f!("capability_denied": "fs_outside_root")], "FS-2"),
    call!("fs.read", REQ, "fs:<root>", FsReadArgs, FsReadResult, "none", false, [f!("not_found"), f!("invalid_request"), f!("capability_denied": "fs_outside_root"), f!("response_too_large")], "FS-2"),
    call!("fs.watch", REQ, "fs:<root>", FsWatchArgs, WatchId, "commit: the watch is registered (not durable)", true, [f!("quota_exceeded"), f!("not_found"), f!("invalid_request": "unknown_handler"), f!("capability_denied": "fs_outside_root")], "FS-3, A2-33"),
    call!("fs.unwatch", REQ, "fs:<root>", UnwatchArgs, NoValue, "commit: the watch is removed", true, [f!("not_found")], "FS-3"),
    call!("plugins.bindings", REQ_BOOT, "none (own bindings)", NoArgs, BindingsResult, "none", false, [], "PD-8, HS-7"),
    call!("plugins.call", REQ, "plugin_call:<owner>", PluginsCallArgs, PluginCallAdmission, "commit: queued; the callee's effect follows ER-5", true, [f!("not_found"), f!("capability_denied": "undeclared_dependency"), f!("unavailable": "provider_unloaded", "requirement_unmet"), f!("conflict": "call_cycle", "idempotency_key_reuse", "call_depth_exceeded"), NO_RECEIPT], "PC-1 to PC-5, A2-10"),
    call!("web.set_ice_servers", REQ, "web_origin", SetIceServersArgs, NoValue, "commit: durable (ST-9)", true, [f!("invalid_request": "bad_ice_url"), f!("quota_exceeded": "ice_servers_max")], "WP-6"),
    call!("web.set_origin", REQ, "web_origin", SetOriginArgs, NoValue, "commit: durable (ST-9)", true, [f!("invalid_request": "bad_origin")], "WP-1, A2-8"),
    call!("relay.open", REQ, "relay_signaling (surface relay)", NoArgs, RelayId, "commit: the attempt is registered", true, [f!("quota_exceeded": "relay_attempts_max")], "WP-2"),
    call!("relay.deliver", REQ, "relay_signaling (surface relay)", RelayDeliverArgs, RelayDeliverResult, "accepted: committed (the envelope was queued to the attempt); rejected: none (the attempt has ended)", true, [f!("backpressured"), f!("not_found"), f!("invalid_request": "envelope_too_large")], "WP-2, A2-18"),
    call!("assets.publish", REQ, "assets", AssetsPublishArgs, AssetId, "commit: durable (ST-9)", true, [f!("quota_exceeded"), f!("invalid_request": "content_type_not_allowed", "bytes_too_long")], "AS-1, A2-19"),
    call!("assets.lookup", REQ, "assets", AssetsLookupArgs, AssetLookupResult, "none", false, [f!("not_found")], "AS-1"),
    call!("sessions.safe_write", REQ, "session_safe_write[:any]", SafeWriteArgs, SafeWriteAdmission, "commit: admission is committed (effect committed); a refusal is none; completion by operation (SW-2)", true, [f!("not_found"), f!("failed": "wrong_state"), f!("invalid_request": "remote_hub_unsupported", "text_too_long", "text_empty"), f!("backpressured": "no_receipt_capacity", "write_pending_full")], "SW-1 to SW-10, A2-2, A4-1"),
    call!("sessions.write", REQ, "session_write[:any] (raw input)", RawWriteArgs, WriteResult, "commit: bytes written (effect committed); refusal none; timeout or cancel after dispatch unknown (no blind retry)", true, [f!("not_found"), f!("failed": "wrong_state", "not_written"), f!("backpressured"), f!("invalid_request": "remote_hub_unsupported", "bytes_too_long", "bytes_empty", "bytes_invalid")], "RW-1 to RW-3, A2-3"),
    call!("sessions.terminal_state", REQ, "session_observe[:any]", SessionArgs, TerminalStateResult, "none", false, [f!("not_found"), f!("unavailable"), SCOPED_WRONG_STATE], "SO-1, A2-30"),
    call!("sessions.read_cursor", REQ, "session_observe[:any]", SessionArgs, ReadCursorResult, "none", false, [f!("not_found"), f!("unavailable"), SCOPED_WRONG_STATE], "SO-2, A2-30"),
    call!("sessions.restart", REQ, "session_restart[:any]", SessionArgs, SpawnAdmission, "commit: the operation is recorded (CA-14)", true, [f!("not_found"), f!("failed": "wrong_state"), NO_RECEIPT, NO_PORT], "SA-2, CA-14, A2-2"),
    call!("session_types.list", REQ, "none", SessionTypesListArgs, SessionTypesListResult, "none", false, [f!("not_found")], "CA-13, A2-15"),
    call!("session_types.show", REQ, "none", SessionTypesShowArgs, SessionTypeDefinition, "none", false, [f!("not_found")], "A2-16"),
    call!("session_types.spawn", REQ, "session_type_spawn", SpawnArgs, SpawnAdmission, "commit: the launch is durably recorded", true, [f!("not_found"), f!("invalid_request": "unknown_context_key", "environment_not_allowed", "env_too_long", "prompt_too_long"), f!("failed": "target_disabled"), f!("capability_denied"), NO_PORT, NO_RECEIPT], "CA-14, A2-1, A2-12"),
    call!("session_types.ensure_worktree_and_spawn", REQ, "session_type_managed_git_spawn", EnsureWorktreeAndSpawnArgs, SpawnAdmission, "commit: the operation is recorded", true, [f!("not_found"), f!("invalid_request": "unknown_context_key", "environment_not_allowed", "env_too_long", "prompt_too_long"), f!("failed": "target_disabled", "worktree_failed"), f!("capability_denied"), NO_PORT, NO_RECEIPT], "CA-14, A2-1"),
    call!("operations.get", REQ, "none (own operations)", OperationArgs, OperationAnswer, "none (one-shot reconciliation)", false, [f!("not_found"), f!("backpressured")], "CA-16, A2-35"),
    call!("operations.cancel", REQ, "none (own operations)", OperationArgs, OperationAnswer, "committed when the cancel took effect; none when already terminal; unknown on timeout", true, [f!("not_found"), f!("backpressured")], "CA-17"),
    call!("spawn_targets.list", REQ, "none", AfterArgs, SpawnTargetsResult, "none", false, [], "CA-13, A2-15"),
    call!("spawn_targets.validate", REQ, "none", TargetIdArgs, ValidateResult, "none", false, [f!("invalid_request")], "CA-13"),
    call!("worktrees.list", REQ, "none", AfterArgs, WorktreesResult, "none", false, [], "CA-13, A2-15"),
    call!("worktrees.show", REQ, "none", WorktreeIdArgs, WorktreeResult, "none", false, [f!("not_found")], "CA-13"),
    call!("worktrees.delete", REQ, "worktree_delete[:any]", WorktreesDeleteArgs, NoValue, "commit: the removal is durable", true, [f!("not_found"), f!("failed": "wrong_state")], "CA-21, A2-2"),
    call!("entity.publish", REQ, "none (own namespace)", EntityPublishArgs, PublishResult, "commit: the Hub admitted the change (no sequence is returned)", true, [f!("backpressured"), f!("quota_exceeded": "entity_bytes"), f!("invalid_request": "entity_schema"), f!("not_found")], "EN-4, A2-7"),
];

/// The `CallSpec` of a call name, if it exists (HP CA-1: an unknown call is `invalid_request{unknown_call}`).
pub fn find(name: &str) -> Option<&'static CallSpec> {
    CALLS.iter().find(|c| c.name == name)
}

/// Whether `call` is made in `ctx` (HP CA-3).
pub fn allowed_in(call: &str, ctx: Ctx) -> bool {
    find(call).is_some_and(|c| c.contexts.contains(&ctx))
}

/// The host-local names of the `botster` table that are not host calls (HP LN-1, A2-34).
pub const LUA_LOCAL_NAMES: &[&str] = &[
    "json.encode",
    "json.decode",
    "json.array",
    "json.empty_array",
    "json.null",
    "base64.encode",
    "base64.decode",
    "crypto.sha256",
    "crypto.hmac_sha256",
    "crypto.random_bytes",
    "ids.new",
    "clock.now",
    "clock.monotonic",
];

/// The load-time registration helpers (HP LN-1). Not host calls; a call after `Ready` fails `invalid_request{registration_after_ready}`.
pub const LUA_REGISTRATION_NAMES: &[&str] = &[
    "register",
    "events.on",
    "events.on_gap",
    "operations.on",
    "webhooks.on",
    "relay.on",
    "fs.on_watch",
];

/// The level helpers that are sugar for `log.write` (HP LN-1).
pub const LUA_LOG_SUGAR: &[&str] = &["log.debug", "log.info", "log.warn", "log.error"];
