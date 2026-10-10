//! The Core step vocabulary of every transcript (design 5.5, 6.2). A transcript must run through `CoreDriver`, so each step uses a form
//! that the driver knows: a begin op that `Op` reads, a call and a control from the documented lists, an event name of the contract, and
//! limits that `CoreLimits` has. The test needs no subject.

use botster_core_contract::prelude::*;
use botster_probe_script::Step as ProgramStep;
use serde_json::{json, Value};

/// The steps of the driver and of the codec route steps that Core transcripts use.
const STEPS: &[&str] = &[
    "data_dir",
    "open",
    "drop",
    "begin",
    "pump",
    "pump_idle",
    "drain",
    "poll_all",
    "pump_until",
    "pump_collect",
    "poll_events",
    "call",
    "advance_clock",
    "wait_wake",
    "real_wait",
    "poll_find",
    "control",
    "await_pty",
    "await_quiet",
    "await_control",
    "await_get",
    "check",
    "assume",
    "let",
    "attach_route",
    "type_check",
    "expect",
    "expect_set",
    "expect_eventually",
    "expect_quiet_until",
    "expect_closed",
    "route_flood",
    "route_expect_refusals",
    "collect_events",
    "forbid_events",
    "tap_drain",
    "run_deterministic",
    "run_suite",
    "check_crates",
    "error_codes_reachable",
];
/// The keys of a step that are not its form.
const STEP_KEYS: &[&str] = &[
    "bind",
    "bind_fields",
    "because",
    "expect",
    "expect_error",
    "expect_refused",
    "on",
    "ignore",
    "barrier",
    "until",
    "route",
];
/// The route steps of the codec suite that Core transcripts use (`botster_hub_conformance::route`).
const ROUTE_STEPS: &[&str] = &[
    "route_send",
    "route_send_raw",
    "route_send_wire",
    "send_invalid",
    "route_expect_output",
    "route_expect_pty",
    "route_closed",
    "route_drain",
];
/// The controls with the arguments that they require and the ones they may take (`docs/core-testkit-controls.md`). `handle` is always allowed.
const CONTROL_ARGS: &[(&str, &[&str], &[&str])] = &[
    ("fail_next", &["target", "error"], &[]),
    ("lose_worker", &["session"], &["reason"]),
    ("break_control", &["session"], &["on"]),
    ("descendants", &["session"], &[]),
    ("pty_chunk", &["session"], &["bytes"]),
    ("pty_blocked", &["session"], &["on"]),
    ("pty_input", &["session"], &[]),
    ("pty_output", &["session", "bytes_hex"], &["scripted"]),
    ("withhold_feature", &["name"], &[]),
    ("unavailable_bound", &["bound"], &[]),
    ("control_queue", &["session"], &["on"]),
    ("revisions", &["session"], &["client", "host", "model"]),
    ("oracle_encode", &["session", "input", "kind"], &["modes"]),
    ("hold_start", &["session"], &[]),
    ("release_start", &["session"], &[]),
    ("spawn_record", &["session"], &[]),
    ("service_connect", &["service", "epoch"], &["lanes"]),
    (
        "service_exit",
        &["service"],
        &["code", "signal", "cause_byte"],
    ),
    (
        "service_send_frame",
        &["service", "lane", "type", "payload_hex"],
        &[],
    ),
    // core-p7: the service lane and process edges (docs/core-testkit-controls.md)
    (
        "service_extra_connection",
        &["service", "lane", "epoch"],
        &["field"],
    ),
    ("service_connection_closed", &["connection"], &[]),
    ("service_pids_alive", &["pids"], &[]),
    ("service_send_raw", &["service", "lane", "bytes_hex"], &[]),
    ("service_close_lane", &["service", "lane"], &[]),
    ("service_lane_open", &["service", "lane"], &["epoch"]),
    ("service_read", &["service", "lane"], &[]),
    ("service_lane_accept", &["service", "lane"], &["bytes"]),
    ("service_unread", &["service", "lane"], &[]),
    ("service_spawn_record", &["service"], &[]),
    ("service_fds", &["service"], &[]),
    ("service_guardian_lost", &["service"], &[]),
    ("service_payload_alive", &["service"], &[]),
    ("service_descendants", &["service"], &[]),
    ("service_rlimits", &["service"], &[]),
    ("host_uid", &[], &[]),
    ("oracle_screen", &["session", "history"], &[]),
    ("oracle_state", &["session"], &[]),
    ("disable_history", &["session"], &[]),
    ("payload_alive", &["session"], &[]),
    ("snapshot_unsupported_version", &["capture"], &[]),
    ("oracle_restore", &["session", "capture"], &[]),
    ("oracle_resume", &["session", "capture"], &[]),
    (
        "oracle_resume_every_cut",
        &["size", "corpus_hex"],
        &["beyond_limit"],
    ),
    ("oracle_graphics", &["session", "capture"], &[]),
    ("tic_compile", &["source"], &[]),
    ("no_spurious_wakes", &[], &[]),
    ("schedule", &["first"], &[]),
    ("withhold_control_link", &["session"], &[]),
    // core-p3: the edges of the testkit (Core A5-1, A5-2, A5-3) and the readers that a transcript needs
    ("scheduler_hold", &["for_op"], &[]),
    ("scheduler_order", &["sessions"], &[]),
    ("scheduler_ready", &["for_op"], &[]),
    ("scheduler_deadline", &["before"], &[]),
    ("scheduler_delay", &["session", "point", "pumps"], &[]),
    ("scheduler_work_limit", &["per_pump"], &[]),
    ("scheduler_poll_batch", &["events"], &[]),
    ("route_read_size", &["bytes"], &[]),
    ("queue_overflow", &[], &[]),
    ("program_write_size", &["session", "bytes"], &[]),
    ("program_write_once", &["session", "bytes_hex"], &[]),
    ("oracle_notification", &["session"], &[]),
    ("scheduler_release", &["for_op"], &[]),
    ("process_end_worker", &["session"], &[]),
    ("process_end_guardian", &["service"], &[]),
    ("process_refuse_spawn", &["errno"], &[]),
    ("storage_fail_write", &[], &["uncertain"]),
    ("wake_spurious", &[], &[]),
    ("oracle_hyperlinks", &[], &["capture", "route"]),
    ("oracle_cursor", &["session"], &[]),
    ("control_link_stats", &["session"], &[]),
    ("input_records_cost", &["session"], &[]),
    ("measure_tap_overhead", &["topology"], &[]),
    ("hold_snapshot", &["session"], &[]),
    ("release_snapshot", &["session"], &[]),
    ("uncarriable_sequence", &["session"], &[]),
    // core-p4a-dp: route-transport observations and one handoff fault (docs/core-testkit-controls.md)
    ("fail_handoff", &[], &[]),
    ("input_blocked", &[], &[]),
    ("route_gate", &[], &["on"]),
    ("pty_accept", &["session", "bytes"], &[]),
    ("pty_fail_after", &["session", "bytes"], &[]),
    // core-p4a-2: boundary measurements of the route's stream, the held client edge, and the compression choice (A9-3)
    ("route_stream_frame", &[], &["index"]),
    ("route_stream_consumed", &[], &[]),
    ("route_stream_written", &[], &[]),
    ("route_stream_holders", &[], &[]),
    ("route_stream_closed", &[], &[]),
    ("deflate_choice", &["choice"], &["seed"]),
    ("edges_quiet", &[], &[]),
    // minimum-core: the held hand-over of a route's stream (OU-9) and the oracle reading of a route's screen payload
    ("hold_handoff", &[], &[]),
    ("release_handoff", &[], &[]),
    ("oracle_screen_payload", &["payload"], &[]),
    // minimum-core: the allocation-peak measurement of DP-3 (R-37)
    ("alloc_window", &[], &[]),
    ("alloc_peak", &[], &[]),
    // core-p4a-ou: the faults of one route's transport (docs/core-testkit-controls.md). `storage_fail_write` and `queue_overflow` are core-p3's.
    ("storage_fail_write", &[], &["uncertain"]),
    ("queue_overflow", &[], &[]),
    ("route_accept", &["bytes"], &[]),
    ("fail_writes", &[], &["on"]),
    ("drop_transport", &[], &[]),
    ("client_close", &[], &[]),
    ("unknown_close_reason", &[], &[]),
    ("route_spurious_ready", &[], &[]),
    ("pty_output_unread", &["session"], &[]),
    // core-p5: adoption (docs/core-testkit-controls.md)
    ("registry_row", &["session"], &[]),
    ("hold_start_at", &["session", "before"], &[]),
    ("release_start_at", &["session"], &[]),
    ("announce_protocol", &["session", "protocol"], &[]),
    ("lose_host", &[], &[]),
    ("process_identity", &[], &["session", "service"]),
    ("host_crash", &[], &[]),
    ("reuse_pid", &["session"], &[]),
    ("bystander_alive", &["session"], &[]),
    // core-p5, Core A10: the impostor, the observer of signals, the corrupted row, and the end of a guardian
    ("impostor_worker", &["session", "field"], &["script"]),
    ("signals_received", &["session"], &[]),
    ("corrupt_registry_row", &["session"], &[]),
    ("guardian_exit", &["service"], &[]),
    ("worker_alive", &["session"], &[]),
    ("wait_worker_exit", &["session", "deadline_ms"], &[]),
    // core-p4b: host-side acts and a read-only observer on the host's own file directory (docs/core-testkit-controls.md)
    ("fs_directory", &[], &["name"]),
    ("fs_chmod", &["path", "mode"], &[]),
    ("fs_list", &["path"], &[]),
    // core-p4b part 2: the libghostty oracle for the reply of the shadow terminal (docs/core-testkit-controls.md)
    (
        "oracle_query_reply",
        &["session", "request_hex"],
        &["prefix_hex", "profile"],
    ),
    // core-p4b, Core A12: the worker's report held at the control-link boundary, and a point inside the cleanup (docs/core-testkit-controls.md)
    ("hold_worker_report", &["session"], &[]),
    ("release_worker_report", &["session"], &[]),
    ("held_reports", &["session"], &[]),
    ("hold_cleanup_after", &["session", "deleted"], &[]),
    // core-e3: a read-only observer of the signals that Core sent (docs/core-testkit-controls.md)
    ("signals_sent", &["session"], &[]),
    // core-p4c: the DP-10 and DP-11 measurements of topology T1 (docs/core-testkit-controls.md; Core A17-3)
    ("measure_route_perf", &["topology", "quantity"], &[]),
    ("route_hops", &["topology"], &[]),
];
/// An argument whose type depends on the control: `(control, argument, type)`. It wins over `ARG_TYPES`.
const CONTROL_ARG_TYPES: &[(&str, &str, &str)] = &[
    ("scheduler_deadline", "before", "bool"),
    ("hold_start_at", "before", "string"),
];
/// The JSON type of each control argument that has one type in every control. A variable (`"$x"`) stands for a bound value and passes.
/// The type of the argument `arg` of the control `op`, if it has one.
fn arg_type(op: &str, arg: &str) -> Option<&'static str> {
    CONTROL_ARG_TYPES
        .iter()
        .find(|(c, a, _)| *c == op && *a == arg)
        .map(|(_, _, ty)| *ty)
        .or_else(|| ARG_TYPES.iter().find(|(n, _)| *n == arg).map(|(_, ty)| *ty))
}
const ARG_TYPES: &[(&str, &str)] = &[
    ("session", "string"),
    ("client", "string"),
    ("service", "string"),
    ("name", "string"),
    ("bound", "string"),
    ("target", "string"),
    ("topology", "string"),
    ("point", "string"),
    ("pumps", "integer"),
    ("per_pump", "integer"),
    ("events", "integer"),
    ("sessions", "array"),
    ("route", "string"),
    ("uncertain", "bool"),
    ("errno", "integer"),
    ("kind", "string"),
    ("before", "string"),
    ("deleted", "integer"),
    ("request_hex", "string"),
    ("prefix_hex", "string"),
    ("profile", "object"),
    ("script", "array"),
    ("path", "string"),
    ("mode", "integer"),
    ("deadline_ms", "integer"),
    ("field", "string"),
    ("protocol", "integer"),
    ("first", "string"),
    ("reason", "string"),
    ("source", "string"),
    ("bytes_hex", "string"),
    ("payload_hex", "string"),
    ("uncertain", "bool"),
    ("history", "bool"),
    ("on", "bool"),
    ("index", "integer"),
    ("choice", "string"),
    ("seed", "integer"),
    ("epoch", "integer"),
    ("code", "integer"),
    ("lane", "integer"),
    ("lanes", "array"),
    ("pids", "array"),
    ("connection", "integer"),
    ("field", "string"),
    ("signal", "integer"),
    ("cause_byte", "integer"),
    ("type", "integer"),
    ("version", "integer"),
    ("bytes", "integer"),
    ("size", "object"),
    ("input", "object"),
    ("modes", "object"),
    ("scripted", "object"),
    ("corpus_hex", "array"),
    ("beyond_limit", "array"),
    ("quantity", "string"),
];

