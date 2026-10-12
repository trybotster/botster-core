//! The registry of `kind` and every published `detail.code` (the source of `plugin-errors.json`).
//!
//! Clause: HP section 14, ER-3, A2-2, A2-27, Appendix B.

use serde_json::{json, Value};

use crate::result::ErrorKind;

/// One published `detail.code`: the kind it travels under and the clauses that publish it.
pub struct CodeSpec {
    pub code: &'static str,
    pub kind: &'static str,
    pub clauses: &'static str,
}

macro_rules! codes {
    ($($kind:literal : { $($code:literal => $cl:literal),* $(,)? }),* $(,)?) => {
        &[ $( $( CodeSpec { code: $code, kind: $kind, clauses: $cl } ),* ),* ]
    };
}

/// Every published `detail.code` that travels in an error `detail` (HP ER-3: stable once published).
pub const CODES: &[CodeSpec] = codes! {
    "invalid_request": {
        "unknown_call" => "CA-1",
        "cursor_invalid" => "ST-5",
        "no_such_index" => "A24 store.query",
        "limit_required" => "ST-4",
        "index_value_type" => "A2-14",
        "duplicate_key" => "ST-3",
        "too_many_ops" => "ST-3",
        "expected_revision_required" => "A2-9",
        "remote_hub_unsupported" => "VC-8, SS-1, SS-2",
        "text_too_long" => "SW-1",
        "text_empty" => "SW-1",
        "bytes_too_long" => "RW-2, AS-1",
        "bytes_empty" => "RW-2",
        "bytes_invalid" => "RW-2",
        "argv_empty" => "CA-19, PX-1",
        "argv_too_long" => "CA-19",
        "env_too_long" => "CA-19, A2-1",
        "cwd_not_allowed" => "CA-19, PX-1",
        "unknown_context_key" => "CA-14, A2-1",
        "environment_not_allowed" => "A2-1",
        "prompt_too_long" => "A2-1",
        "net_userinfo" => "NT-2",
        "net_header_forbidden" => "NT-2",
        "net_scheme" => "A2-17",
        "content_type_not_allowed" => "A2-19",
        "bad_ice_url" => "WP-6",
        "bad_origin" => "WP-1",
        "envelope_too_large" => "WP-2",
        "catalog_version_unsupported" => "A2-29",
        "registration_after_ready" => "LN-1",
        "unknown_handler" => "FS-3, A2-53",
        "handler_id_required" => "A2-53",
        "gap_handler_required" => "WP-4",
        "entity_schema" => "A2-7",
        "interface_schema_unsupported" => "PD-3, PD-3b",
        "interface_unknown_required" => "PD-4",
        "interface_not_additive" => "A8-2, PD-9",
        "manifest_unknown_key" => "MN-1, PK-4",
        "manifest_bad_version" => "PD-1",
        "manifest_bad_scope" => "MN-2",
        "manifest_bad_allow" => "NT-2",
        "manifest_bad_applies_to" => "VW-8",
        "manifest_bad_mcp_server" => "MP-1",
        "manifest_bad_notifications" => "CA-22",
        "manifest_bad_pointer" => "VW-8, VW-9",
        "manifest_bad_port_env" => "CA-20",
        "manifest_bad_reaction" => "VW-9",
        "manifest_missing" => "PK-3",
        "manifest_malformed" => "PK-3, A2-36",
        "manifest_ambiguous" => "PK-6, PK-8",
        "path_invalid" => "PK-2",
    },
    "capability_denied": {
        "net_denied" => "NT-2, NT-9, MP-3",
        "fs_outside_root" => "FS-2",
        "undeclared_dependency" => "PD-5",
        "not_plugin_writable" => "NT-5, A24 secrets.put",
    },
    "conflict": {
        "revision_conflict" => "ST-2, ST-3",
        "stale_descriptor" => "IV-8, VW-3",
        "idempotency_key_reuse" => "PC-4",
        "call_cycle" => "PC-3",
        "call_depth_exceeded" => "A2-10, PC-3",
        "dependency_cycle" => "PD-7",
        "requirement_conflict" => "PD-6",
        "ambiguous_provider" => "PD-6",
        "breaking_not_accepted" => "A8-3, PD-6",
        "grants_mismatch" => "A2-29",
    },
    "quota_exceeded": {
        "record_too_large" => "A24 log.write",
        "document_bytes" => "ST-8",
        "plugin_bytes" => "ST-8",
        "documents" => "ST-8",
        "max_watches" => "ST-6",
        "max_timers" => "A24 timers.arm",
        "host_memory" => "LM-7",
        "no_session_port" => "CA-20, A2-1, A2-12",
        "relay_attempts_max" => "WP-2",
        "ice_servers_max" => "WP-6",
        "entity_bytes" => "A2-7",
        "package_too_large" => "A2-29",
        "catalog_too_large" => "A2-29",
    },
    "backpressured": {
        "no_receipt_capacity" => "CA-16, A2-1, A2-12",
        "write_pending_full" => "A4-1, SW-5",
    },
    "timed_out": {
        "net_timeout" => "NT-9",
    },
    "cancelled": {
        "net_cancelled" => "NT-9",
    },
    "response_too_large": {
        "net_too_large" => "NT-9",
    },
    "unavailable": {
        "not_ready" => "SS-3, A24 sessions.list",
        "provider_unloaded" => "PD-8",
        "requirement_unmet" => "PD-6, PD-8",
        "mcp_protocol" => "MP-2",
        "mcp_server_unavailable" => "MP-6",
        "mcp_bound_exceeded" => "MP-12",
        "instance_fenced" => "A2-11",
    },
    "failed": {
        "wrong_state" => "SA-1, SA-2, A2-2",
        "wrong_context" => "CA-3, LD-3, A2-11",
        "incompatible_document" => "ST-11, LD-10",
        "document_exceeds_page" => "ST-4",
        "plugin_crashed" => "CR-2, PC-5",
        "handler_error" => "A2-10",
        "not_written" => "RW-2, A2-3",
        "os_write_failed" => "RW-2, A2-3",
        "net_dns" => "NT-9",
        "net_connect" => "NT-5, NT-9",
        "net_tls" => "NT-9",
        "net_closed" => "A24 ws.send",
        "command_not_found" => "PX-2",
        "process_bridge_failed" => "PX-2",
        "worktree_failed" => "A24 ensure_worktree_and_spawn",
        "target_disabled" => "A2-1, A2-4",
        "hash_mismatch" => "PM-6, PD-3",
        "requirement_missing" => "PD-6",
        "mcp_server_restarted" => "MP-15",
        "path_not_found" => "PK-2",
        "path_not_contained" => "PK-2",
        "api_unsupported" => "PD-2, A2-27",
        "feature_missing" => "HS-4, PD-2, A2-27",
        "ui_contract_unsupported" => "PD-2, A2-27",
        "catalog_invalid" => "A2-44",
        "unpack_refused" => "A2-29, PM-4",
    },
};

