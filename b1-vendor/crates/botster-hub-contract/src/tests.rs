//! Tests of the Hub contract types: serde forms (A.1, A.1b), the open sets (EN-6), the registries, the schemas and their drift.

use crate::prelude::*;
use crate::schema;
use serde_json::{json, Value};

fn valid_against(schema: &Value, def: Option<&str>, v: &Value) -> bool {
    let root = match def {
        None => schema.clone(),
        Some(name) => {
            let mut s = json!({ "$ref": format!("#/$defs/{name}") });
            s["$defs"] = schema["$defs"].clone();
            s
        }
    };
    jsonschema::validator_for(&root).unwrap().is_valid(v)
}

fn envelope() -> Value {
    schema::envelope()
}

fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(v)
}

fn round<T: serde::de::DeserializeOwned + serde::Serialize + PartialEq + std::fmt::Debug>(
    v: Value,
) {
    let t: T = parse(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"));
    assert_eq!(
        serde_json::to_value(&t).unwrap(),
        v,
        "re-serialization is the same JSON"
    );
}

// ---------------------------------------------------------------- one inventory per non-exhaustive set (design 8; vault: one variant inventory)

macro_rules! open_set_checks {
    ($($t:ty),* $(,)?) => {
        /// Every variant of every open set writes its inventory name and reads back as itself. An unknown name reads as `Other`.
        #[test]
        fn every_open_set_has_one_inventory() {
            $(
                assert_eq!(<$t>::ALL.len(), <$t>::NAMES.len(), stringify!($t));
                for (variant, name) in <$t>::ALL.iter().zip(<$t>::NAMES) {
                    assert_eq!(variant.as_str(), *name);
                    assert_eq!(serde_json::to_value(variant).unwrap(), json!(name));
                    assert_eq!(serde_json::from_value::<$t>(json!(name)).unwrap(), *variant);
                }
                assert_eq!(serde_json::from_value::<$t>(json!("a-value-from-a-later-version")).unwrap(), <$t>::Other, stringify!($t));
                assert!(serde_json::from_value::<$t>(json!(7)).is_err(), stringify!($t));
                // The schema lists exactly the inventory (and accepts any other string, EN-6).
                let schema = serde_json::to_value(schemars::schema_for!($t)).unwrap();
                assert_eq!(schema["anyOf"][0]["enum"], json!(<$t>::NAMES), stringify!($t));
            )*
        }
    };
}

open_set_checks!(
    ErrorCode,
    Category,
    ErrorEffect,
    Retryable,
    Verb,
    EffectClass,
    ListMode,
    SnapshotReason,
    WatchClosedReason,
    EventPriority,
    SourceKind,
    Recovery,
    Severity,
    Activity,
    Interaction,
    Presentation,
    SoftwareChannel,
    HubRunState,
    NotificationMode,
    TargetKind,
    WorktreeState,
    PackageKind,
    PackageState,
    BindingState,
    Delivery,
    QuarantineReason,
    NavTargetKind,
    ViewComponent,
    LogLevel,
    LogGap,
    CheckState,
    AttachHistory,
    Lifecycle,
    Decision,
);

#[test]
fn data_carrying_sets_read_unknown_names_as_other() {
    // A.1b: a name with data is a one-key object. An unknown name whose data is an object reads as `Other` (EN-6).
    assert_eq!(
        parse::<CloseReason>(json!({ "from_a_later_version": { "x": 1 } })).unwrap(),
        CloseReason::Other
    );
    assert_eq!(
        parse::<CloseReason>(json!("from_a_later_version")).unwrap(),
        CloseReason::Other
    );
    assert!(
        parse::<CloseReason>(json!({ "busy": {} })).is_err(),
        "a known name with bad data stays an error"
    );
    assert_eq!(
        parse::<PackageStatus>(json!("from_a_later_version")).unwrap(),
        PackageStatus::Other
    );
    assert_eq!(
        parse::<PackageStatus>(json!({ "degraded": { "interface": "x@1" } })).unwrap(),
        PackageStatus::Degraded(DegradedData {
            interface: "x@1".into()
        })
    );
    for name in PackageStatus::NAMES {
        assert!(*name != "other");
    }
    assert_eq!(
        parse::<UpdateState>(json!({ "failed": { "reason": "download_failed" } })).unwrap(),
        UpdateState::Failed(ReasonData {
            reason: "download_failed".into()
        })
    );
}

// ---------------------------------------------------------------- errors (section 11)

#[test]
fn error_table_has_one_row_per_code_in_order() {
    // HC section 11 lists 16 codes.
    assert_eq!(ErrorCode::ALL.len(), 16);
    assert_eq!(ERROR_TABLE.len(), ErrorCode::ALL.len());
    for (row, code) in ERROR_TABLE.iter().zip(ErrorCode::ALL) {
        assert_eq!(row.code, *code);
        assert_eq!(code.row().unwrap(), row);
    }
    assert_eq!(ErrorCode::Other.row(), None);
}

#[test]
fn error_table_matches_the_section_11_table() {
    // Written from the section 11 table: (code, category, retryable).
    let table = [
        ("invalid_arguments", "caller", "no"),
        ("not_found", "caller", "no"),
        ("unsupported", "caller", "no"),
        ("conflict", "state", "no"),
        ("wrong_state", "state", "depends_on_reason"),
        ("permission_denied", "auth", "no"),
        ("too_many_requests", "capacity", "yes"),
        ("busy", "capacity", "yes"),
        ("response_too_large", "capacity", "no"),
        ("plugin_failed", "fault", "no"),
        ("remote_hub_unsupported", "caller", "no"),
        ("hub_shutdown", "state", "yes"),
        ("expired_cursor", "state", "yes"),
        ("stale_descriptor", "state", "yes"),
        ("cancelled", "state", "yes"),
        ("internal", "fault", "no"),
    ];
    assert_eq!(ERROR_TABLE.len(), table.len());
    for (row, (code, category, retryable)) in ERROR_TABLE.iter().zip(table) {
        assert_eq!(
            (
                row.code.as_str(),
                row.category.as_str(),
                row.retryable.as_str()
            ),
            (code, category, retryable)
        );
    }
    // `busy` is retryable with `retry_after_ms`; no other code requires it.
    assert_eq!(
        ERROR_TABLE
            .iter()
            .filter(|r| r.retry_after_ms)
            .map(|r| r.code.as_str())
            .collect::<Vec<_>>(),
        ["busy"]
    );
}

#[test]
fn an_unknown_code_keeps_its_category_and_an_unknown_category_is_fault() {
    let e: Error = parse(json!({ "code": "from_a_later_version", "category": "state", "retryable": true, "message": "m" })).unwrap();
    assert_eq!(e.code, ErrorCode::Other);
    assert_eq!(e.category, Category::State);
    let e: Error = parse(json!({ "code": "internal", "category": "from_a_later_version", "retryable": false, "message": "m" })).unwrap();
    assert_eq!(e.category.or_fault(), Category::Fault);
}

#[test]
fn errors_json_lists_every_code() {
    let doc: Value = serde_json::from_str(&errors_json()).unwrap();
    let codes: Vec<&str> = doc["codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ErrorCode::NAMES);
    assert_eq!(doc["unknown_category_reads_as"], "fault");
}

#[test]
fn error_object_forms() {
    // A.1b: error data is a member of `detail`; `reason` stays a string. `effect` is none, unknown or partial.
    round::<Error>(json!({
        "code": "wrong_state", "category": "state", "retryable": false, "effect": "none", "message": "m",
        "reason": "unadoptable_sessions", "detail": { "sessions": [], "unknown": [] },
        "field_errors": [{ "pointer": "/a", "field_id": "f", "code": "not_found", "message": "m" }], "retry_after_ms": 5
    }));
    assert!(parse::<Error>(json!({ "code": "busy", "category": "capacity", "retryable": true, "message": "m", "reason": null })).is_err(), "A.1: null is not a value of an optional field");
}

// ---------------------------------------------------------------- the envelope (section 3)

#[test]
fn client_frames_have_the_clause_shapes() {
    round::<ClientFrame>(json!({
        "type": "hello", "protocol": "botster-hub", "versions": { "min": 1, "max": 1 }, "client": { "name": "web", "version": "0.1.0" },
        "features": ["ui_embed"], "terminal_formats": ["ghostsnp/2+raw"]
    }));
    round::<ClientFrame>(
        json!({ "type": "req", "id": "1", "verb": "list", "kind": "session", "args": { "limit": 10 } }),
    );
    round::<ClientFrame>(json!({
        "type": "req", "id": "2", "verb": "invoke", "kind": "session", "target": "s1", "action": "stop", "args": {},
        "idempotency_key": "k", "operation_id": "op", "descriptor_id": "d"
    }));
    round::<ClientFrame>(json!({ "type": "cancel", "id": "2" }));
}

#[test]
fn ids_are_decimal_strings() {
    // A.1, EN-1: a u64 is a decimal string. A number, a sign, a leading zero and an overflow are refused.
    for bad in [
        json!(1),
        json!("-1"),
        json!("01"),
        json!(""),
        json!("1.0"),
        json!("18446744073709551616"),
    ] {
        assert!(parse::<Cancel>(json!({ "id": bad })).is_err(), "{bad}");
    }
    assert_eq!(
        parse::<Cancel>(json!({ "id": "18446744073709551615" }))
            .unwrap()
            .id
            .0,
        u64::MAX
    );
    assert_eq!(parse::<Cancel>(json!({ "id": "0" })).unwrap().id.0, 0);
}

#[test]
fn an_unknown_frame_type_is_an_error_and_an_unknown_verb_is_not() {
    // EN-6: an unknown frame type is a protocol error; an unknown verb or kind is read, and the Hub answers `unsupported`.
    assert!(parse::<ClientFrame>(json!({ "type": "ping" })).is_err());
    assert!(parse::<ServerFrame>(json!({ "type": "ping" })).is_err());
    let req: Req =
        parse(json!({ "id": "1", "verb": "frobnicate", "kind": "no_such_kind" })).unwrap();
    assert_eq!(req.verb, Verb::Other);
}

#[test]
fn an_unknown_optional_field_is_ignored() {
    // EN-6.
    let req: Req = parse(json!({ "id": "1", "verb": "get", "kind": "session", "target": "s", "from_a_later_version": true })).unwrap();
    assert_eq!(req.target.as_deref(), Some("s"));
}

#[test]
fn req_action_rule() {
    // A4 EN-1a: `action` exactly when the verb is `invoke`.
    let invoke = |action: Option<&str>| {
        parse::<Req>(json!({ "id": "1", "verb": "invoke", "kind": "session", "action": action }))
            .unwrap_or_else(|_| {
                parse::<Req>(json!({ "id": "1", "verb": "invoke", "kind": "session" })).unwrap()
            })
    };
    assert_eq!(invoke(None).check(), Err(ReqViolation::ActionRequired));
    assert_eq!(invoke(Some("stop")).check(), Ok(()));
    let get: Req =
        parse(json!({ "id": "1", "verb": "get", "kind": "session", "action": "stop" })).unwrap();
    assert_eq!(get.check(), Err(ReqViolation::ActionForbidden));
    // The schema says the same, so a typed negative frame can name the rule.
    let env = envelope();
    let ok = json!({ "type": "req", "id": "1", "verb": "invoke", "kind": "session", "target": "s", "action": "stop" });
    let missing =
        json!({ "type": "req", "id": "1", "verb": "invoke", "kind": "session", "target": "s" });
    let forbidden = json!({ "type": "req", "id": "1", "verb": "get", "kind": "session", "target": "s", "action": "stop" });
    assert!(valid_against(&env, Some("ClientFrame"), &ok));
    assert!(!valid_against(&env, Some("ClientFrame"), &missing));
    assert!(!valid_against(&env, Some("ClientFrame"), &forbidden));
}

#[test]
fn server_frames_have_the_clause_shapes() {
    let limits = json!({
        "max_frame_bytes": 1, "max_chunk_bytes": 1, "max_outstanding": 1, "max_watches": 32, "route_connect_seconds": 1, "idempotency_window_seconds": 1,
        "cursor_idle_seconds": 1, "operation_retention_seconds": 1, "max_paste_bytes": 1, "max_file_bytes": 1, "query_client_deadline_ms": 1
    });
    round::<ServerFrame>(
        json!({ "type": "welcome", "hub_id": "h", "incarnation": "i", "version": 1, "features": [], "terminal_format": "ghostsnp/2+raw", "limits": limits }),
    );
    round::<ServerFrame>(json!({ "type": "res", "id": "1", "ok": {} }));
    round::<ServerFrame>(json!({
        "type": "res", "id": "1", "err": { "code": "busy", "category": "capacity", "retryable": true, "message": "m", "retry_after_ms": 10 }
    }));
    // A `res` has `ok` or `err`, never neither.
    assert!(parse::<ServerFrame>(json!({ "type": "res", "id": "1" })).is_err());
}

#[test]
fn close_reasons_use_the_a1b_forms() {
    // HS-9, HS-8, HS-2, A.1b.
    for (frame, ok) in [
        (
            json!({ "type": "close", "reason": "handshake_order" }),
            true,
        ),
        (json!({ "type": "close", "reason": "auth_failed" }), true),
        (
            json!({ "type": "close", "reason": "route_token_used" }),
            true,
        ),
        (
            json!({ "type": "close", "reason": "route_token_invalid" }),
            true,
        ),
        (
            json!({ "type": "close", "reason": { "busy": { "retry_after_ms": 100 } } }),
            true,
        ),
        (
            json!({ "type": "close", "reason": { "hub_shutdown": { "lock_released": true } } }),
            true,
        ),
        (
            json!({ "type": "close", "reason": { "version_unsupported": { "hub_min": 1, "hub_max": 2 } } }),
            true,
        ),
        (
            json!({ "type": "close", "reason": { "protocol_error": { "code": "duplicate_request_id" } } }),
            true,
        ),
    ] {
        assert_eq!(parse::<ServerFrame>(frame.clone()).is_ok(), ok, "{frame}");
        round::<ServerFrame>(frame.clone());
        assert!(
            valid_against(&envelope(), Some("ServerFrame"), &frame),
            "{frame}"
        );
    }
}

#[test]
fn watch_and_event_frames() {
    // WT-1 to WT-5, EV-2, EV-4. The sub-frame tag is pending steward ruling H2.
    for frame in [
        json!({ "type": "watch", "frame": "snapshot_begin", "watch_id": "w" }),
        json!({ "type": "watch", "frame": "snapshot_begin", "watch_id": "w", "reason": "overflow" }),
        json!({ "type": "watch", "frame": "snapshot_items", "watch_id": "w", "items": [{ "id": "a" }] }),
        json!({ "type": "watch", "frame": "snapshot_end", "watch_id": "w" }),
        json!({ "type": "watch", "frame": "upsert", "watch_id": "w", "id": "a", "value": {} }),
        json!({ "type": "watch", "frame": "patch", "watch_id": "w", "id": "a", "patch": { "x": null } }),
        json!({ "type": "watch", "frame": "remove", "watch_id": "w", "id": "a" }),
        json!({ "type": "watch", "frame": "batch_begin", "watch_id": "w" }),
        json!({ "type": "watch", "frame": "batch_items", "watch_id": "w", "changes": [{ "frame": "remove", "id": "a" }] }),
        json!({ "type": "watch", "frame": "batch_end", "watch_id": "w" }),
        json!({ "type": "watch", "frame": "watch_closed", "watch_id": "w", "reason": "owner_unloaded" }),
        json!({ "type": "event", "stream_id": "s", "owner": "hub", "name": "notice", "priority": "high",
                "source": { "kind": "hub", "instance": "i" }, "payload": {}, "at": 1 }),
        json!({ "type": "event", "frame": "gap", "stream_id": "s", "owner": "hub", "name": "notice", "count": 2, "lost_bytes": 5, "recovery": "unavailable" }),
    ] {
        round::<ServerFrame>(frame.clone());
        assert!(
            valid_against(&envelope(), Some("ServerFrame"), &frame),
            "{frame}"
        );
    }
    // An unknown watch_closed reason is shown as "other" (EN-6).
    let f: ServerFrame = parse(json!({ "type": "watch", "frame": "watch_closed", "watch_id": "w", "reason": "from_a_later_version" })).unwrap();
    assert!(matches!(
        f,
        ServerFrame::Watch(WatchFrame::WatchClosed {
            reason: WatchClosedReason::Other,
            ..
        })
    ));
}

#[test]
fn a_gap_is_not_an_event_and_the_two_never_mix() {
    let gap = json!({ "type": "event", "frame": "gap", "stream_id": "s", "owner": "o", "name": "n", "count": 1, "recovery": "state",
                      "state_read": { "surface": "kind", "grant": "g" } });
    assert!(matches!(
        parse::<ServerFrame>(gap).unwrap(),
        ServerFrame::Event(EventFrame::Gap(_))
    ));
    // A frame with a gap tag and no count is not an event.
    assert!(parse::<ServerFrame>(
        json!({ "type": "event", "frame": "gap", "stream_id": "s", "owner": "o", "name": "n" })
    )
    .is_err());
}

#[test]
fn optional_fields_are_absent_never_null() {
    // A.1: a required field is never null, and an optional field with no value is absent.
    let v = serde_json::to_value(Welcome {
        hub_id: "h".into(),
        incarnation: "i".into(),
        version: 1,
        features: vec![],
        terminal_format: None,
        limits: serde_json::from_value(json!({
            "max_frame_bytes": 1, "max_chunk_bytes": 1, "max_outstanding": 1, "max_watches": 32, "route_connect_seconds": 1, "idempotency_window_seconds": 1,
            "cursor_idle_seconds": 1, "operation_retention_seconds": 1, "max_paste_bytes": 1, "max_file_bytes": 1, "query_client_deadline_ms": 1
        }))
        .unwrap(),
    })
    .unwrap();
    assert!(v.get("terminal_format").is_none());
    assert!(parse::<Welcome>(json!({ "hub_id": "h", "incarnation": null, "version": 1, "features": [], "limits": v["limits"] })).is_err());
    assert!(parse::<Welcome>({
        let mut w = v.clone();
        w["terminal_format"] = Value::Null;
        w
    })
    .is_err());
}

// ---------------------------------------------------------------- the session row (R-11)

fn session_min() -> Value {
    json!({
        "id": "s", "hub_id": "h", "label": "l", "state": "running", "lifecycle_class": "current", "exit": {}, "size": { "rows": 24, "cols": 80 },
        "session_type": { "id": "t", "source": "device" }, "traits": [], "restartable": true, "activity": "idle", "presentation": "default", "metadata": {}
    })
}

#[test]
fn session_row_requires_exactly_the_r11_set() {
    let kind = schema::kind_schema(kind_spec("session").unwrap());
    let mut required: Vec<&str> = kind["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    required.sort_unstable();
    let mut expect = vec![
        "id",
        "hub_id",
        "label",
        "state",
        "lifecycle_class",
        "exit",
        "size",
        "session_type",
        "traits",
        "restartable",
        "activity",
        "presentation",
        "metadata",
    ];
    expect.sort_unstable();
    assert_eq!(required, expect);
    round::<SessionRow>(session_min());
    // Each required field, removed in turn, makes the row invalid.
    for field in &expect {
        let mut row = session_min();
        row.as_object_mut().unwrap().remove(*field);
        assert!(parse::<SessionRow>(row.clone()).is_err(), "{field}");
        assert!(!valid_against(&kind, None, &row), "{field}");
    }
}

#[test]
fn session_row_optional_fields_are_absent_not_null_and_attention_is_typed() {
    let mut row = session_min();
    row["attention"] = json!({ "since": 5, "source": "osc9", "title": "t" });
    row["failure_reason"] = json!("worker_lost");
    row["exit"] = json!({ "code": 1 });
    row["port"] = json!(3000);
    round::<SessionRow>(row.clone());
    for field in [
        "failure_reason",
        "role",
        "interaction",
        "port",
        "owner",
        "worktree_id",
        "branch",
        "title",
        "task",
        "attention",
    ] {
        let mut with_null = session_min();
        with_null[field] = Value::Null;
        assert!(parse::<SessionRow>(with_null.clone()).is_err(), "{field}");
        assert!(
            !valid_against(
                &schema::kind_schema(kind_spec("session").unwrap()),
                None,
                &with_null
            ),
            "{field}"
        );
    }
    row["attention"] = json!({ "since": 5, "source": "osc9", "extra": 1 });
    assert!(
        parse::<SessionRow>(row).is_ok(),
        "an unknown member is ignored (EN-6)"
    );
}

#[test]
fn session_state_to_class_matches_a2c() {
    // A.2c, through the codec's table.
    assert_eq!(
        SessionState::Lost.lifecycle_class(),
        LifecycleClass::Indeterminate
    );
}

// ---------------------------------------------------------------- operations (A.7b)

#[test]
fn operation_states_use_the_a1b_forms() {
    let row =
        |state: Value| json!({ "id": "o", "kind": "spawn", "state": state, "terminal": true });
    for state in [
        json!("starting"),
        json!("installed"),
        json!({ "failed": { "reason": "exec_failed" } }),
        json!("cancelled"),
        json!({ "abandoned": { "reason": "no_echo" } }),
    ] {
        round::<OperationRow>(row(state.clone()));
        assert!(valid_against(
            &schema::kind_schema(kind_spec("operation").unwrap()),
            None,
            &row(state)
        ));
    }
    assert!(parse::<OperationRow>(row(json!({ "failed": "x" }))).is_err());
    assert!(
        parse::<OperationRow>(json!({ "id": "o", "kind": "spawn", "state": "running" })).is_err(),
        "`terminal` is required"
    );
    for name in OPERATION_STATE_NAMES {
        assert!(parse::<OperationState>(json!(name)).is_ok());
    }
}

// ---------------------------------------------------------------- the registry against A.7

#[test]
fn registry_effects_match_a7() {
    // Written from the A.7 table: (kind, verb or action, effect, durable_operation).
    use EffectClass::*;
    let a7: &[(&str, &str, EffectClass, bool)] = &[
        ("session", "create", Accepted, true),
        ("session", "update", Committed, false),
        ("session", "delete", Committed, false),
        ("session", "attach", Immediate, false),
        ("session", "detach", Immediate, false),
        ("route", "delete", Immediate, false),
        ("session", "stop", Accepted, false),
        ("session", "restart", Accepted, true),
        ("session", "read_screen", Read, false),
        ("session", "clear_attention", Committed, false),
        ("session", "safe_write", Accepted, false),
        ("session", "write", Immediate, false),
        ("session_type", "create", Committed, false),
        ("session_type", "update", Committed, false),
        ("session_type", "delete", Committed, false),
        ("spawn_target", "create", Committed, false),
        ("spawn_target", "update", Committed, false),
        ("spawn_target", "delete", Committed, false),
        ("session_type", "resolve", Read, false),
        ("session_type", "definition", Read, false),
        ("spawn_target", "validate", Read, false),
        ("spawn_target", "browse", Read, false),
        ("worktree", "create", Accepted, true),
        ("worktree", "delete", Accepted, true),
        ("package", "update", Committed, false),
        ("package", "enable", Accepted, false),
        ("package", "disable", Accepted, false),
        ("package", "reload", Accepted, false),
        ("package", "start_entrypoint", Accepted, false),
        ("package", "stop_entrypoint", Accepted, false),
        ("package", "restart_entrypoint", Accepted, false),
        ("package", "create", Accepted, true),
        ("package", "delete", Accepted, true),
        ("package", "preview_upgrade", Read, false),
        ("package", "upgrade", Accepted, true),
        ("package", "pin", Committed, false),
        ("package", "unpin", Committed, false),
        ("package", "approve_grants", Committed, false),
        ("package", "rollback", Accepted, true),
        ("catalog", "create", Committed, false),
        ("catalog", "delete", Committed, false),
        ("catalog", "refresh", Accepted, false),
        ("hub", "preview_package_upgrades", Read, false),
        ("hub", "upgrade_packages", Accepted, true),
        ("hub", "rollback_packages", Accepted, true),
        ("package", "open_asset", Immediate, false),
        ("quarantine", "resolve", Committed, false),
        ("navigation", "resolve_route", Read, false),
        ("hub", "check_update", Read, false),
        ("hub", "update", Accepted, true),
        ("hub", "shutdown", Accepted, false),
        ("hub", "restart", Accepted, false),
        ("hub", "diagnostics", Read, false),
        ("hub", "swap_preflight", Read, false),
        ("hub", "swap", Accepted, false),
        ("hub", "issue_pairing", Immediate, false),
        ("client_credential", "delete", Committed, false),
        ("hub", "reload_config", Committed, false),
        ("spawn_target", "rescan_packages", Accepted, false),
        ("push_subscription", "create", Committed, false),
        ("push_subscription", "delete", Committed, false),
    ];
    for (kind, name, effect, durable) in a7 {
        let spec = kind_spec(kind).unwrap_or_else(|| panic!("kind {kind}"));
        if let Some(a) = spec.actions.iter().find(|a| a.name == *name) {
            assert_eq!(
                (a.effect, a.durable_operation),
                (*effect, *durable),
                "{kind}.{name}"
            );
        } else {
            let verb = spec
                .verbs
                .iter()
                .find(|v| v.verb == *name)
                .unwrap_or_else(|| panic!("{kind}.{name} is neither an action nor a verb"));
            assert_eq!(verb.effect, *effect, "{kind}.{name}");
            // A durable accepted verb is the durable operation of A.7 (create, delete).
            assert_eq!(verb.effect == Accepted, *effect == Accepted);
        }
    }
}

#[test]
fn every_kind_of_section_6_is_registered_once() {
    let kinds: Vec<&str> = KINDS.iter().map(|k| k.kind).collect();
    assert_eq!(
        kinds,
        [
            "hub",
            "session",
            "session_type",
            "spawn_target",
            "worktree",
            "package",
            "catalog",
            "quarantine",
            "navigation",
            "view",
            "plugin_log",
            "operation",
            "client_credential",
            "push_subscription",
            "route",
            "kind"
        ]
    );
    for k in KINDS {
        assert!(
            k.schema_version
                .split_once('.')
                .is_some_and(|(a, b)| a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok()),
            "{} schema_version is M.m",
            k.kind
        );
        assert!(k.watchable() == k.verbs.iter().any(|v| v.verb == "watch"));
        // Verbs are declared once, and only the nine verbs of P2 (`describe` is for every kind).
        let mut seen = std::collections::HashSet::new();
        for v in k.verbs {
            assert!(seen.insert(v.verb), "{} {}", k.kind, v.verb);
            assert_ne!(
                Verb::from_name(v.verb),
                Verb::Other,
                "{} {}",
                k.kind,
                v.verb
            );
        }
    }
}

impl Verb {
    fn from_name(name: &str) -> Verb {
        serde_json::from_value(json!(name)).unwrap()
    }
}

#[test]
fn a_registry_reason_names_a_known_code() {
    for k in KINDS {
        for v in k.verbs {
            for r in v.reasons {
                let code = r.split(':').next().unwrap();
                assert_ne!(
                    serde_json::from_value::<ErrorCode>(json!(code)).unwrap(),
                    ErrorCode::Other,
                    "{}.{}: {r}",
                    k.kind,
                    v.verb
                );
            }
        }
        for a in k.actions {
            for r in a.reasons {
                let code = r.split(':').next().unwrap();
                assert_ne!(
                    serde_json::from_value::<ErrorCode>(json!(code)).unwrap(),
                    ErrorCode::Other,
                    "{}.{}: {r}",
                    k.kind,
                    a.name
                );
            }
        }
    }
}

#[test]
fn the_verbs_carry_their_a9b_failures() {
    // HC A2 A.9b: the specific failures of the verb rows travel in `x-verbs`, as the actions' do in `x-actions`.
    let reasons = |kind: &str, verb: &str| {
        schema::kind_schema(kind_spec(kind).unwrap())["x-verbs"][verb]["reasons"].clone()
    };
    assert_eq!(
        reasons("worktree", "create"),
        json!(["wrong_state:not_git", "conflict:branch_checked_out"])
    );
    assert_eq!(
        reasons("worktree", "delete"),
        json!(["wrong_state:dirty", "wrong_state:occupied"])
    );
    assert_eq!(
        reasons("package", "create"),
        json!([
            "invalid_arguments:path_invalid",
            "conflict:already_installed"
        ])
    );
    assert_eq!(
        reasons("spawn_target", "delete"),
        json!(["wrong_state:in_use"])
    );
    assert_eq!(
        reasons("session_type", "delete"),
        json!(["wrong_state:not_editable"])
    );
    assert_eq!(
        reasons("catalog", "create"),
        json!(["invalid_arguments:url_not_https", "conflict"])
    );
    assert_eq!(
        reasons("push_subscription", "create"),
        json!(["invalid_arguments:endpoint_not_https", "conflict"])
    );
    assert_eq!(reasons("client_credential", "delete"), json!([]));
    assert_eq!(reasons("session", "list"), json!([]));
}

#[test]
fn open_asset_carries_its_a9b_failures() {
    // HC A2 A.9b, `package open_asset`: `not_found` (a foreign, disabled, quarantined or removed asset),
    // `wrong_state { assets_origin_unconfigured }`, and `invalid_arguments` when both or neither of `asset_id` and `name` are given.
    let spec = kind_spec("package").unwrap();
    let open = spec
        .actions
        .iter()
        .find(|a| a.name == "open_asset")
        .unwrap();
    assert_eq!(
        open.reasons,
        &[
            "not_found",
            "wrong_state:assets_origin_unconfigured",
            "invalid_arguments"
        ]
    );
}

// ---------------------------------------------------------------- generated schemas

macro_rules! kind_drift {
    ($($kind:ident),* $(,)?) => {$(
        #[test]
        fn $kind() {
            let name = stringify!($kind);
            let spec = kind_spec(name).unwrap();
            let want = schema::render(&schema::kind_schema(spec));
            let have = std::fs::read_to_string(format!("{}/../../schemas/hub-client/kinds/{name}.schema.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
            assert_eq!(have, want, "schemas/hub-client/kinds/{name}.schema.json is out of date; run `cargo xtask codegen`");
            let doc: Value = serde_json::from_str(&have).unwrap();
            assert_eq!(doc["x-kind"], name);
            assert_eq!(doc["x-schema-version"], spec.schema_version, "the kind schema carries its schema_version M.m (section 12, Versioning)");
            assert!(jsonschema::validator_for(&doc).is_ok(), "{name} is a valid JSON Schema");
        }
    )*};
}

/// One drift test per kind schema (design 8 item 4).
mod drift {
    use super::*;
    kind_drift!(
        hub,
        session,
        session_type,
        spawn_target,
        worktree,
        package,
        catalog,
        quarantine,
        navigation,
        view,
        plugin_log,
        operation,
        client_credential,
        push_subscription,
        route,
        kind
    );

    fn drift(file: &str, want: String) {
        let have = std::fs::read_to_string(format!(
            "{}/../../schemas/hub-client/{file}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert_eq!(
            have, want,
            "schemas/hub-client/{file} is out of date; run `cargo xtask codegen`"
        );
    }

    #[test]
    fn envelope() {
        drift("envelope.schema.json", schema::render(&schema::envelope()));
    }

    #[test]
    fn pairing_and_mcp_and_errors() {
        drift("pairing.schema.json", schema::render(&schema::pairing()));
        drift("mcp.schema.json", schema::render(&schema::mcp()));
        drift("errors.json", errors_json());
    }

    #[test]
    fn every_kind_has_a_file_and_every_file_a_kind() {
        let dir = format!(
            "{}/../../schemas/hub-client/kinds",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut files: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        let mut want: Vec<String> = KINDS
            .iter()
            .map(|k| format!("{}.schema.json", k.kind))
            .collect();
        want.sort();
        assert_eq!(files, want);
    }
}

#[test]
fn the_envelope_schema_accepts_the_examples_and_refuses_what_the_clauses_refuse() {
    let env = envelope();
    let hello = json!({ "type": "hello", "protocol": "botster-hub", "versions": { "min": 1, "max": 1 }, "client": { "name": "n", "version": "v" }, "features": [], "terminal_formats": [] });
    assert!(valid_against(&env, Some("ClientFrame"), &hello));
    // HS-1: a missing required field; EN-1: a numeric id; EN-6: an unknown frame type; A.1: a null optional member.
    let mut bad = hello.clone();
    bad.as_object_mut().unwrap().remove("features");
    assert!(!valid_against(&env, Some("ClientFrame"), &bad));
    assert!(!valid_against(
        &env,
        Some("ClientFrame"),
        &json!({ "type": "cancel", "id": 3 })
    ));
    assert!(!valid_against(
        &env,
        Some("ClientFrame"),
        &json!({ "type": "ping" })
    ));
    assert!(!valid_against(
        &env,
        Some("ClientFrame"),
        &json!({ "type": "req", "id": "1", "verb": "get", "kind": "session", "target": null })
    ));
    // EN-6: an unknown verb is valid, because the Hub answers `unsupported`.
    assert!(valid_against(
        &env,
        Some("ClientFrame"),
        &json!({ "type": "req", "id": "1", "verb": "frobnicate", "kind": "k" })
    ));
}

#[test]
fn a_merge_patch_may_carry_null() {
    // A.1, VB-4: null deletes a member of a merge patch.
    let patch: SessionPatch = parse(json!({ "label": null, "task": "t" })).unwrap();
    assert_eq!(patch.label, Some(None));
    assert_eq!(patch.task, Some(Some("t".into())));
    assert_eq!(
        patch,
        SessionPatch {
            label: Some(None),
            task: Some(Some("t".into()))
        }
    );
    assert_eq!(
        serde_json::to_value(&patch).unwrap(),
        json!({ "label": null, "task": "t" })
    );
    assert_eq!(
        serde_json::to_value(SessionPatch::default()).unwrap(),
        json!({})
    );
    let kind = schema::kind_schema(kind_spec("session").unwrap());
    assert!(valid_against(
        &kind,
        Some("SessionPatch"),
        &json!({ "label": null })
    ));
}

#[test]
fn every_generated_schema_is_a_valid_json_schema() {
    // Every generated schema is valid JSON Schema 2020-12.
    for (name, s) in schema::all() {
        assert!(jsonschema::validator_for(&s).is_ok(), "{name}");
    }
}

#[test]
fn mcp_resource_uris() {
    // TB-M.
    assert_eq!(row_uri("h", "session", "s1"), "botster://h/session/s1");
    assert_eq!(list_uri("h", "session"), "botster://h/session");
}

#[test]
fn p5_session_ref_converts_to_a_resource_ref() {
    let r: ResourceRef = SessionRef {
        hub_id: "h".into(),
        session_id: "s".into(),
    }
    .into();
    assert_eq!(
        r,
        ResourceRef {
            hub_id: "h".into(),
            kind: "session".into(),
            id: "s".into()
        }
    );
}

#[test]
fn pairing_frames_round_trip() {
    for frame in [
        json!({ "frame": "route_connect", "route_id": "r", "route_token": "t" }),
        json!({ "frame": "auth_challenge", "protocol": 1, "protocols": [1], "route": "/hubs/h", "hub_public_key": "k", "challenge": "c", "client_fingerprint": "a", "hub_fingerprint": "b" }),
        json!({ "frame": "pair_request", "protocol": 1, "challenge": "c", "client_public_key": "k", "expires_at": 9, "proof": "p" }),
        json!({ "frame": "auth_response", "protocol": 1, "challenge": "c", "client_public_key": "k", "signature": "s" }),
        json!({ "frame": "auth_accepted", "protocol": 1, "challenge": "c", "hub_public_key": "k", "client_public_key": "c", "credential_id": "i", "paired": true, "signature": "s" }),
    ] {
        round::<AuthFrame>(frame.clone());
        assert!(
            valid_against(&schema::pairing(), Some("AuthFrame"), &frame),
            "{frame}"
        );
    }
    round::<Signal>(
        json!({ "frame": "signal_offer", "hub_id": "h", "connection_id": "c", "sdp": "s" }),
    );
}

// ---------------------------------------------------------------- review round 1

#[test]
fn ids_and_timestamps_are_bounded_in_the_schema_as_in_the_reader() {
    // EN-1, A.1: a decimal u64 string, and a JSON integer within ±(2^53 − 1).
    let env = envelope();
    let cancel = |id: &str| json!({ "type": "cancel", "id": id });
    for (id, ok) in [
        ("0", true),
        ("9", true),
        ("18446744073709551615", true),
        ("18446744073709551616", false),
        ("19999999999999999999", false),
        ("99999999999999999999", false),
        ("100000000000000000000", false),
        ("00", false),
        ("01", false),
    ] {
        assert_eq!(
            valid_against(&env, Some("ClientFrame"), &cancel(id)),
            ok,
            "schema {id}"
        );
        assert_eq!(
            parse::<Cancel>(json!({ "id": id })).is_ok(),
            ok,
            "reader {id}"
        );
    }
    let event = |at: u64| json!({ "type": "event", "stream_id": "s", "owner": "o", "name": "n", "priority": "low", "source": { "kind": "hub", "instance": "i" }, "payload": {}, "at": at });
    let max_safe = (1u64 << 53) - 1;
    assert!(valid_against(&env, Some("ServerFrame"), &event(max_safe)));
    assert!(!valid_against(
        &env,
        Some("ServerFrame"),
        &event(max_safe + 1)
    ));
    // The limits are integers too.
    let mut limits = json!({
        "max_frame_bytes": 1, "max_chunk_bytes": 1, "max_outstanding": 1, "max_watches": 32, "route_connect_seconds": 1, "idempotency_window_seconds": 1,
        "cursor_idle_seconds": 1, "operation_retention_seconds": 1, "max_paste_bytes": 1, "max_file_bytes": 1, "query_client_deadline_ms": 1
    });
    assert!(valid_against(&env, Some("Limits"), &limits));
    limits["max_file_bytes"] = json!(max_safe + 1);
    assert!(!valid_against(&env, Some("Limits"), &limits));
}

#[test]
fn open_data_sets_have_the_same_reader_in_the_schema() {
    // EN-6, A.1b, A.2: an unknown string or one-key object with an object value is "other"; a known name with bad data is an error. Reader and schema agree.
    let env = envelope();
    for (frame, ok) in [
        (
            json!({ "type": "close", "reason": { "future_reason": {} } }),
            true,
        ),
        (json!({ "type": "close", "reason": "future_reason" }), true),
        (json!({ "type": "close", "reason": { "busy": {} } }), false),
        (
            json!({ "type": "close", "reason": { "future_reason": 1 } }),
            false,
        ),
        (
            json!({ "type": "close", "reason": { "a": {}, "b": {} } }),
            false,
        ),
        (
            json!({ "type": "close", "reason": { "busy": { "retry_after_ms": 5 } } }),
            true,
        ),
    ] {
        assert_eq!(
            parse::<ServerFrame>(frame.clone()).is_ok(),
            ok,
            "reader {frame}"
        );
        assert_eq!(
            valid_against(&env, Some("ServerFrame"), &frame),
            ok,
            "schema {frame}"
        );
    }
    let package = schema::kind_schema(kind_spec("package").unwrap());
    let hub = schema::kind_schema(kind_spec("hub").unwrap());
    for (status, ok) in [
        (json!({ "future_status": {} }), true),
        (json!("future_status"), true),
        (json!({ "degraded": { "interface": "x@1" } }), true),
        (json!({ "degraded": {} }), false),
        (json!("live"), true),
    ] {
        assert_eq!(
            parse::<PackageStatus>(status.clone()).is_ok(),
            ok,
            "reader {status}"
        );
        assert_eq!(
            valid_against(&package, Some("PackageStatus"), &status),
            ok,
            "schema {status}"
        );
    }
    for (state, ok) in [
        (json!({ "future": {} }), true),
        (json!({ "failed": { "reason": "r" } }), true),
        (json!({ "failed": {} }), false),
        (json!("idle"), true),
    ] {
        assert_eq!(
            parse::<UpdateState>(state.clone()).is_ok(),
            ok,
            "reader {state}"
        );
        assert_eq!(
            valid_against(&hub, Some("UpdateState"), &state),
            ok,
            "schema {state}"
        );
    }
}

#[test]
fn every_operation_kind_has_its_result_type_in_the_operation_schema() {
    // A.7b: the result types are reachable from the schema, and the table names each kind's result.
    let op = schema::kind_schema(kind_spec("operation").unwrap());
    let defs = op["$defs"].as_object().unwrap();
    for name in [
        "UpdateResult",
        "BulkPackagesResult",
        "PackageInstallResult",
        "PackageUpgradeResult",
        "PackageRollbackResult",
        "RescanResult",
        "SessionRef",
    ] {
        assert!(defs.contains_key(name), "{name}");
    }
    let table = op["x-operation-kinds"].as_object().unwrap();
    assert_eq!(table.len(), OPERATION_KINDS.len());
    assert_eq!(table["hub.update"]["result"], "UpdateResult");
    assert_eq!(
        table["spawn_target.rescan_packages"]["result"],
        "RescanResult"
    );
    assert_eq!(
        table["spawn"]["states"],
        json!(["starting", "installed", "failed", "cancelled"])
    );
    assert_eq!(
        table["spawn"]["failed_reasons"],
        json!([
            "exec_failed",
            "cwd_missing",
            "worktree_failed",
            "worker_failed",
            "session_type_unavailable"
        ])
    );
    assert!(table["hub.shutdown"].get("result").is_none());
    // HC A6-1: `package.install` names the package it installed.
    assert_eq!(table["package.install"]["result"], "PackageInstallResult");
}

#[test]
fn a_known_operation_kind_checks_its_result_and_an_unknown_kind_is_opaque() {
    let row = |kind: &str, result: Value| OperationRow {
        id: "o".into(),
        kind: kind.into(),
        state: OperationState {
            name: "succeeded".into(),
            reason: None,
            data: None,
        },
        terminal: true,
        progress: None,
        result: Some(result),
        error: None,
    };
    assert!(row("hub.update", json!({ "version": "1.2.3" }))
        .check_result()
        .is_ok());
    assert!(row("hub.update", json!({ "ver": "1" }))
        .check_result()
        .is_err());
    assert!(row(
        "spawn_target.rescan_packages",
        json!({ "found": ["a@t"], "removed": [] })
    )
    .check_result()
    .is_ok());
    assert!(
        row("spawn_target.rescan_packages", json!({ "found": ["a@t"] }))
            .check_result()
            .is_err()
    );
    assert!(row(
        "hub.upgrade_packages",
        json!({ "packages": [{ "package": "p", "state": "failed", "reason": "r" }] })
    )
    .check_result()
    .is_ok());
    assert!(row(
        "package.upgrade",
        json!({ "package": "p", "version": "1", "resolved_commit": "c" })
    )
    .check_result()
    .is_ok());
    assert!(row(
        "package.rollback",
        json!({ "package": "p", "ref": "r", "resolved_commit": "c" })
    )
    .check_result()
    .is_ok());
    assert!(row("spawn", json!({ "hub_id": "h", "session_id": "s" }))
        .check_result()
        .is_ok());
    assert!(
        row("from_a_later_version", json!({ "anything": 1 }))
            .check_result()
            .is_ok(),
        "EN-6: an unknown kind is opaque"
    );
    // A kind with no A.7b result type keeps whatever it carries.
    assert!(row("hub.shutdown", json!({ "anything": 1 }))
        .check_result()
        .is_ok());
    for k in OPERATION_KINDS {
        assert_eq!(k.result.is_some(), k.check_result.is_some(), "{}", k.kind);
    }
}

#[test]
fn the_write_result_has_the_required_counts_of_each_state() {
    // HP RW-2 through A.9: written and partial carry both counts, unknown carries max_written_bytes.
    let action = kind_spec("session").unwrap();
    assert_eq!(
        action
            .actions
            .iter()
            .find(|a| a.name == "write")
            .unwrap()
            .effect,
        EffectClass::Immediate
    );
    let session = schema::kind_schema(action);
    for (v, ok) in [
        (
            json!({ "state": "written", "accepted_bytes": 3, "written_bytes": 3 }),
            true,
        ),
        (
            json!({ "state": "partial", "accepted_bytes": 3, "written_bytes": 1 }),
            true,
        ),
        (json!({ "state": "unknown", "max_written_bytes": 3 }), true),
        (json!({ "state": "written" }), false),
        (json!({ "state": "written", "accepted_bytes": 3 }), false),
        (json!({ "state": "partial", "written_bytes": 1 }), false),
        (json!({ "state": "partial" }), false),
        (json!({ "state": "unknown" }), false),
        (json!({ "state": "unknown", "accepted_bytes": 3 }), false),
        (json!({ "state": "from_a_later_version" }), false),
    ] {
        assert_eq!(parse::<WriteResult>(v.clone()).is_ok(), ok, "reader {v}");
        assert_eq!(
            valid_against(&session, Some("WriteResult"), &v),
            ok,
            "schema {v}"
        );
    }
    round::<WriteResult>(json!({ "state": "partial", "accepted_bytes": 3, "written_bytes": 1 }));
}

#[test]
fn open_asset_takes_exactly_one_selector() {
    // A2 A.9b: both or neither of `asset_id` and `name` is invalid_arguments.
    let package = schema::kind_schema(kind_spec("package").unwrap());
    for (v, ok) in [
        (json!({ "asset_id": "a" }), true),
        (json!({ "name": "n" }), true),
        (json!({ "asset_id": "a", "name": "b" }), false),
        (json!({}), false),
        (json!({ "asset_id": "a", "from_a_later_version": 1 }), true),
        (json!({ "asset_id": null, "name": "n" }), false),
    ] {
        assert_eq!(parse::<OpenAssetArgs>(v.clone()).is_ok(), ok, "reader {v}");
        assert_eq!(
            valid_against(&package, Some("OpenAssetArgs"), &v),
            ok,
            "schema {v}"
        );
    }
}

#[test]
fn a_res_has_exactly_one_outcome() {
    // EN-2, EN-3: `ok` or `err`, never both, never neither.
    let env = envelope();
    let err =
        json!({ "code": "internal", "category": "fault", "retryable": false, "message": "x" });
    for (frame, ok) in [
        (json!({ "type": "res", "id": "1", "ok": {} }), true),
        (json!({ "type": "res", "id": "1", "err": err }), true),
        (
            json!({ "type": "res", "id": "1", "ok": {}, "err": err }),
            false,
        ),
        (json!({ "type": "res", "id": "1" }), false),
        (json!({ "type": "res", "id": "1", "ok": null }), false),
        (
            json!({ "type": "res", "id": "1", "ok": {}, "from_a_later_version": true }),
            true,
        ),
    ] {
        assert_eq!(
            parse::<ServerFrame>(frame.clone()).is_ok(),
            ok,
            "reader {frame}"
        );
        assert_eq!(
            valid_against(&env, Some("ServerFrame"), &frame),
            ok,
            "schema {frame}"
        );
    }
    round::<ServerFrame>(json!({ "type": "res", "id": "1", "ok": { "a": 1 } }));
}

// ---------------------------------------------------------------- review round 2

#[test]
fn the_operation_row_applies_its_kinds_result_type_in_reader_and_schema() {
    // A.7b: a known kind's `result` has its type; an unknown kind keeps an opaque result (EN-6).
    let op = schema::kind_schema(kind_spec("operation").unwrap());
    let row = |kind: &str, result: Value| json!({ "id": "o", "kind": kind, "state": "succeeded", "terminal": true, "result": result });
    for (kind, result, ok) in [
        ("hub.update", json!({ "version": "1" }), true),
        ("hub.update", json!({ "ver": "1" }), false),
        (
            "spawn_target.rescan_packages",
            json!({ "found": [], "removed": [] }),
            true,
        ),
        (
            "spawn_target.rescan_packages",
            json!({ "found": [] }),
            false,
        ),
        (
            "package.upgrade",
            json!({ "package": "p", "version": "1" }),
            false,
        ),
        // HC A6-1: the install result is the package key and the admitted version.
        (
            "package.install",
            json!({ "package": "p@t1", "version": "1.2.0" }),
            true,
        ),
        ("package.install", json!({ "package": "p" }), false),
        ("spawn", json!({ "hub_id": "h", "session_id": "s" }), true),
        ("spawn", json!({ "session_id": "s" }), false),
        ("hub.shutdown", json!({ "anything": 1 }), true),
        ("from_a_later_version", json!({ "ver": "1" }), true),
    ] {
        let v = row(kind, result);
        assert_eq!(parse::<OperationRow>(v.clone()).is_ok(), ok, "reader {v}");
        assert_eq!(valid_against(&op, None, &v), ok, "schema {v}");
    }
    // The rule is in the definition, so every file that embeds the row has it.
    let session = schema::kind_schema(kind_spec("session").unwrap());
    assert!(!valid_against(
        &session,
        Some("OperationRow"),
        &row("hub.update", json!({ "ver": "1" }))
    ));
}

#[test]
fn a_worktree_row_has_a_head_and_an_absent_branch_when_detached() {
    // HC A6-2: `branch` is present on a branch and absent (never null) when HEAD is detached; `head` is always present.
    let wt = schema::kind_schema(kind_spec("worktree").unwrap());
    let head = "0123456789abcdef0123456789abcdef01234567";
    let row = |extra: Value| {
        let mut v = json!({ "id": "w", "target_id": "t", "path": "/w", "head": head, "state": "ready", "active_sessions": [] });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    for v in [row(json!({ "branch": "main" })), row(json!({}))] {
        round::<WorktreeRow>(v.clone());
        assert!(valid_against(&wt, None, &v), "{v}");
    }
    let detached: WorktreeRow = parse(row(json!({}))).unwrap();
    assert_eq!(detached.branch, None);
    assert_eq!(detached.head, head);
    let null_branch = row(json!({ "branch": null }));
    assert!(parse::<WorktreeRow>(null_branch.clone()).is_err());
    assert!(!valid_against(&wt, None, &null_branch));
    let mut no_head = row(json!({ "branch": "main" }));
    no_head.as_object_mut().unwrap().remove("head");
    assert!(parse::<WorktreeRow>(no_head.clone()).is_err());
    assert!(!valid_against(&wt, None, &no_head));
}

#[test]
fn the_upgrade_previews_carry_the_breaking_pairs() {
    // HC A7-1: `breaking` is always present, with `{consumer, interface, range, interface_version}`; the bulk preview has a set list.
    let hub = schema::kind_schema(kind_spec("hub").unwrap());
    let package = schema::kind_schema(kind_spec("package").unwrap());
    let entry =
        json!({ "consumer": "c", "interface": "i", "range": "^1.0", "interface_version": "2.0.0" });
    let preview = |breaking: Value| {
        let mut v = json!({ "old_commit": "a", "new_commit": "b", "old_hash": "h1", "new_hash": "h2", "changed_files": {}, "manifest_diff": {}, "grant_diff": {} });
        if !breaking.is_null() {
            v["breaking"] = breaking;
        }
        v
    };
    for v in [preview(json!([])), preview(json!([entry.clone()]))] {
        round::<UpgradePreview>(v.clone());
        assert!(valid_against(&package, Some("UpgradePreview"), &v), "{v}");
    }
    for bad in [
        preview(Value::Null),
        preview(json!([{ "consumer": "c", "interface": "i" }])),
    ] {
        assert!(parse::<UpgradePreview>(bad.clone()).is_err(), "{bad}");
        assert!(
            !valid_against(&package, Some("UpgradePreview"), &bad),
            "{bad}"
        );
    }
    let mut each = preview(json!([entry.clone()]));
    each["package"] = json!("p");
    let bulk = json!({ "previews": [each], "ambiguous": [], "breaking": [entry] });
    round::<PreviewPackageUpgradesResult>(bulk.clone());
    assert!(valid_against(
        &hub,
        Some("PreviewPackageUpgradesResult"),
        &bulk
    ));
    let mut no_set = bulk.clone();
    no_set.as_object_mut().unwrap().remove("breaking");
    assert!(parse::<PreviewPackageUpgradesResult>(no_set.clone()).is_err());
    assert!(!valid_against(
        &hub,
        Some("PreviewPackageUpgradesResult"),
        &no_set
    ));
}

#[test]
fn a_package_row_shows_static_origin_or_omits_it() {
    // HC A7-2: `static_origin?` is a string, absent (never null) when not shown.
    let package = schema::kind_schema(kind_spec("package").unwrap());
    let row = |extra: Value| {
        let mut v = json!({ "id": "p", "name": "p", "kind": "plugin", "state": "enabled", "status": "installed", "version": "1.0.0",
            "source": { "kind": "local", "path": "/p" }, "configuration": {}, "entrypoints": [], "notice_reactions": [] });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    for v in [
        row(json!({ "static_origin": "http://127.0.0.1:4100" })),
        row(json!({})),
    ] {
        round::<PackageRow>(v.clone());
        assert!(valid_against(&package, None, &v), "{v}");
    }
    let null = row(json!({ "static_origin": null }));
    assert!(parse::<PackageRow>(null.clone()).is_err());
    assert!(!valid_against(&package, None, &null));
}

#[test]
fn an_unknown_operation_state_with_data_is_readable() {
    // EN-6, A.1b: an unknown name may carry any object; a known state with data needs its `reason`.
    let op = schema::kind_schema(kind_spec("operation").unwrap());
    let row =
        |state: Value| json!({ "id": "o", "kind": "future", "state": state, "terminal": false });
    for (state, ok) in [
        (json!({ "future_state": {} }), true),
        (json!({ "future_state": { "progress": 1 } }), true),
        (json!("future_state"), true),
        (json!({ "future_state": 1 }), false),
        (json!({ "failed": {} }), false),
        (json!({ "failed": { "reason": "r" } }), true),
        (json!({ "abandoned": { "reason": "no_echo" } }), true),
        (json!({ "a": {}, "b": {} }), false),
    ] {
        assert_eq!(
            parse::<OperationRow>(row(state.clone())).is_ok(),
            ok,
            "reader {state}"
        );
        assert_eq!(
            valid_against(&op, None, &row(state.clone())),
            ok,
            "schema {state}"
        );
    }
    round::<OperationRow>(row(json!({ "future_state": { "progress": 1 } })));
}

#[test]
fn narrow_integers_keep_their_rust_range_in_the_schema() {
    // A.1 and the reader's type: the bound is the smaller of the type's range and ±(2^53 − 1).
    let session = schema::kind_schema(kind_spec("session").unwrap());
    let env = envelope();
    let mut row = session_min();
    for (path, max) in [
        ("size/rows", u32::MAX as u64),
        ("size/cols", u32::MAX as u64),
    ] {
        let (a, b) = path.split_once('/').unwrap();
        for (value, ok) in [(max, true), (max + 1, false)] {
            let mut r = row.clone();
            r[a][b] = json!(value);
            assert_eq!(
                parse::<SessionRow>(r.clone()).is_ok(),
                ok,
                "reader {path} {value}"
            );
            assert_eq!(
                valid_against(&session, None, &r),
                ok,
                "schema {path} {value}"
            );
        }
    }
    row["port"] = json!(65535);
    assert!(valid_against(&session, None, &row) && parse::<SessionRow>(row.clone()).is_ok());
    row["port"] = json!(65536);
    assert!(!valid_against(&session, None, &row) && parse::<SessionRow>(row.clone()).is_err());
    let welcome = |v: u64| {
        json!({ "type": "welcome", "hub_id": "h", "incarnation": "i", "version": v, "features": [], "limits": {
        "max_frame_bytes": 1, "max_chunk_bytes": 1, "max_outstanding": 1, "max_watches": 32, "route_connect_seconds": 1, "idempotency_window_seconds": 1,
        "cursor_idle_seconds": 1, "operation_retention_seconds": 1, "max_paste_bytes": 1, "max_file_bytes": 1, "query_client_deadline_ms": 1 } })
    };
    for (value, ok) in [(65535u64, true), (65536, false)] {
        assert_eq!(
            parse::<ServerFrame>(welcome(value)).is_ok(),
            ok,
            "reader version {value}"
        );
        assert_eq!(
            valid_against(&env, Some("ServerFrame"), &welcome(value)),
            ok,
            "schema version {value}"
        );
    }
    let hub = schema::kind_schema(kind_spec("hub").unwrap());
    assert_eq!(hub["properties"]["pid"]["maximum"], u32::MAX);
    assert_eq!(hub["properties"]["worker_protocol"]["maximum"], u16::MAX);
}

// ---------------------------------------------------------------- pairing bytes (PR-2, PR-7b)

/// PR-2 layout, written out by hand: version 1, expires_at 1700000000 (u64be), route_len 8 (u16be), route `/hubs/h1`,
/// hub_public_key 32 × 0x11, pairing_secret 32 × 0x22: 83 bytes. The expected text is RFC 4648 base32 of those bytes with the `=`
/// padding removed (computed outside this crate).
const PR_2_VECTOR: &str = "AEAAAAAAMVJ7CAAABAXWQ5LCOMXWQMIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCERCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCE";

fn pr_2_bundle() -> PairingBundle {
    PairingBundle {
        version: 1,
        expires_at: 1_700_000_000,
        route: "/hubs/h1".into(),
        hub_public_key: [0x11; 32],
        pairing_secret: [0x22; 32],
    }
}

#[test]
fn pr_2_bundle_encodes_to_the_vector_and_decodes_back() {
    assert_eq!(BUNDLE_VERSION, 1);
    assert_eq!(pr_2_bundle().encode().unwrap(), PR_2_VECTOR);
    assert_eq!(PairingBundle::decode(PR_2_VECTOR).unwrap(), pr_2_bundle());
    // R-39: only the upper-case RFC 4648 section 6 alphabet is read; a lower-case bundle is malformed.
    assert_eq!(
        PairingBundle::decode(&PR_2_VECTOR.to_ascii_lowercase()),
        Err(BundleError::Base32)
    );
}

/// The same layout with route `/hubs/h123` (route_len 10): 85 bytes, a multiple of 5, so the text has no partial last character.
/// Computed outside this crate, as [`PR_2_VECTOR`].
const PR_2_VECTOR_85: &str = "AEAAAAAAMVJ7CAAABIXWQ5LCOMXWQMJSGMIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRCEIRC";

#[test]
fn pr_2_bundle_on_a_five_byte_boundary_has_no_partial_character() {
    let mut bundle = pr_2_bundle();
    bundle.route = "/hubs/h123".into();
    assert_eq!(bundle.encode().unwrap(), PR_2_VECTOR_85);
    assert_eq!(PairingBundle::decode(PR_2_VECTOR_85).unwrap(), bundle);
    // One more character holds 5 bits and no byte: refused, also when the bits are zero (`A`).
    assert_eq!(
        PairingBundle::decode(&format!("{PR_2_VECTOR_85}A")),
        Err(BundleError::Base32)
    );
    // The last character of the 83-byte text carries one unused bit; a nonzero unused bit is refused (`E` is 00100, `F` 00101).
    let last = &PR_2_VECTOR[..PR_2_VECTOR.len() - 1];
    assert_eq!(
        PairingBundle::decode(&format!("{last}F")),
        Err(BundleError::Base32)
    );
}

#[test]
fn pr_2_bundle_refuses_padding_an_unknown_version_and_a_wrong_length() {
    assert_eq!(
        PairingBundle::decode(&format!("{PR_2_VECTOR}=")),
        Err(BundleError::Base32)
    );
    assert_eq!(PairingBundle::decode("A1"), Err(BundleError::Base32));
    // Version 2 (`AI` is the base32 of the one byte 0x02): refused before the rest is read.
    assert_eq!(
        PairingBundle::decode("AI"),
        Err(BundleError::UnknownVersion(2))
    );
    let mut long = pr_2_bundle();
    long.route = "/hubs/h12".into();
    let text = long.encode().unwrap();
    assert_eq!(PairingBundle::decode(&text).unwrap(), long);
    // Drop the last 8 characters (5 bytes of the secret): too short.
    assert_eq!(
        PairingBundle::decode(&PR_2_VECTOR[..PR_2_VECTOR.len() - 8]),
        Err(BundleError::Layout)
    );
    long.route = "x".repeat(usize::from(u16::MAX) + 1);
    assert_eq!(long.encode(), Err(BundleError::RouteTooLong));
}

#[test]
fn pr_7b_transcripts_are_the_fields_on_their_own_lines() {
    assert_eq!(
        pairing_transcript(
            "/hubs/h1",
            "HPK",
            "CH",
            "CPK",
            1_700_000_000,
            "sha-256 aa:bb",
            "sha-256 cc:dd"
        ),
        "botster-pairing-v1\n/hubs/h1\nHPK\nCH\nCPK\n1700000000\nsha-256 aa:bb\nsha-256 cc:dd"
    );
    assert_eq!(
        auth_transcript(
            "/hubs/h1",
            "HPK",
            "CH",
            "CPK",
            "sha-256 aa:bb",
            "sha-256 cc:dd"
        ),
        "botster-auth-v1\n/hubs/h1\nHPK\nCH\nCPK\nsha-256 aa:bb\nsha-256 cc:dd"
    );
    assert_eq!(
        accepted_transcript(
            "/hubs/h1",
            "CH",
            "CPK",
            "0123",
            AcceptedMode::Pair,
            "sha-256 aa:bb",
            "sha-256 cc:dd"
        ),
        "botster-auth-accepted-v1\n/hubs/h1\nCH\nCPK\n0123\npair\nsha-256 aa:bb\nsha-256 cc:dd"
    );
    assert_eq!(AcceptedMode::Auth.as_str(), "auth");
}

// ---------------------------------------------------------------- host files, details and closed lists

#[test]
fn mc_1c_context_file_of_a_command_spawn_has_no_session_type_fields() {
    let session = SessionRef {
        hub_id: "h".into(),
        session_id: "s".into(),
    };
    let file = SessionContextFile {
        prompt: None,
        context: Default::default(),
        session,
        session_type_id: None,
        session_type_dir: None,
        target_id: None,
        worktree_id: None,
        worktree_path: None,
        branch: None,
        repo: None,
    };
    assert_eq!(
        serde_json::to_value(&file).unwrap(),
        json!({ "context": {}, "session": { "hub_id": "h", "session_id": "s" } })
    );
    let typed: SessionContextFile = serde_json::from_value(json!({
        "context": { "k": "v" }, "session": { "hub_id": "h", "session_id": "s" },
        "session_type_id": "t", "session_type_dir": "/d", "worktree_path": "/w", "branch": "b", "repo": "o/r"
    }))
    .unwrap();
    assert_eq!(typed.session_type_dir.as_deref(), Some("/d"));
    assert!(serde_json::from_value::<SessionContextFile>(json!({
        "context": {}, "session": { "hub_id": "h", "session_id": "s" }, "branch": null
    }))
    .is_err());
}

#[test]
fn small_typed_shapes_have_their_wire_form() {
    let r = SessionRef {
        hub_id: "h".into(),
        session_id: "s".into(),
    };
    assert_eq!(
        serde_json::to_value(UnadoptableDetail {
            sessions: vec![r.clone()],
            unknown: vec![]
        })
        .unwrap(),
        json!({ "sessions": [{ "hub_id": "h", "session_id": "s" }], "unknown": [] })
    );
    assert_eq!(
        serde_json::to_value(SecretDisplay { set: true }).unwrap(),
        json!({ "set": true })
    );
    assert_eq!(
        serde_json::from_value::<TrustedHost>(json!({ "path": "/bin/x", "sha256": "ab" })).unwrap(),
        TrustedHost {
            path: "/bin/x".into(),
            sha256: "ab".into()
        }
    );
}

#[test]
fn closed_lists_hold_the_contract_values() {
    assert_eq!(
        COMMON_REASONS,
        [
            "conflict:operation_id_in_use",
            "conflict:idempotency_key_reused"
        ]
    );
    assert_eq!(
        CHECK_UPDATE_FAILURE_REASONS,
        ["check_failed", "offline", "dev_build"]
    );
    assert_eq!(PACKAGE_OUTCOME_STATES, ["succeeded", "failed"]);
    assert_eq!(range_filters("plugin_log"), ["after"]);
    assert!(range_filters("session").is_empty());
    for (kind, _) in RANGE_FILTERS {
        assert!(kind_spec(kind).is_some(), "{kind} is a Hub kind");
    }
}

#[test]
fn a8_2_route_channel_labels() {
    assert_eq!(route_datachannel_label("r1"), "route:r1");
    assert_eq!(route_id_of_label("route:r1"), Some("r1"));
    assert_eq!(
        route_id_of_label(&route_datachannel_label("a:b")),
        Some("a:b")
    );
    for label in ["route:", "control", "terminal", "r1", "Route:r1"] {
        assert_eq!(route_id_of_label(label), None, "{label}");
    }
}

#[test]
fn a8_4_route_send_buffer_bytes_parameter() {
    let p = ROUTE_SEND_BUFFER_BYTES;
    assert_eq!((p.table, p.key), ("limits", "route_send_buffer_bytes"));
    assert_eq!((p.default, p.min, p.max), (1_048_576, 4_096, 67_108_864));
    assert!(p.restart);
    for v in [p.min, p.default, p.max] {
        assert!(p.accepts(v), "{v}");
    }
    for v in [0, p.min - 1, p.max + 1] {
        assert!(!p.accepts(v), "{v}");
    }
}