fn has_type(v: &Value, ty: &str) -> bool {
    match ty {
        "string" => v.is_string(),
        "bool" => v.is_boolean(),
        "integer" => v.is_u64() || v.is_i64(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        _ => false,
    }
}

/// The fields of each event (`botster_core_contract::Event`): a matcher that names another field matches nothing.
const EVENT_FIELDS: &[(&str, &[&str])] = &[
    ("Completed", &["op", "result"]),
    ("SessionState", &["id", "instance", "state"]),
    ("RouteClosed", &["route", "reason", "route_tag"]),
    ("RouteStalled", &["route"]),
    ("RouteResumed", &["route"]),
    ("SessionWritable", &["id", "instance"]),
    ("ModesChanged", &["id", "instance", "flags"]),
    ("FocusChanged", &["id", "instance", "focused"]),
    ("SizeChanged", &["id", "instance", "size"]),
    ("Activity", &["id", "instance", "source", "at"]),
    ("Silent", &["id", "instance", "since"]),
    ("TitleChanged", &["id", "instance", "title"]),
    ("CwdChanged", &["id", "instance", "cwd"]),
    ("Bell", &["id", "instance", "at"]),
    ("PromptMark", &["id", "instance", "mark", "exit_code", "at"]),
    (
        "Notification",
        &[
            "id",
            "instance",
            "source",
            "title",
            "body",
            "truncated",
            "at",
        ],
    ),
    (
        "ClipboardWrite",
        &[
            "id",
            "instance",
            "selection",
            "contents",
            "total_bytes",
            "reason",
        ],
    ),
    (
        "EventsLost",
        &["id", "instance", "kinds", "tap_dropped_bytes"],
    ),
    ("MetadataChanged", &["id", "instance"]),
    (
        "RouteAdopted",
        &[
            "session",
            "instance",
            "route",
            "route_tag",
            "owner",
            "client",
        ],
    ),
    ("ServiceState", &["id", "state"]),
    ("LaneConnected", &["id", "lane", "epoch"]),
    (
        "LaneEnded",
        &[
            "id",
            "lane",
            "epoch",
            "reason",
            "dropped_unsent_frames",
            "dropped_unsent_bytes",
        ],
    ),
    ("ServiceWritable", &["id", "lane"]),
    ("ServiceReadable", &["id", "lane"]),
];
const CALLS: &[&str] = &[
    "tap_read",
    "get",
    "list",
    "status",
    "terminal_state",
    "features",
    "limits",
    "worker_protocol",
    "adoptable_worker_protocols",
    "worker_protocol_compatibility",
    "cancel",
    "read_page",
    "release",
    "service_report",
    "service_send",
    "service_recv",
    "service_log_tail",
    "diagnostics",
    "terminal_identity",
    "snapshot_formats",
    "release_owner",
    "set_silence_threshold",
    "next_deadline",
];
/// The test controls (`docs/core-testkit-controls.md`): FakeCore's, and the testkit's.
const CONTROLS: &[&str] = &[
    "scheduler_hold",
    "scheduler_order",
    "scheduler_ready",
    "scheduler_deadline",
    "scheduler_delay",
    "scheduler_work_limit",
    "scheduler_poll_batch",
    "route_read_size",
    "queue_overflow",
    "program_write_size",
    "program_write_once",
    "oracle_notification",
    "scheduler_release",
    "process_end_worker",
    "process_end_guardian",
    "process_refuse_spawn",
    "storage_fail_write",
    "wake_spurious",
    "oracle_hyperlinks",
    "oracle_cursor",
    "control_link_stats",
    "input_records_cost",
    "measure_tap_overhead",
    "oracle_modes",
    "oracle_state",
    "fail_next",
    "lose_worker",
    "break_control",
    "descendants",
    "pty_chunk",
    "pty_blocked",
    "pty_input",
    "pty_output",
    "withhold_feature",
    "unavailable_bound",
    "control_queue",
    "revisions",
    "oracle_encode",
    "hold_start",
    "release_start",
    "spawn_record",
    "service_connect",
    "service_exit",
    "service_send_frame",
    "service_extra_connection",
    "service_send_raw",
    "service_connection_closed",
    "service_pids_alive",
    "service_close_lane",
    "service_lane_open",
    "service_read",
    "service_lane_accept",
    "service_unread",
    "service_spawn_record",
    "service_fds",
    "service_guardian_lost",
    "service_payload_alive",
    "service_descendants",
    "service_rlimits",
    "host_uid",
    "oracle_screen",
    "disable_history",
    "payload_alive",
    "snapshot_unsupported_version",
    "oracle_restore",
    "oracle_resume",
    "oracle_resume_every_cut",
    "oracle_graphics",
    "tic_compile",
    "no_spurious_wakes",
    "schedule",
    "withhold_control_link",
    "hold_snapshot",
    "release_snapshot",
    "uncarriable_sequence",
    "fail_handoff",
    "input_blocked",
    "route_gate",
    "pty_accept",
    "pty_fail_after",
    "route_stream_frame",
    "route_stream_consumed",
    "route_stream_written",
    "route_stream_holders",
    "route_stream_closed",
    "deflate_choice",
    "edges_quiet",
    "hold_handoff",
    "release_handoff",
    "oracle_screen_payload",
    "alloc_window",
    "alloc_peak",
    "storage_fail_write",
    "queue_overflow",
    "route_accept",
    "fail_writes",
    "drop_transport",
    "client_close",
    "unknown_close_reason",
    "route_spurious_ready",
    "pty_output_unread",
    "registry_row",
    "hold_start_at",
    "release_start_at",
    "announce_protocol",
    "lose_host",
    "process_identity",
    "host_crash",
    "reuse_pid",
    "bystander_alive",
    "impostor_worker",
    "signals_received",
    "corrupt_registry_row",
    "guardian_exit",
    "worker_alive",
    "wait_worker_exit",
    "fs_directory",
    "fs_chmod",
    "fs_list",
    "oracle_query_reply",
    "signals_sent",
    // core-p4c: the DP-10 and DP-11 measurements of topology T1 (Core A17-3)
    "measure_route_perf",
    "route_hops",
    "hold_worker_report",
    "release_worker_report",
    "held_reports",
    "hold_cleanup_after",
];
/// The controls that name a capability and have no step of their own: `requires.controls` may name them.
const CAPABILITIES: &[&str] = &["worker_file_name"];
const EVENTS: &[&str] = &[
    "Completed",
    "SessionState",
    "RouteClosed",
    "RouteStalled",
    "RouteResumed",
    "SessionWritable",
    "ModesChanged",
    "FocusChanged",
    "SizeChanged",
    "Activity",
    "Silent",
    "TitleChanged",
    "CwdChanged",
    "Bell",
    "PromptMark",
    "Notification",
    "ClipboardWrite",
    "EventsLost",
    "MetadataChanged",
    "RouteAdopted",
    "ServiceState",
    "LaneConnected",
    "LaneEnded",
    "ServiceWritable",
    "ServiceReadable",
];
const OUTPUTS: &[&str] = &[
    "unit",
    "record",
    "end",
    "screen",
    "cursor",
    "modes",
    "capture",
    "facts",
    "resize",
    "input",
    "spawn",
    "service_end",
    "remove_report",
];

/// Variables stand for a number or a string of the run: try a number, then a string.
fn bind_variables(v: &Value, with: &Value) -> Value {
    match v {
        Value::String(s) if s.starts_with('$') => with.clone(),
        Value::Array(a) => Value::Array(a.iter().map(|x| bind_variables(x, with)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), bind_variables(x, with)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn each_step<'a>(steps: &'a [Value], out: &mut Vec<&'a Value>) {
    for s in steps {
        out.push(s);
        for key in ["barrier"] {
            if let Some(b) = s.get(key) {
                each_step(std::slice::from_ref(b), out);
            }
        }
        // The script of a `run_deterministic` statement runs on fresh harnesses: its steps use the same vocabulary.
        if let Some(inner) = s
            .get("run_deterministic")
            .and_then(|d| d.get("steps"))
            .and_then(Value::as_array)
        {
            each_step(inner, out);
        }
    }
}

/// The schema of the events (`schemas/core/event.schema.json`): a matcher may name only the fields that the contract gives (Core 6.2, A2-1).
fn event_schema() -> &'static Value {
    static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(include_str!("../../../schemas/core/event.schema.json")).unwrap()
    })
}