/// Codes that name a load or registration failure without a stated kind. The contract writes the bare code (HS-4, HS-5, CP-2, TR-1, section 18).
pub const LOAD_FAILURE_CODES: &[(&str, &str)] = &[
    ("capability_missing", "CP-2"),
    ("startup_timeout", "HS-5"),
    ("host_not_trusted", "TR-1"),
    ("unsupported_handler_kind", "VW-2"),
];

/// The `Fault` codes (HP CR-2, HS-3, VR-2).
pub const FAULT_CODES: &[(&str, &str)] = &[
    ("unsupported_frame", "VR-2, section 2"),
    ("unsupported_protocol", "HS-3"),
];

/// The closed `detail.reason` set of a `wrong_state` refusal, per call (HP A2-2).
pub const WRONG_STATE_REASONS: &[(&str, &[&str])] = &[
    (
        "sessions.stop",
        &[
            "starting",
            "running",
            "stopping",
            "ended",
            "lost",
            "no_screen_model",
        ],
    ),
    (
        "sessions.delete",
        &[
            "starting",
            "running",
            "stopping",
            "ended",
            "lost",
            "no_screen_model",
        ],
    ),
    (
        "sessions.read_screen",
        &[
            "starting",
            "running",
            "stopping",
            "ended",
            "lost",
            "no_screen_model",
        ],
    ),
    (
        "sessions.terminal_state",
        &[
            "starting",
            "running",
            "stopping",
            "ended",
            "lost",
            "no_screen_model",
        ],
    ),
    (
        "sessions.read_cursor",
        &[
            "starting",
            "running",
            "stopping",
            "ended",
            "lost",
            "no_screen_model",
        ],
    ),
    ("sessions.safe_write", &["session_ended", "not_running"]),
    ("sessions.write", &["session_ended", "not_running"]),
    (
        "sessions.restart",
        &[
            "not_ended",
            "not_restartable",
            "record_unavailable",
            "environment_not_retained",
            "not_ready",
        ],
    ),
    ("worktrees.delete", &["occupied"]),
];