/// A schema node with its `$ref` followed (`#/$defs/Name`).
fn resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    let mut node = node;
    for _ in 0..16 {
        match node.get("$ref").and_then(Value::as_str) {
            Some(r) => match r
                .strip_prefix("#/$defs/")
                .and_then(|n| root["$defs"].get(n))
            {
                Some(next) => node = next,
                None => return node,
            },
            None => return node,
        }
    }
    node
}

/// The alternatives of a node: itself, or each member of its `oneOf` and `anyOf`, followed down.
fn alternatives<'a>(root: &'a Value, node: &'a Value, out: &mut Vec<&'a Value>) {
    let node = resolve(root, node);
    let members: Vec<&Value> = ["oneOf", "anyOf"]
        .iter()
        .filter_map(|k| node.get(*k).and_then(Value::as_array))
        .flatten()
        .collect();
    if members.is_empty() {
        out.push(node);
    }
    for m in members {
        alternatives(root, m, out);
    }
}

/// Every field name of a matcher is a field that the schema node has, at every depth. A matcher operator (`$one_of`, `$array`, `$not`)
/// is followed into its parts, and any other `$` key is a value matcher that names no field. This is what rejects a matcher that
/// puts a field at the wrong depth, such as `facts.input.input`.
fn check_fields(m: &Value, node: &Value, at: &str, errors: &mut Vec<String>) {
    let root = event_schema();
    match m {
        Value::Object(obj) => {
            if let Some(list) = obj.get("$one_of").and_then(Value::as_array) {
                list.iter().for_each(|o| check_fields(o, node, at, errors));
                return;
            }
            if let Some(a) = obj.get("$array").and_then(Value::as_object) {
                let mut alts = vec![];
                alternatives(root, node, &mut alts);
                if let Some(items) = alts.iter().find_map(|a| a.get("items")) {
                    if let Some(each) = a.get("each") {
                        check_fields(each, items, at, errors);
                    }
                }
                return;
            }
            if let Some(inner) = obj.get("$not") {
                check_fields(inner, node, at, errors);
                return;
            }
            if obj.keys().any(|k| k.starts_with('$')) {
                return;
            }
            let mut alts = vec![];
            alternatives(root, node, &mut alts);
            let objects: Vec<&&Value> = alts
                .iter()
                .filter(|a| a.get("properties").is_some())
                .collect();
            if objects.is_empty() {
                return;
            }
            let open = alts.iter().any(|a| {
                a.get("additionalProperties")
                    .is_some_and(|x| x != &json!(false))
                    || (a.get("properties").is_none() && a.get("type") != Some(&json!("string")))
            });
            for (k, sub) in obj {
                let found = objects.iter().find_map(|a| a["properties"].get(k));
                match found {
                    Some(s) => check_fields(sub, s, &format!("{at}.{k}"), errors),
                    None if open => {}
                    None => errors.push(format!("{at}: the contract has no field '{k}' here")),
                }
            }
        }
        Value::Array(items) => {
            let mut alts = vec![];
            alternatives(root, node, &mut alts);
            if let Some(item) = alts.iter().find_map(|a| a.get("items")) {
                items.iter().for_each(|i| check_fields(i, item, at, errors));
            }
        }
        _ => {}
    }
}