/// The reasons of a `not_written` failure (HP A2-3).
pub const NOT_WRITTEN_REASONS: &[&str] = &[
    "refused",
    "unsupported",
    "mode_precondition_failed",
    "stale",
    "not_reported",
];

/// The `reason` values of a `sessions.write` result (HP A2-3).
pub const WRITE_RESULT_REASONS: &[&str] = &["os_write_failed", "cancelled", "link_lost"];

/// The ingress statuses of `events.emit` and the kind each maps to (HP EV-3). Non-exhaustive: the kind is authoritative.
pub const INGRESS_STATUSES: &[(&str, &str)] = &[
    ("accepted", ""),
    ("rejected_undeclared", "capability_denied"),
    ("rejected_foreign", "capability_denied"),
    ("rejected_audience", "capability_denied"),
    ("rejected_invalid", "invalid_request"),
    ("shed_ingress_full", "backpressured"),
];

/// The meaning of each kind (HP section 14).
pub const KIND_MEANINGS: &[(&str, &str)] = &[
    ("invalid_request", "argument shape or value refused"),
    (
        "capability_denied",
        "the package lacks the grant, or the object is not the plugin's",
    ),
    ("not_found", "the named object does not exist"),
    ("conflict", "revision conflict or already exists"),
    ("quota_exceeded", "a plugin-owned quota is full"),
    (
        "response_too_large",
        "one result or item is larger than the bound; nothing is truncated",
    ),
    (
        "backpressured",
        "a bounded queue or reservation is full now",
    ),
    ("timed_out", "the deadline expired"),
    ("cancelled", "the plugin or the Hub cancelled the operation"),
    (
        "unavailable",
        "the target runtime is stopped, not ready or not configured",
    ),
    ("failed", "the operation ran and failed; detail explains"),
];

/// The text of `plugin-errors.json` as a JSON value (HP Appendix B).
pub fn registry_json() -> Value {
    let kinds: Vec<Value> = KIND_MEANINGS
        .iter()
        .map(|(k, m)| json!({"kind": k, "meaning": m}))
        .collect();
    debug_assert_eq!(kinds.len(), ErrorKind::ALL.len());
    let codes: Vec<Value> = CODES
        .iter()
        .map(|c| json!({"code": c.code, "kind": c.kind, "clauses": c.clauses}))
        .collect();
    let reasons: serde_json::Map<String, Value> = WRONG_STATE_REASONS
        .iter()
        .map(|(call, r)| (call.to_string(), json!(r)))
        .collect();
    json!({
        "description": "HP Appendix B: the registry of kind and every published detail.code",
        "effects": ["none", "committed", "unknown"],
        "kinds": kinds,
        "codes": codes,
        "load_failure_codes": LOAD_FAILURE_CODES.iter().map(|(c, cl)| json!({"code": c, "clauses": cl})).collect::<Vec<_>>(),
        "fault_codes": FAULT_CODES.iter().map(|(c, cl)| json!({"code": c, "clauses": cl})).collect::<Vec<_>>(),
        "wrong_state_reasons": reasons,
        "not_written_reasons": NOT_WRITTEN_REASONS,
        "write_result_reasons": WRITE_RESULT_REASONS,
        "ingress_statuses": INGRESS_STATUSES.iter().map(|(s, k)| json!({"status": s, "kind": k})).collect::<Vec<_>>(),
    })
}