fn check_event_fields(name: &str, body: &Value, what: &str, errors: &mut Vec<String>) {
    let root = event_schema();
    let variant = root["oneOf"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["properties"].get(name).is_some()));
    if let Some(variant) = variant {
        check_fields(
            body,
            &variant["properties"][name],
            &format!("{what}: {name}"),
            errors,
        );
    }
}

fn check_event_matcher(m: &Value, what: &str, errors: &mut Vec<String>) {
    if let Some(obj) = m.as_object() {
        if let Some(options) = obj.get("$one_of").and_then(Value::as_array) {
            options
                .iter()
                .for_each(|o| check_event_matcher(o, what, errors));
            return;
        }
        if obj.contains_key("$any") || obj.contains_key("$type") {
            return;
        }
        // A matcher that names a route frame has `frame`; every other one names an event.
        if obj.contains_key("frame") || obj.contains_key("eof") {
            return;
        }
        for (name, body) in obj {
            if !EVENTS.contains(&name.as_str()) {
                errors.push(format!("{what}: '{name}' is not an event"));
            }
            if let (Some(fields), Some(body)) = (
                EVENT_FIELDS.iter().find(|(n, _)| n == name),
                body.as_object(),
            ) {
                for k in body.keys() {
                    if !fields.1.contains(&k.as_str()) {
                        errors.push(format!("{what}: the event {name} has no field '{k}'"));
                    }
                }
            }
            check_event_fields(name, body, what, errors);
            if name == "Completed" {
                if let Some(ok) = body
                    .get("result")
                    .and_then(|r| r.get("ok"))
                    .and_then(Value::as_object)
                {
                    for k in ok.keys() {
                        if !OUTPUTS.contains(&k.as_str()) {
                            errors.push(format!("{what}: '{k}' is not an `ok` value"));
                        }
                    }
                }
            }
        }
    }
}

fn error_code_name(v: &Value) -> Option<&str> {
    match v {
        Value::String(s) => Some(s),
        Value::Object(m) if m.len() == 1 => m.keys().next().map(String::as_str),
        _ => None,
    }
}

#[test]
fn every_core_transcript_uses_the_vocabulary_of_the_driver() {
    // The optional fields are absent from the default value.
    let mut limit_names: Vec<String> = serde_json::to_value(CoreLimits::default())
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    limit_names.extend([
        "max_route_frame_bytes".to_string(),
        "tap_inflight_bytes".to_string(),
    ]);
    let codes = [
        "UnknownSession",
        "UnknownRoute",
        "IdTooLong",
        "IdInUse",
        "WrongState",
        "SessionEnded",
        "MissingWorkerPath",
        "DataDirInUse",
        "StartFailed",
        "WorkerLinkFailed",
        "LaneFull",
        "InvalidInput",
        "PayloadTooLarge",
        "PendingLimit",
        "SessionLimit",
        "RouteLimit",
        "CaptureLimit",
        "ServiceLimit",
        "Cancelled",
        "CursorReadUnsupported",
        "UnknownCapture",
        "PageOutOfRange",
        "Unsupported",
        "InvalidConfig",
        "RegistryFailed",
        "SnapshotTooLarge",
        "BoundUnavailable",
        "Internal",
        "UnknownService",
    ];
    let mut errors: Vec<String> = vec![];
    let transcripts =
        botster_conformance::load_dir(&botster_core_conformance::CORE_TRANSCRIPTS).unwrap();
    println!("{} Core transcripts", transcripts.len());
    for t in transcripts {
        let id = &t.id;
        for c in &t.requires.controls {
            if !CONTROLS.contains(&c.as_str()) && !CAPABILITIES.contains(&c.as_str()) {
                errors.push(format!("{id}: requires an unknown control '{c}'"));
            }
        }
        let mut all = vec![];
        each_step(&t.steps, &mut all);
        let mut limit_sets: Vec<&Value> = all
            .iter()
            .filter_map(|s| s.get("open").and_then(|o| o.get("limits")))
            .collect();
        if let Some(l) = t.setup.get("limits") {
            limit_sets.push(l);
        }
        for l in limit_sets {
            for k in l.as_object().into_iter().flat_map(|m| m.keys()) {
                if !limit_names.contains(k) {
                    errors.push(format!("{id}: '{k}' is not a CoreLimits field"));
                }
            }
        }
        // A limits value that a transcript opens with must be valid (Core 9B): `open` would refuse it before any tested operation. The
        // `open` steps that expect an `InvalidConfig` are the tests of the refusal.
        let mut valid_opens: Vec<&Value> = all
            .iter()
            .filter(|s| s.get("expect_error").is_none())
            .filter_map(|s| s.get("open").and_then(|o| o.get("limits")))
            .collect();
        if t.setup.get("open") != Some(&Value::Bool(false)) {
            valid_opens.extend(t.setup.get("limits"));
        }
        for l in valid_opens {
            // A variable stands for a size that an earlier step bound: 1 MiB stands in for it (a capture of an empty screen is far smaller).
            let l = &bind_variables(l, &json!(1_048_576));
            match serde_json::from_value::<CoreLimits>(l.clone()) {
                Ok(limits) => {
                    if let Err(e) = limits.validate() {
                        errors.push(format!("{id}: open refuses the limits {l}: {e}"));
                    }
                }
                Err(e) => errors.push(format!("{id}: not CoreLimits: {e}")),
            }
        }
        check_inventory(id, &all, &mut errors);
        for (i, step) in all.iter().enumerate() {
            let at = format!("{id} step {i}");
            let Some(obj) = step.as_object() else {
                errors.push(format!("{at}: a step is an object"));
                continue;
            };
            let forms: Vec<&String> = obj
                .keys()
                .filter(|k| !STEP_KEYS.contains(&k.as_str()) || STEPS.contains(&k.as_str()))
                .collect();
            if !forms
                .iter()
                .any(|k| STEPS.contains(&k.as_str()) || ROUTE_STEPS.contains(&k.as_str()))
            {
                errors.push(format!("{at}: no known step form in {forms:?}"));
            }
            if let Some(b) = obj.get("begin") {
                let mut op = b["op"].clone();
                botster_core_conformance::normalize_op(&mut op, "botster-conformance-probe");
                let parsed = [json!(1), json!("x")]
                    .iter()
                    .any(|with| serde_json::from_value::<Op>(bind_variables(&op, with)).is_ok());
                if !parsed {
                    errors.push(format!("{at}: `Op` does not read {}", b["op"]));
                }
                if let Some(program) = b["op"]
                    .pointer("/Create/request/program")
                    .and_then(Value::as_array)
                {
                    for p in program {
                        if serde_json::from_value::<ProgramStep>(p.clone()).is_err() {
                            errors.push(format!("{at}: not a probe step: {p}"));
                        }
                    }
                }
            }
            // The statement steps (design 5.5; Core A5-1, A5-3, A5-4) name what the Stage 1 runner executes: their bodies are checked here.
            if let Some(s) = obj.get("run_suite") {
                let subject_ok = s
                    .get("subject")
                    .and_then(Value::as_str)
                    .is_some_and(|x| ["core", "hub"].contains(&x));
                let harnesses_ok = s
                    .get("harnesses")
                    .and_then(Value::as_array)
                    .is_some_and(|a| {
                        !a.is_empty()
                            && a.iter().all(|h| {
                                ["real_worker_processes", "testkit"]
                                    .contains(&h.as_str().unwrap_or(""))
                            })
                    });
                let under_ok = s.get("under").is_none_or(|u| {
                    u.as_array().is_some_and(|a| {
                        a.iter()
                            .all(|x| ["seeds", "shuttle"].contains(&x.as_str().unwrap_or("")))
                    })
                });
                if !(subject_ok && harnesses_ok && under_ok) {
                    errors.push(format!(
                        "{at}: run_suite names a subject, harnesses and optional `under`: {s}"
                    ));
                }
            }
            if let Some(s) = obj.get("check_crates") {
                let names = |k: &str| {
                    s.get(k)
                        .and_then(Value::as_array)
                        .is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_string))
                };
                if s.get("crate").and_then(Value::as_str).is_none()
                    || !names("not_in")
                    || !names("no_dependency_of")
                {
                    errors.push(format!(
                        "{at}: check_crates names `crate`, `not_in` and `no_dependency_of`: {s}"
                    ));
                }
            }
            if let Some(s) = obj.get("error_codes_reachable") {
                // Every code of 9.3 and A2 is named once, at its timing: scripted sync refusal, edge, limit, a real argument or state, or
                // no scenario (9.3: `Internal`; A2-2: `Cancelled`). Typed outcomes of a write are named too (Core A5-3).
                let mut named: Vec<String> = vec![];
                let items = |k: &str| {
                    s.get(k)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                };
                let code_of = |e: &Value| -> Option<String> {
                    e.as_str()
                        .or_else(|| e.get("code").and_then(Value::as_str))
                        .map(str::to_string)
                };
                for e in items("scripted_sync") {
                    if e.get("op").and_then(Value::as_str).is_none() {
                        errors.push(format!("{at}: a scripted_sync entry names its op: {e}"));
                    }
                    named.extend(code_of(&e));
                }
                for k in [
                    "by_edge",
                    "by_limit",
                    "by_input",
                    "no_scenario",
                    "open_steward",
                ] {
                    for e in items(k) {
                        if k == "by_edge" && (e.get("edge").is_none() || e.get("cause").is_none()) {
                            errors.push(format!(
                                "{at}: a by_edge entry names its edge and its cause: {e}"
                            ));
                        }
                        if (k == "no_scenario" || k == "open_steward") && e.get("because").is_none()
                        {
                            errors.push(format!("{at}: a no_scenario entry says why: {e}"));
                        }
                        named.extend(code_of(&e));
                    }
                }
                for c in &named {
                    if !codes.contains(&c.as_str()) {
                        errors.push(format!("{at}: error_codes_reachable names no code '{c}'"));
                    }
                }
                for c in codes.iter() {
                    if !named.iter().any(|n| n == c) {
                        errors.push(format!("{at}: error_codes_reachable omits the code {c}"));
                    }
                }
                let outcomes = s.get("typed_outcomes").cloned().unwrap_or_default();
                let mut seen: Vec<String> = outcomes
                    .get("reached")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                for r in outcomes
                    .get("real_only_named")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    // A real-only outcome names the real-process test of the replacement map (Core A5-3).
                    if !r
                        .get("test")
                        .and_then(Value::as_str)
                        .is_some_and(|t| t.starts_with("slow:"))
                    {
                        errors.push(format!("{at}: a real-only outcome names a slow test: {r}"));
                    }
                    seen.extend(r.get("outcome").and_then(Value::as_str).map(str::to_string));
                }
                for o in [
                    "Written",
                    "NotWritten",
                    "Partial",
                    "Failed",
                    "Cancelled",
                    "Unknown",
                ] {
                    if !seen.iter().any(|x| x == o) {
                        errors.push(format!(
                            "{at}: error_codes_reachable omits the write outcome {o}"
                        ));
                    }
                }
            }
            if let Some(s) = obj.get("run_deterministic") {
                let steps_ok = s
                    .get("steps")
                    .and_then(Value::as_array)
                    .is_some_and(|a| !a.is_empty());
                let runs_ok = s
                    .get("runs")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n >= 2);
                if !(steps_ok && runs_ok) {
                    errors.push(format!(
                        "{at}: run_deterministic names at least two runs and a script: {s}"
                    ));
                }
            }
            if let Some(s) = obj.get("forbid_events") {
                if s.get("clear") != Some(&json!(true)) {
                    match s.get("events").and_then(Value::as_array) {
                        Some(list) if !list.is_empty() => list
                            .iter()
                            .for_each(|m| check_event_matcher(m, &at, &mut errors)),
                        _ => errors.push(format!(
                            "{at}: forbid_events names event matchers, or `clear`: {s}"
                        )),
                    }
                }
            }
            if let Some(s) = obj.get("tap_drain") {
                if s.get("session").and_then(Value::as_str).is_none() {
                    errors.push(format!("{at}: tap_drain names a session: {s}"));
                }
                if s.get("accounted").is_some_and(|a| !a.is_u64()) {
                    errors.push(format!("{at}: tap_drain `accounted` is a byte count: {s}"));
                }
            }
            if let Some(c) = obj.get("call") {
                let m = c["method"].as_str().unwrap_or_default();
                if !CALLS.contains(&m) {
                    errors.push(format!("{at}: unknown call '{m}'"));
                }
            }
            if let Some(c) = obj
                .get("control")
                .or_else(|| obj.get("await_control").and_then(|a| a.get("control")))
            {
                let op = c["op"].as_str().unwrap_or_default();
                if let Some((_, required, optional)) =
                    CONTROL_ARGS.iter().find(|(n, _, _)| *n == op)
                {
                    for r in required.iter() {
                        if c.get(r).is_none() {
                            errors.push(format!("{at}: control '{op}' needs '{r}'"));
                        }
                    }
                    for (k, v) in c.as_object().into_iter().flat_map(|m| m.iter()) {
                        let variable = v.as_str().is_some_and(|t| t.starts_with('$'));
                        let ty = arg_type(op, k);
                        if let Some(ty) = ty {
                            if !variable && !has_type(v, ty) {
                                errors.push(format!(
                                    "{at}: control '{op}' argument '{k}' is not a {ty}: {v}"
                                ));
                            }
                        }
                    }
                    for k in c.as_object().into_iter().flat_map(|m| m.keys()) {
                        if !["op", "handle"].contains(&k.as_str())
                            && !required.contains(&k.as_str())
                            && !optional.contains(&k.as_str())
                        {
                            errors.push(format!("{at}: control '{op}' has no argument '{k}'"));
                        }
                    }
                }
                if !CONTROLS.contains(&op) {
                    errors.push(format!("{at}: unknown control '{op}'"));
                } else if !t.requires.controls.iter().any(|r| r == op)
                    && t.fake.is_none()
                    && !["pty_output", "pty_input"].contains(&op)
                {
                    errors.push(format!("{at}: control '{op}' is not in requires.controls"));
                }
            }
            if let Some(e) = obj.get("expect_error") {
                match e.get("code").and_then(error_code_name) {
                    Some(n) if codes.contains(&n) => {
                        // A code with data is an object (`{"Unsupported": {..}}`); a code without data is a string (`ErrorCode`).
                        let with_data = [
                            "StartFailed",
                            "InvalidInput",
                            "Unsupported",
                            "InvalidConfig",
                            "RegistryFailed",
                            "BoundUnavailable",
                        ];
                        if with_data.contains(&n) != e["code"].is_object() {
                            errors.push(format!(
                                "{at}: the code {n} has the wrong JSON shape: {}",
                                e["code"]
                            ));
                        }
                    }
                    other => {
                        errors.push(format!("{at}: expect_error names no error code: {other:?}"))
                    }
                }
            }
            for key in ["pump_until", "pump_collect"] {
                if let Some(s) = obj.get(key) {
                    if let Some(ev) = s.get("event") {
                        check_event_matcher(ev, &at, &mut errors);
                    }
                    for ev in s
                        .get("events")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        check_event_matcher(ev, &at, &mut errors);
                    }
                }
            }
            for key in ["expect", "expect_eventually", "expect_quiet_until"] {
                if obj.contains_key("on") {
                    continue;
                }
                if let Some(m) = obj.get(key).filter(|_| {
                    obj.contains_key(key)
                        && !obj.contains_key("call")
                        && !obj.contains_key("control")
                        && !obj.contains_key("pump")
                        && !obj.contains_key("poll_events")
                }) {
                    check_event_matcher(m, &at, &mut errors);
                }
            }
            if let Some(until) = obj.get("until") {
                check_event_matcher(until, &at, &mut errors);
            }
            // A step that expects has `because`: the citation test reads the same rule.
        }
    }
    assert!(
        errors.is_empty(),
        "{} problems:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

/// Every operation of the A2-1 table, with the row of A3-3 (Core A2-1, Core A3-3).
const ROWS: &[&str] = &[
    "Create",
    "Start",
    "Stop",
    "Signal",
    "Remove",
    "StopAll",
    "UpdateMetadata",
    "SetSizePolicy",
    "SetColorProfile",
    "SetOutputTap",
    "ReadScreen",
    "ReadCursor",
    "ReadModeFlags",
    "CaptureSnapshot",
    "ReadFacts",
    "Resize",
    "WriteInput",
    "Detach",
    "AdoptAll",
    "Adopt",
    "SpawnService",
    "StopService",
    "RemoveService",
    "EndEpoch",
    "SetNotificationPolicy",
];
/// The rows whose sync column has `WrongState` (A2-1, A3-3).
const WRONG_STATE_ROWS: &[&str] = &[
    "Start",
    "Stop",
    "Signal",
    "Remove",
    "SetSizePolicy",
    "SetColorProfile",
    "SetOutputTap",
    "ReadScreen",
    "ReadCursor",
    "ReadModeFlags",
    "CaptureSnapshot",
    "ReadFacts",
    "Resize",
    "WriteInput",
    "AdoptAll",
    "Adopt",
    "StopService",
    "RemoveService",
    "EndEpoch",
    "SetNotificationPolicy",
];

/// The name of the operation that a `begin` step names.
fn op_name(step: &Value) -> Option<String> {
    let op = step.get("begin")?.get("op")?;
    match op {
        Value::String(s) => Some(s.clone()),
        Value::Object(m) => m.keys().next().cloned(),
        _ => None,
    }
}

/// The three "every row" ids of A2-1 name their whole table: a row that a transcript leaves out fails here (Core A2-1, A3-3).
fn check_inventory(id: &str, steps: &[&Value], errors: &mut Vec<String>) {
    let refused = |code: &str| -> Vec<String> {
        steps
            .iter()
            .filter(|s| s["expect_error"]["code"] == json!(code))
            .filter_map(|s| op_name(s))
            .collect()
    };
    let missing = |what: &str, want: &[&str], have: &[String]| {
        want.iter()
            .filter(|w| !have.iter().any(|h| h == *w))
            .map(|w| format!("{id}: the row {w} has no {what}"))
            .collect::<Vec<_>>()
    };
    match id {
        "conf::a2_1_every_begin_operation_has_a_row_and_the_documented_ok_value" => {
            let begun: Vec<String> = steps.iter().filter_map(|s| op_name(s)).collect();
            errors.extend(missing("begin", ROWS, &begun));
        }
        "conf::a2_1_not_admitted_state_is_wrong_state_sync_for_every_row" => {
            errors.extend(missing(
                "WrongState case",
                WRONG_STATE_ROWS,
                &refused("WrongState"),
            ));
        }
        "conf::a2_1_sync_errors_allocate_no_op_for_every_row" => {
            errors.extend(missing("PendingLimit case", ROWS, &refused("PendingLimit")));
            // Every row but StopAll (no sync error) and AdoptAll (only WrongState) has an error of its own sync column besides it.
            let others: Vec<String> = steps
                .iter()
                .filter(|s| {
                    let c = &s["expect_error"]["code"];
                    !c.is_null() && c != "PendingLimit" && c != "WrongState"
                })
                .filter_map(|s| op_name(s))
                .collect();
            let want: Vec<&str> = ROWS
                .iter()
                .copied()
                .filter(|r| !["StopAll", "AdoptAll"].contains(r))
                .collect();
            errors.extend(missing("sync error of its own", &want, &others));
        }
        _ => {}
    }
}

#[test]
fn an_argument_type_belongs_to_its_control() {
    // `before` is a bool of scheduler_deadline and a point name of hold_start_at: each control rejects the other's type.
    let ty = |op, arg| arg_type(op, arg).expect("typed");
    assert!(has_type(&json!(true), ty("scheduler_deadline", "before")));
    assert!(!has_type(
        &json!("running"),
        ty("scheduler_deadline", "before")
    ));
    assert!(has_type(&json!("running"), ty("hold_start_at", "before")));
    assert!(!has_type(&json!(true), ty("hold_start_at", "before")));
    // An argument with one type in every control keeps it.
    assert_eq!(arg_type("scheduler_delay", "session"), Some("string"));
    assert_eq!(arg_type("scheduler_delay", "no_such_argument"), None);
}
