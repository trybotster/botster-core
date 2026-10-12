//! The static tables and the generated artifacts: limits, calls, handler kinds, events, errors, manifest and package files.

mod common;
use botster_plugin_contract::prelude::*;
use common::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn artifact(name: &str) -> Value {
    let text = artifact_files()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1;
    serde_json::from_str(&text).unwrap()
}

/// A validator for `#/$defs/<def>` of an artifact that keeps its definitions at the root.
fn def_validator(root: &Value, def: &str) -> jsonschema::Validator {
    let mut schema = json!({"$ref": format!("#/$defs/{def}")});
    schema["$defs"] = root["$defs"].clone();
    jsonschema::validator_for(&schema).expect("schema compiles")
}

#[test]
fn the_frames_artifact_states_lane_type_and_class() {
    let art = artifact("plugin-frames.schema.json");
    let frames = art["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 10);
    let hc = frames.iter().find(|f| f["name"] == "HostCall").unwrap();
    assert_eq!(hc["lane"], "2, or 3 for a bulk-class call");
    assert_eq!(hc["type"], 0x20);
    assert_eq!(hc["class"], "state_changing");
    let hello = frames.iter().find(|f| f["name"] == "Hello").unwrap();
    assert_eq!(hello["lane"], 0);
    assert_eq!(
        art["$id"],
        "urn:botster:schema:plugin:v1:plugin-frames.schema.json"
    );
}

#[test]
fn limits_sample_is_valid_and_ranges_are_enforced() {
    // HP section 15, A2-52.
    let mut l = sample_limits();
    assert!(l.validate(MAX_KEY_REPEAT).is_empty());
    l.echo_deadline_ms = 10;
    assert!(l
        .validate(MAX_KEY_REPEAT)
        .contains(&LimitViolation::OutOfRange {
            name: "echo_deadline_ms",
            value: 10
        }));
    let mut l = sample_limits();
    l.write_client_quiet_ms = 5;
    l.write_max_wait_ms = 4;
    assert!(l.validate(MAX_KEY_REPEAT).iter().any(|v| matches!(
        v,
        LimitViolation::CrossLimit {
            name: "write_max_wait_ms",
            ..
        }
    )));
    let mut l = sample_limits();
    l.max_relay_envelope_bytes = l.frame_bytes + 1;
    assert!(l.validate(MAX_KEY_REPEAT).iter().any(|v| matches!(
        v,
        LimitViolation::CrossLimit {
            name: "max_relay_envelope_bytes",
            ..
        }
    )));
    let mut l = sample_limits();
    l.probe_text = "this is too long".into();
    assert!(l
        .validate(MAX_KEY_REPEAT)
        .contains(&LimitViolation::ProbeText));
    let mut l = sample_limits();
    l.session_port_range = [20000, 20010];
    assert!(l
        .validate(MAX_KEY_REPEAT)
        .contains(&LimitViolation::PortRange));
    let mut l = sample_limits();
    l.probe_width = 9;
    assert!(l.validate(MAX_KEY_REPEAT).iter().any(|v| matches!(
        v,
        LimitViolation::CrossLimit {
            name: "probe_width",
            ..
        }
    )));
}

#[test]
fn every_limit_has_a_hub_toml_key_except_the_two_the_tables_do_not_name() {
    // HP A2-52.
    let none: Vec<&str> = LIMIT_SPECS
        .iter()
        .filter(|s| s.toml.is_none())
        .map(|s| s.name)
        .collect();
    assert_eq!(none, ["probe_width", "host_memory_bytes"]);
    let names: BTreeSet<&str> = LIMIT_SPECS.iter().map(|s| s.name).collect();
    assert_eq!(names.len(), LIMIT_SPECS.len(), "no duplicate limit");
    for n in [
        "frame_bytes",
        "operations_get_rate",
        "mcp_proxy_message_bytes",
        "event_queue_high",
        "net_rate",
        "ice_servers_max",
        "lock_history_max",
        "instruction_budget",
    ] {
        assert!(names.contains(n), "{n}");
    }
}

#[test]
fn hello_limits_carry_every_limit_name() {
    // HP LM-1.
    let v = serde_json::to_value(sample_limits()).unwrap();
    for s in LIMIT_SPECS {
        let optional = [
            "cpu_seconds",
            "open_files",
            "processes",
            "address_space_bytes",
            "stderr_bytes",
            "instruction_budget",
        ];
        if !optional.contains(&s.name) {
            assert!(
                v.get(s.name).is_some(),
                "{} missing from Hello.limits",
                s.name
            );
        }
    }
}

#[test]
fn the_call_list_is_appendix_a() {
    // HP section 24, LN-2.
    assert_eq!(CALLS.len(), 57);
    let names: BTreeSet<&str> = CALLS.iter().map(|c| c.name).collect();
    assert_eq!(names.len(), CALLS.len());
    assert!(find_call("store.put").is_some());
    assert!(
        find_call("sessions.close").is_none(),
        "names dropped from v1 do not exist"
    );
    assert!(find_call("events.on").is_none());
    assert!(find_call("operations.await").is_none());
    assert_eq!(
        find_call("sessions.stop").unwrap().lua_name(),
        "botster.sessions.stop"
    );
}

#[test]
fn contexts_follow_section_24() {
    // HP CA-3, HS-7.
    assert!(allowed_in("log.write", Ctx::Auto));
    for c in CALLS {
        if c.contexts.contains(&Ctx::Auto) {
            assert_eq!(c.name, "log.write", "autonomous is log only");
        }
        assert!(c.contexts.contains(&Ctx::Req), "{}", c.name);
    }
    let boot: BTreeSet<&str> = CALLS
        .iter()
        .filter(|c| c.contexts.contains(&Ctx::Boot))
        .map(|c| c.name)
        .collect();
    let want: BTreeSet<&str> = [
        "config.get",
        "hub.identity",
        "plugins.bindings",
        "store.get",
        "store.query",
        "secrets.get",
        "log.write",
    ]
    .into_iter()
    .collect();
    assert_eq!(boot, want, "the bootstrap set of HS-7");
    assert!(CALLS
        .iter()
        .filter(|c| c.bulk)
        .map(|c| c.name)
        .eq(["log.write"]));
}

#[test]
fn every_listed_failure_code_is_registered_under_its_kind() {
    // HP ER-3, Appendix B plugin-errors.json.
    for c in CALLS {
        for f in c.failures {
            assert!(
                ErrorKind::ALL.iter().any(|k| k.as_str() == f.kind),
                "{}: kind {}",
                c.name,
                f.kind
            );
            for code in f.codes {
                assert!(
                    CODES.iter().any(|s| s.code == *code && s.kind == f.kind),
                    "{}: {}({}) is not in plugin-errors.json",
                    c.name,
                    f.kind,
                    code
                );
            }
        }
    }
    for f in COMMON_FAILURES {
        for code in f.codes {
            assert!(
                CODES.iter().any(|s| s.code == *code && s.kind == f.kind),
                "{code}"
            );
        }
    }
    let mut seen = BTreeSet::new();
    for s in CODES {
        assert!(
            seen.insert((s.kind, s.code)),
            "duplicate {}({})",
            s.kind,
            s.code
        );
    }
}

#[test]
fn wrong_state_reasons_are_closed_per_call() {
    // HP A2-2.
    for (call, _) in WRONG_STATE_REASONS {
        assert!(find_call(call).is_some(), "{call}");
    }
    let r = |c: &str| WRONG_STATE_REASONS.iter().find(|(n, _)| *n == c).unwrap().1;
    assert_eq!(r("worktrees.delete"), ["occupied"]);
    assert_eq!(r("sessions.write"), ["session_ended", "not_running"]);
    assert!(r("sessions.restart").contains(&"environment_not_retained"));
    assert!(error_registry()["wrong_state_reasons"]["sessions.stop"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "no_screen_model"));
}

#[test]
fn every_call_schema_is_a_valid_schema() {
    // HP Appendix B plugin-calls.schema.json.
    let art = artifact("plugin-calls.schema.json");
    for c in art["calls"].as_array().unwrap() {
        for side in ["args", "result"] {
            jsonschema::validator_for(&c[side])
                .unwrap_or_else(|e| panic!("{} {side}: {e}", c["name"]));
        }
    }
}

#[test]
fn arguments_refuse_unknown_fields_and_results_accept_them() {
    // HP CF-1, CA-1, section 18.
    let get = |n: &str, side: &str| {
        jsonschema::validator_for(
            &artifact("plugin-calls.schema.json")["calls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == n)
                .unwrap()[side],
        )
        .unwrap()
    };
    assert!(get("config.get", "args").is_valid(&json!({})));
    assert!(
        !get("config.get", "args").is_valid(&json!({"x": 1})),
        "config.get refuses any argument (CF-1)"
    );
    assert!(get("store.query", "args")
        .is_valid(&json!({"collection": "c", "index": "id", "limit": 10})));
    assert!(
        !get("store.query", "args").is_valid(&json!({"collection": "c", "index": "id"})),
        "limit is required (ST-4)"
    );
    assert!(
        !get("events.emit", "args").is_valid(&json!({"name": "n", "payload": {}, "owner": "x"})),
        "Hub-stamped fields are refused (EV-1)"
    );
    assert!(get("hub.identity", "result")
        .is_valid(&json!({"hub_id": "h", "display_name": "d", "extra": 1})));
}

#[test]
fn null_and_absent_are_distinct_where_the_contract_says_so() {
    // HP WF-4, A2-8, A2-21.
    let u: SessionsUpdateArgs =
        serde_json::from_value(json!({"session": {"hub_id": "h", "session_id": "s"}})).unwrap();
    assert_eq!(u.metadata, None);
    let u: SessionsUpdateArgs = serde_json::from_value(
        json!({"session": {"hub_id": "h", "session_id": "s"}, "metadata": null}),
    )
    .unwrap();
    assert_eq!(u.metadata, Some(None));
    let o: SetOriginArgs = serde_json::from_value(json!({"origin": null})).unwrap();
    assert_eq!(o.origin, WebOrigin::Clear);
    let o: SetOriginArgs = serde_json::from_value(json!({"origin": {"served": true}})).unwrap();
    assert!(matches!(o.origin, WebOrigin::Served(_)));
    let s: SecretsPutArgs = serde_json::from_value(json!({"name": "n", "value": null})).unwrap();
    assert_eq!(s.value, SecretValue::Delete);
    assert!(serde_json::from_value::<SecretsPutArgs>(json!({"name": "n"})).is_err());
}

#[test]
fn every_handler_kind_has_a_row() {
    // HP A2-5.
    assert_eq!(HandlerKind::ALL.len(), 13);
    for k in HandlerKind::ALL {
        assert!(HANDLERS.iter().any(|h| h.kind == k), "{}", k.as_str());
        assert_eq!(serde_json::to_value(k).unwrap(), json!(k.as_str()));
    }
    assert_eq!(HANDLERS.len(), 13);
    assert!(check_descriptor(
        HandlerKind::McpTool,
        &json!({"name": "n", "description": "d", "input_schema": {}})
    )
    .is_ok());
    assert!(check_descriptor(HandlerKind::McpTool, &json!({"name": "n"})).is_err());
    assert!(check_descriptor(HandlerKind::Webhook, &json!({"path": "/x"})).is_ok());
    assert!(check_descriptor(HandlerKind::Timer, &json!({})).is_ok());
    assert!(check_descriptor(HandlerKind::Timer, &json!({"x": 1})).is_err());
}

#[test]
fn the_event_table_names_every_hub_and_terminal_event_of_a2_6() {
    let names: BTreeSet<String> = EVENTS
        .iter()
        .map(|e| format!("{}/{}", e.owner, e.name))
        .collect();
    for n in [
        "hub/session.activity",
        "hub/session.modes",
        "hub/session.size",
        "hub/session.silent",
        "hub/route.ended",
        "hub/plugin.loaded",
        "hub/plugin.unloaded",
        "hub/plugin.failed",
        "hub/config.changed",
        "hub/package.configuration_changed",
        "hub/session_notification",
        "hub/notice",
        "hub/plugin.dependency",
        "terminal/title",
        "terminal/cwd",
        "terminal/bell",
        "terminal/notification",
        "terminal/prompt_mark",
        "terminal/modes",
        "terminal/exit",
        "terminal/output",
        "net/ws.message",
        "net/ws.close",
    ] {
        assert!(names.contains(n), "{n}");
    }
    let prio = |n: &str| {
        EVENTS
            .iter()
            .find(|e| format!("{}/{}", e.owner, e.name) == n)
            .unwrap()
            .priority
    };
    assert_eq!(prio("terminal/exit"), "high");
    assert_eq!(prio("terminal/output"), "low");
    assert_eq!(prio("hub/session.ended"), "high");
}

#[test]
fn operation_states_are_closed_per_kind() {
    // HP A2-4.
    let t = |k: &str| STATE_TABLES.iter().find(|t| t.kind == k).unwrap();
    assert_eq!(
        t("process").states,
        [
            "queued",
            "running",
            "done",
            "failed",
            "cancelled",
            "unknown"
        ]
    );
    assert_eq!(t("plugin_call").states.len(), 7);
    for st in STATE_TABLES {
        assert!(OPERATION_KINDS.contains(&st.kind));
        for term in st.terminal {
            assert!(st.states.contains(term));
        }
    }
    let r: OperationRecord = serde_json::from_value(
        json!({"operation_id": "o", "kind": "future_kind", "state": "weird", "terminal": false}),
    )
    .unwrap();
    assert_eq!(r.kind, "future_kind", "an unknown kind is opaque (CA-16)");
}

#[test]
fn the_manifest_refuses_unknown_keys_and_accepts_schema_hint() {
    // HP MN-1, A2-43.
    let ok = json!({"$schema": "x", "botster_plugin_api": 1, "name": "demo", "version": "0.1.0", "entrypoints": {"service": {"lua": "main.lua"}}});
    let m: Manifest = serde_json::from_value(ok.clone()).unwrap();
    assert_eq!(m.name.0, "demo");
    let mut bad = ok.clone();
    bad["surprise"] = json!(1);
    assert!(serde_json::from_value::<Manifest>(bad).is_err());
    let mut missing = ok;
    missing.as_object_mut().unwrap().remove("name");
    assert!(serde_json::from_value::<Manifest>(missing).is_err());
    let art = artifact("plugin-manifest.schema.json");
    let v = jsonschema::validator_for(&art).unwrap();
    assert!(v.is_valid(&json!({"botster_plugin_api": 1, "name": "demo", "version": "0.1.0", "entrypoints": {"service": {"lua": "main.lua"}}})));
    assert!(!v.is_valid(&json!({"botster_plugin_api": 1, "name": "demo", "version": "0.1.0", "entrypoints": {"service": {"lua": "main.lua"}}, "surprise": 1})));
    assert!(!v.is_valid(&json!({"botster_plugin_api": 2, "name": "demo", "version": "0.1.0", "entrypoints": {"service": {"lua": "main.lua"}}})));
}

#[test]
fn agent_definition_accessory_and_catalog_validate() {
    // HP PK-6, A2-40, A2-44.
    let agent = json!({"id": "a", "label": "A", "command": "claude", "args": ["-p"], "role": "worker", "interaction": "interactive", "lifecycle": "restartable", "context_keys": ["k"], "port_env": ["PORT"], "script_file": "s.sh"});
    serde_json::from_value::<AgentDefinition>(agent.clone()).unwrap();
    assert!(
        jsonschema::validator_for(&artifact("agent-definition.schema.json"))
            .unwrap()
            .is_valid(&agent)
    );
    let mut with_source = agent.clone();
    with_source["source"] = json!("device");
    assert!(
        serde_json::from_value::<AgentDefinition>(with_source).is_err(),
        "`source` is the one field that is not in the file (A2-40)"
    );
    let acc = json!({"id": "dev", "label": "Dev server", "command": "npm", "role": "accessory"});
    serde_json::from_value::<Accessory>(acc.clone()).unwrap();
    assert!(
        jsonschema::validator_for(&artifact("accessory.schema.json"))
            .unwrap()
            .is_valid(&acc)
    );
    assert!(serde_json::from_value::<Accessory>(
        json!({"id": "dev", "label": "x", "command": "npm", "role": "agent"})
    )
    .is_err());
    let cat = json!({"catalog_version": 1, "entries": [{"id": "a-1", "kind": "agent_definition", "name": "n", "description": "d", "source": {"url": "https://x/y.git", "ref": "v1", "path": "p"}}], "ignored": true});
    serde_json::from_value::<Catalog>(cat.clone()).unwrap();
    let v = jsonschema::validator_for(&artifact("catalog.schema.json")).unwrap();
    assert!(v.is_valid(&cat), "an unknown key is ignored (A2-44)");
    let mut bad = cat.clone();
    bad["entries"][0]["id"] = json!("Bad_Id");
    assert!(!v.is_valid(&bad));
    let mut bad = cat;
    bad["catalog_version"] = json!(2);
    assert!(!v.is_valid(&bad));
}

#[test]
fn lua_globals_are_the_binding_list() {
    // HP LH-4; Lua binding section D.
    let g = lua_globals();
    for n in [
        "assert", "tonumber", "select", "string", "utf8", "require", "botster",
    ] {
        assert!(g.contains(&n), "{n}");
    }
    for n in LUA_UNAVAILABLE {
        assert!(!g.contains(n), "{n}");
    }
    let v = lua_host_globals();
    assert_eq!(v["globals"].as_array().unwrap().len(), g.len());
    let names = botster_names();
    assert!(
        names.contains(&"store.put".to_string())
            && names.contains(&"json.null".to_string())
            && names.contains(&"events.on_gap".to_string())
    );
    assert!(!names.contains(&"sessions.close".to_string()));
}

#[test]
fn features_are_distinct_and_selected_by_intersection() {
    // HP HS-4, section 18.
    let f = Features {
        required: vec!["a".into(), "b".into()],
        optional: vec!["b".into(), "c".into()],
    };
    assert_eq!(f.overlap(), ["b"]);
    let offered = vec!["a".to_string(), "c".to_string()];
    assert_eq!(f.missing(&offered), ["b"]);
    assert_eq!(f.selected_optional(&offered), ["c"]);
}

#[test]
fn error_registry_lists_every_kind_and_effect() {
    let r = error_registry();
    assert_eq!(r["kinds"].as_array().unwrap().len(), 11);
    assert_eq!(r["effects"], json!(["none", "committed", "unknown"]));
    assert!(r["codes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["code"] == "revision_conflict" && c["kind"] == "conflict"));
}

// ---- review round 1 ----

fn call_schema(name: &str, side: &str) -> jsonschema::Validator {
    jsonschema::validator_for(
        &artifact("plugin-calls.schema.json")["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()[side],
    )
    .unwrap()
}

fn session_ref() -> Value {
    json!({"hub_id": "h", "session_id": "s"})
}

#[test]
fn rows_have_their_stated_required_fields() {
    // HP SS-1, A2-16, CA-13.
    let v = call_schema("spawn_targets.list", "result");
    let good = json!({"targets": [{"id": "t", "label": "L", "enabled": true, "kind": "git", "root": "r", "metadata": {}, "git": {"repo": "x"}}]});
    assert!(v.is_valid(&good));
    for bad in [
        json!({"targets": [{"id": 1}]}),
        json!({"targets": [{}]}),
        json!({"targets": [{"id": "t", "label": "L", "enabled": true, "kind": "svn", "root": "r", "metadata": {}}]}),
    ] {
        assert!(!v.is_valid(&bad), "{bad}");
    }
    assert!(serde_json::from_value::<SpawnTargetRow>(json!({})).is_err());
    let w = call_schema("worktrees.show", "result");
    let wt = json!({"worktree": {"worktree_id": "w", "target_id": "t", "label": "L", "path": "p", "status": "ready", "metadata": {}}});
    assert!(w.is_valid(&wt));
    let mut bad = wt.clone();
    bad["worktree"]["status"] = json!("weird");
    assert!(!w.is_valid(&bad));
    assert!(serde_json::from_value::<WorktreeRow>(bad["worktree"].clone()).is_err());
    let st = call_schema("session_types.list", "result");
    let row = json!({"id": "a", "source": "device", "label": "A", "description": "d", "editable": true, "targets": []});
    assert!(st.is_valid(&json!({"session_types": [row.clone()]})));
    assert!(st.is_valid(&json!({"session_types": [{"id": "a", "source": {"repo": {"target_id": "t"}}, "label": "A", "description": "d", "editable": false, "targets": [], "context_keys": ["k"]}]})));
    assert!(!st.is_valid(&json!({"session_types": [{"id": "a"}]})));
    let sr = call_schema("sessions.get", "result");
    let mut row = session_row();
    assert!(sr.is_valid(&row));
    row["owner"] = json!("p");
    row["port"] = json!(4000);
    row["future_field"] = json!(1);
    assert!(sr.is_valid(&row));
    row["state"] = json!("zombie");
    assert!(!sr.is_valid(&row));
}

#[test]
fn each_admission_has_its_own_shape() {
    // HP CA-14, CA-16, A2-32.
    let spawn = json!({"operation_id": "o", "kind": "spawn", "session": session_ref(), "state": "starting", "terminal": false});
    let http = json!({"operation_id": "o", "kind": "http", "state": "queued", "terminal": false});
    let done = json!({"operation_id": "x", "kind": "http", "state": "done", "terminal": true});
    assert!(call_schema("session_types.spawn", "result").is_valid(&spawn));
    assert!(
        !call_schema("session_types.spawn", "result").is_valid(&done),
        "a terminal http record is not a spawn answer"
    );
    assert!(!call_schema("session_types.spawn", "result").is_valid(&http));
    assert!(call_schema("http.request", "result").is_valid(&http));
    assert!(!call_schema("http.request", "result").is_valid(&spawn));
    assert!(!call_schema("http.request", "result").is_valid(
        &json!({"operation_id": "o", "kind": "http", "state": "queued", "terminal": true})
    ));
    let mut sw = http.clone();
    sw["kind"] = json!("safe_write");
    assert!(call_schema("sessions.safe_write", "result").is_valid(&sw));
    assert!(serde_json::from_value::<SpawnAdmission>(spawn).is_ok());
    assert!(serde_json::from_value::<SpawnAdmission>(done).is_err());
    for (call, kind) in [
        ("ws.open", "ws"),
        ("process.run", "process"),
        ("plugins.call", "plugin_call"),
    ] {
        let mut v = http.clone();
        v["kind"] = json!(kind);
        assert!(call_schema(call, "result").is_valid(&v), "{call}");
        assert!(!call_schema(call, "result").is_valid(&http) || kind == "http");
    }
}

#[test]
fn known_terminal_operation_records_need_their_fields() {
    // HP CA-16, PX-2, NT-3, NT-4, PC-1, A2-4; an unknown kind or state stays opaque.
    let v = call_schema("operations.get", "result");
    let base = |kind: &str, state: &str, terminal: bool| json!({"operation_id": "o", "kind": kind, "state": state, "terminal": terminal});
    let process_done = base("process", "done", true);
    assert!(!v.is_valid(&process_done));
    assert!(serde_json::from_value::<OperationRecord>(process_done.clone()).is_err());
    let mut full = process_done;
    full["stdout_tail_b64"] = json!("");
    full["stderr_tail_b64"] = json!("");
    full["truncated"] = json!({"stdout": false, "stderr": false});
    full["duration_ms"] = json!(5);
    assert!(v.is_valid(&full));
    assert!(serde_json::from_value::<OperationRecord>(full).is_ok());
    assert!(!v.is_valid(&base("http", "done", true)));
    assert!(
        !v.is_valid(&base("http", "failed", true)),
        "a failure carries `error`"
    );
    assert!(!v.is_valid(&base("ws", "open", true)));
    assert!(!v.is_valid(&base("plugin_call", "done", true)));
    assert!(
        !v.is_valid(&base("spawn", "failed", true)),
        "failed{{reason}}"
    );
    assert!(
        !v.is_valid(&base("http", "queued", true)),
        "terminal must follow the state table"
    );
    assert!(!v.is_valid(&base("process", "done", false)));
    let mut sw = base("safe_write", "abandoned", true);
    sw["reason"] = json!("no_echo");
    sw["probe_written"] = json!(true);
    assert!(
        !v.is_valid(&sw),
        "a record that wrote the probe reports residue (SW-4)"
    );
    sw["residue"] = json!("possible");
    assert!(!v.is_valid(&sw), "and text_written_bytes (SW-4)");
    assert!(serde_json::from_value::<OperationRecord>(sw.clone()).is_err());
    sw["text_written_bytes"] = json!(0);
    assert!(v.is_valid(&sw));
    let mut delivered = base("safe_write", "delivered", true);
    delivered["probe_written"] = json!(true);
    delivered["residue"] = json!("none");
    assert!(
        !v.is_valid(&delivered),
        "a delivered write that wrote the probe still needs text_written_bytes"
    );
    delivered["text_written_bytes"] = json!(5);
    assert!(v.is_valid(&delivered));
    // An http record carries the request's effect (CA-16).
    let mut http = base("http", "done", true);
    http["status"] = json!(200);
    http["headers"] = json!([["a", "b"]]);
    http["body_b64"] = json!("");
    assert!(!v.is_valid(&http));
    assert!(serde_json::from_value::<OperationRecord>(http.clone()).is_err());
    http["effect"] = json!("committed");
    assert!(v.is_valid(&http));
    assert!(serde_json::from_value::<OperationRecord>(http).is_ok());
    // Reader behavior for unknown kinds and states.
    assert!(v.is_valid(&base("future_kind", "weird", false)));
    assert!(v.is_valid(&base("http", "future_state", false)));
    assert!(serde_json::from_value::<OperationRecord>(base("future_kind", "weird", false)).is_ok());
}

#[test]
fn relay_deliver_effect_follows_a2_18() {
    let c = find_call("relay.deliver").unwrap();
    assert!(c.mutating, "an accepted envelope is a committed effect");
    assert!(c.commit.contains("accepted: committed") && c.commit.contains("rejected: none"));
    let art = artifact("plugin-calls.schema.json");
    let row = art["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "relay.deliver")
        .unwrap();
    assert_eq!(row["mutating"], true);
}

#[test]
fn structural_constraints_of_arguments() {
    // HP PX-1, CA-19, NT-4, WP-1.
    let cmd = call_schema("sessions.create_command", "args");
    assert!(cmd.is_valid(&json!({"argv": ["ls"], "cwd": {"target_id": "t", "relative": "x"}})));
    assert!(cmd.is_valid(&json!({"argv": ["ls"], "cwd": {"worktree_id": "w"}})));
    assert!(!cmd.is_valid(&json!({"argv": ["ls"], "cwd": {}})));
    assert!(!cmd.is_valid(&json!({"argv": ["ls"], "cwd": {"target_id": "t", "worktree_id": "w"}})));
    assert!(serde_json::from_value::<CwdRef>(json!({})).is_err());
    assert!(
        serde_json::from_value::<CwdRef>(json!({"target_id": "t", "worktree_id": "w"})).is_err()
    );
    let send = call_schema("ws.send", "args");
    assert!(send.is_valid(&json!({"ws_id": "w", "text": "hi"})));
    assert!(send.is_valid(&json!({"ws_id": "w", "data_b64": "aGk="})));
    assert!(!send.is_valid(&json!({"ws_id": "w"})));
    assert!(!send.is_valid(&json!({"ws_id": "w", "text": "a", "data_b64": "b"})));
    assert!(serde_json::from_value::<WsSendArgs>(json!({"ws_id": "w"})).is_err());
    assert!(serde_json::from_value::<WsSendArgs>(
        json!({"ws_id": "w", "text": "a", "data_b64": "b"})
    )
    .is_err());
    let origin = call_schema("web.set_origin", "args");
    for ok in [
        json!({"origin": null}),
        json!({"origin": "https://x.example"}),
        json!({"origin": {"served": true}}),
    ] {
        assert!(origin.is_valid(&ok), "{ok}");
    }
    assert!(!origin.is_valid(&json!({"origin": {"served": false}})));
    assert!(!origin.is_valid(&json!({"origin": 5})));
    assert!(serde_json::from_value::<SetOriginArgs>(json!({"origin": {"served": false}})).is_err());
    assert!(call_schema("secrets.put", "args").is_valid(&json!({"name": "n", "value": null})));
    assert!(!call_schema("secrets.put", "args").is_valid(&json!({"name": "n", "value": 3})));
    let msg = &artifact("plugin-calls.schema.json")["events"];
    let ws = msg
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "ws.message")
        .unwrap();
    let v = jsonschema::validator_for(&ws["payload"]).unwrap();
    assert!(v.is_valid(&json!({"ws_id": "w", "text": "t"})));
    assert!(!v.is_valid(&json!({"ws_id": "w"})));
    assert!(!v.is_valid(&json!({"ws_id": "w", "text": "t", "data_b64": "d"})));
}

#[test]
fn null_is_refused_where_the_contract_does_not_allow_it() {
    // HP WF-4, ST-2.
    let put = call_schema("store.put", "args");
    assert!(put.is_valid(&json!({"collection": "c", "id": "i", "doc": {}})));
    assert!(
        put.is_valid(&json!({"collection": "c", "id": "i", "doc": {}, "expected_revision": "r"}))
    );
    assert!(
        !put.is_valid(&json!({"collection": "c", "id": "i", "doc": {}, "expected_revision": null}))
    );
    assert!(serde_json::from_value::<StorePutArgs>(
        json!({"collection": "c", "id": "i", "doc": {}, "expected_revision": null})
    )
    .is_err());
    let a: StorePutArgs =
        serde_json::from_value(json!({"collection": "c", "id": "i", "doc": {}})).unwrap();
    assert_eq!(a.expected_revision, None);
    // Explicit null stays valid where a clause says so (A2-21, A2-8).
    let upd = call_schema("sessions.update", "args");
    assert!(upd.is_valid(&json!({"session": session_ref(), "metadata": null})));
    assert!(!upd.is_valid(&json!({"session": session_ref(), "label": null})));
    assert!(serde_json::from_value::<SessionsUpdateArgs>(
        json!({"session": session_ref(), "label": null})
    )
    .is_err());
    // Other optional fields.
    assert!(serde_json::from_value::<HttpRequestArgs>(
        json!({"method": "GET", "url": "https://x", "timeout_ms": 1, "headers": null})
    )
    .is_err());
    assert!(!call_schema("store.query", "args")
        .is_valid(&json!({"collection": "c", "index": "i", "limit": 1, "cursor": null})));
    assert!(
        serde_json::from_value::<Capability>(json!({"surface": "net", "scope": null})).is_err()
    );
}

#[test]
fn counters_use_the_wire_integer_form() {
    // HP WF-3: 2^53 is a number; 2^53 + 1 is a decimal string.
    let v = call_schema("log.write", "result");
    for (input, ok) in [
        (json!({"seq": 9007199254740992u64}), true),
        (json!({"seq": "9007199254740993"}), true),
        (json!({"seq": 9007199254740993u64}), false),
        (json!({"seq": -1}), false),
        (json!({"seq": "x"}), false),
    ] {
        assert_eq!(v.is_valid(&input), ok, "{input}");
        assert_eq!(
            serde_json::from_value::<LogWriteResult>(input.clone()).is_ok(),
            ok,
            "{input}"
        );
    }
    let big = LogWriteResult {
        seq: WireU64((1 << 53) + 1),
        dropped_before: Some(WireU64(1 << 53)),
    };
    let j = serde_json::to_value(&big).unwrap();
    assert_eq!(
        j,
        json!({"seq": "9007199254740993", "dropped_before": 9007199254740992u64})
    );
    assert_eq!(serde_json::from_value::<LogWriteResult>(j).unwrap(), big);
    // Through the frame codec.
    let f = Frame::HostResult(HostResult {
        epoch: Epoch(1),
        call_id: WireId::Num(1),
        result: PluginResult::Ok(serde_json::to_value(&big).unwrap()),
        effect: Effect::None,
    });
    let bytes = encode(Lane::Bulk, &f, 1 << 20).unwrap();
    assert!(matches!(
        decode(&bytes, 1 << 20, None).unwrap(),
        Decoded::Frame { .. }
    ));
    let gap = serde_json::from_value::<EventGapRecord>(json!({"gap_id": "g", "owner": "o", "name": "n", "count": "9007199254740993", "lost_bytes": 9007199254740992u64, "recovery": "partial"})).unwrap();
    assert_eq!(gap.count, WireU64((1 << 53) + 1));
}

#[test]
fn manifest_value_constraints() {
    // HP MP-1, MN-1, PD-1, A2-19.
    let base = |extra: Value| {
        let mut m = json!({"botster_plugin_api": 1, "name": "demo", "version": "0.1.0", "entrypoints": {"service": {"lua": "m.lua"}}});
        for (k, v) in extra.as_object().unwrap() {
            m[k] = v.clone();
        }
        m
    };
    let schema = jsonschema::validator_for(&artifact("plugin-manifest.schema.json")).unwrap();
    let check = |m: Value, ok: bool| {
        assert_eq!(schema.is_valid(&m), ok, "schema: {m}");
        assert_eq!(
            serde_json::from_value::<Manifest>(m.clone()).is_ok(),
            ok,
            "decoder: {m}"
        );
    };
    check(base(json!({})), true);
    check(
        base(json!({"mcp_servers": [{"id": "s", "transport": "stdio", "command": "c"}]})),
        true,
    );
    check(
        base(json!({"mcp_servers": [{"id": "s", "transport": "http", "url": "https://x"}]})),
        true,
    );
    check(
        base(json!({"mcp_servers": [{"id": "s", "transport": "stdio"}]})),
        false,
    );
    check(
        base(json!({"mcp_servers": [{"id": "s", "transport": "http"}]})),
        false,
    );
    check(
        base(
            json!({"mcp_servers": [{"id": "s", "transport": "stdio", "command": "c", "url": "https://x"}]}),
        ),
        false,
    );
    check(
        base(json!({"mcp_servers": [{"id": "s", "transport": "ftp", "command": "c"}]})),
        false,
    );
    check(base(json!({"fs_roots": [{"id": "r", "read": true}]})), true);
    check(
        base(json!({"fs_roots": [{"id": "r", "read": false}]})),
        false,
    );
    for bad in ["Demo", "de mo", "de@mo", "", "de_mo"] {
        check(
            json!({"botster_plugin_api": 1, "name": bad, "version": "0.1.0", "entrypoints": {"service": {"lua": "m.lua"}}}),
            false,
        );
    }
    for (version, ok) in [
        ("1.2.3", true),
        ("1.2.3-rc.1+build.5", true),
        ("1.2", false),
        ("01.2.3", false),
        ("v1.2.3", false),
        ("1.2.3.4", false),
        ("1.2.3-", false),
    ] {
        check(
            json!({"botster_plugin_api": 1, "name": "demo", "version": version, "entrypoints": {"service": {"lua": "m.lua"}}}),
            ok,
        );
    }
    check(
        base(json!({"assets": [{"name": "p", "path": "p.html", "content_type": "text/html"}]})),
        true,
    );
    check(
        base(
            json!({"assets": [{"name": "p", "path": "p.exe", "content_type": "application/x-msdownload"}]}),
        ),
        false,
    );
    check(
        base(json!({"capabilities": [{"surface": "net", "scope": "net", "allow": ["https://x"]}]})),
        true,
    );
    check(
        base(
            json!({"capabilities": [{"surface": "events", "scope": "events_hub", "allow": ["https://x"]}]}),
        ),
        false,
    );
    check(
        base(json!({"provides": [{"interface": "x@1.0.0", "exports": []}]})),
        true,
    );
    check(
        base(json!({"provides": [{"interface": "x", "exports": []}]})),
        false,
    );
    let publish = call_schema("assets.publish", "args");
    assert!(
        publish.is_valid(&json!({"name": "n", "content_type": "image/svg+xml", "bytes_b64": ""}))
    );
    assert!(
        !publish.is_valid(&json!({"name": "n", "content_type": "text/x-evil", "bytes_b64": ""}))
    );
    assert!(serde_json::from_value::<AssetsPublishArgs>(
        json!({"name": "n", "content_type": "text/x-evil", "bytes_b64": ""})
    )
    .is_err());
}

#[test]
fn the_result_schema_and_decoder_agree() {
    // HP section 18, VR-3: descriptive extension fields are accepted; `error` in a success and `value` in a failure are refused.
    let art = artifact("plugin-frames.schema.json");
    let v = def_validator(&art, "PluginResult");
    let err = json!({"kind": "failed", "message": "m", "retryable": false});
    let cases = [
        (json!({"ok": true, "value": {}}), true),
        (
            json!({"ok": true, "value": {}, "description": "future"}),
            true,
        ),
        (json!({"ok": false, "error": err, "future": 1}), true),
        (json!({"ok": true, "value": {}, "error": err}), false),
        (json!({"ok": false, "error": err, "value": 1}), false),
        (json!({"ok": true}), false),
        (json!({"ok": false}), false),
    ];
    for (input, ok) in cases {
        assert_eq!(v.is_valid(&input), ok, "schema: {input}");
        assert_eq!(
            serde_json::from_value::<PluginResult>(input.clone()).is_ok(),
            ok,
            "decoder: {input}"
        );
    }
}

#[test]
fn an_unmet_optional_binding_has_provider_null() {
    // HP HS-1, PD-5, PD-8, A2-25.
    let unmet = HelloBinding {
        interface: "i".into(),
        interface_version: "1.0.0".into(),
        provider: None,
        provider_version: None,
        interface_hash: None,
    };
    let j = serde_json::to_value(&unmet).unwrap();
    assert_eq!(
        j["provider"],
        Value::Null,
        "an unmet binding is listed with provider: null"
    );
    assert_eq!(serde_json::from_value::<HelloBinding>(j).unwrap(), unmet);
    // An absent provider is accepted as well (A2-25).
    assert_eq!(
        serde_json::from_value::<HelloBinding>(
            json!({"interface": "i", "interface_version": "1.0.0"})
        )
        .unwrap(),
        unmet
    );
    // Through the Hello frame: schema, serde and framing.
    let mut h = hello();
    h.bindings = vec![unmet.clone()];
    let payload = serde_json::to_value(&h).unwrap();
    let art = artifact("plugin-frames.schema.json");
    assert!(def_validator(&art, "Hello").is_valid(&payload));
    let bytes = encode(Lane::Control, &Frame::Hello(h.clone()), 1 << 20).unwrap();
    assert!(
        matches!(decode(&bytes, 1 << 20, None).unwrap(), Decoded::Frame { frame: Frame::Hello(got), .. } if got == h)
    );
    // A provider that is bound is a string; a number is refused.
    let mut bad = payload.clone();
    bad["bindings"][0]["provider"] = json!(5);
    assert!(!def_validator(&art, "Hello").is_valid(&bad));
    // plugins.bindings answers the same way.
    let v = call_schema("plugins.bindings", "result");
    assert!(v.is_valid(&json!({"bindings": [{"interface": "i", "interface_version": "1.0.0", "provider": null, "state": "unmet"}]})));
    let r: BindingsResult = serde_json::from_value(json!({"bindings": [{"interface": "i", "interface_version": "1.0.0", "provider": null, "state": "unmet"}]})).unwrap();
    assert_eq!(r.bindings[0].provider, None);
}

#[test]
fn cwd_relative_refuses_null_in_both_branches() {
    // HP WF-4, PX-1, CA-19.
    let cmd = call_schema("sessions.create_command", "args");
    for cwd in [
        json!({"target_id": "t", "relative": null}),
        json!({"worktree_id": "w", "relative": null}),
    ] {
        assert!(
            !cmd.is_valid(&json!({"argv": ["ls"], "cwd": cwd.clone()})),
            "schema: {cwd}"
        );
        assert!(
            serde_json::from_value::<CwdRef>(cwd.clone()).is_err(),
            "decoder: {cwd}"
        );
    }
    assert!(serde_json::from_value::<CwdRef>(json!({"target_id": "t", "relative": "x"})).is_ok());
}

fn session_row() -> Value {
    json!({
        "id": "s", "hub_id": "h", "label": "L", "state": "running", "lifecycle_class": "current", "exit": {},
        "size": {"rows": 24, "cols": 80}, "session_type": {"id": "t", "source": "device"}, "traits": [], "restartable": false,
        "activity": "idle", "presentation": "default", "metadata": {}
    })
}

#[test]
fn the_session_row_follows_r11() {
    // Steward ruling R-11 (HC section 6, A.8, A.1): the thirteen required fields, the optional ones absent when empty, never null.
    let sr = call_schema("sessions.get", "result");
    let good = session_row();
    assert!(sr.is_valid(&good));
    assert!(serde_json::from_value::<SessionRow>(good.clone()).is_ok());
    for field in [
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
    ] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(!sr.is_valid(&bad), "schema: {field} is required");
        assert!(
            serde_json::from_value::<SessionRow>(bad).is_err(),
            "decoder: {field} is required"
        );
    }
    // Optional fields are absent when there is no value; null is refused.
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
        let mut bad = good.clone();
        bad[field] = Value::Null;
        assert!(!sr.is_valid(&bad), "schema: {field} is never null");
        assert!(
            serde_json::from_value::<SessionRow>(bad).is_err(),
            "decoder: {field} is never null"
        );
    }
    for field in ["label", "exit", "metadata", "traits"] {
        let mut bad = good.clone();
        bad[field] = Value::Null;
        assert!(!sr.is_valid(&bad), "schema: {field} is not null");
        assert!(serde_json::from_value::<SessionRow>(bad).is_err());
    }
    // exit is {} until the process ends, then code and/or signal as integers, each absent when unknown.
    let mut ended = good.clone();
    ended["exit"] = json!({"code": 0});
    assert!(sr.is_valid(&ended));
    ended["exit"] = json!({"signal": 9});
    assert!(sr.is_valid(&ended));
    ended["exit"] = json!({"code": 1, "signal": 15});
    assert!(sr.is_valid(&ended));
    ended["exit"] = json!({"signal": "SIGKILL"});
    assert!(!sr.is_valid(&ended), "signal is an integer (A.8)");
    ended["exit"] = json!({"code": null});
    assert!(!sr.is_valid(&ended));
    // attention{since, source: bell|osc9|osc777, title?, body?}.
    let mut att = good.clone();
    att["attention"] =
        json!({"since": 1700000000000u64, "source": "osc9", "title": "t", "body": "b"});
    assert!(sr.is_valid(&att));
    assert!(serde_json::from_value::<SessionRow>(att.clone()).is_ok());
    for bad in [
        json!({"source": "bell"}),
        json!({"since": 1}),
        json!({"since": 1, "source": "osc99"}),
        json!({"since": 1, "source": "bell", "title": null}),
    ] {
        att["attention"] = bad.clone();
        assert!(!sr.is_valid(&att), "schema: {bad}");
        assert!(
            serde_json::from_value::<SessionRow>(att.clone()).is_err(),
            "decoder: {bad}"
        );
    }
    // Descriptive extension fields are ignored, not refused (HP section 18, VR-2).
    let mut ext = good.clone();
    ext["attention"] = json!({"since": 1, "source": "bell", "future": 1});
    ext["exit"] = json!({"code": 0, "future": true});
    assert!(
        sr.is_valid(&ext),
        "schema accepts extension fields in attention and exit"
    );
    assert!(serde_json::from_value::<SessionRow>(ext).is_ok());
    // The session family frames carry the same row (A2-28).
    let chunk = json!({"frame": "snapshot_chunk", "sessions": [good]});
    assert!(serde_json::from_value::<SessionFamilyFrame>(chunk).is_ok());
}

// HP A2-40, CA-20, LB-19: the legacy `port_forward` of an accessory is `allocate_port`, one to one. The accessory file accepts it and the schema lists it.
#[test]
fn the_legacy_port_forward_marker_maps_to_allocate_port_one_to_one() {
    use botster_plugin_contract::prelude::*;
    let base = serde_json::json!({"id": "dev", "label": "Dev", "command": "npm"});
    let with = |v: serde_json::Value| {
        let mut o = base.clone();
        o["port_forward"] = v;
        serde_json::from_value::<Accessory>(o).unwrap()
    };
    assert!(with(serde_json::json!(true)).allocate_port());
    assert!(!with(serde_json::json!(false)).allocate_port());
    assert!(!serde_json::from_value::<Accessory>(base)
        .unwrap()
        .allocate_port());
    assert!(serde_json::from_value::<Accessory>(
        serde_json::json!({"id": "d", "label": "D", "command": "c", "port_forward": "yes"})
    )
    .is_err());
    let schema = artifact_files()
        .into_iter()
        .find(|(n, _)| *n == "accessory.schema.json")
        .unwrap()
        .1;
    let schema: serde_json::Value = serde_json::from_str(&schema).unwrap();
    assert_eq!(schema["properties"]["port_forward"]["type"], "boolean");
}

fn handler_result_schema(kind: &str) -> jsonschema::Validator {
    jsonschema::validator_for(
        &artifact("plugin-calls.schema.json")["handler_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["kind"] == kind)
            .unwrap()["result"],
    )
    .unwrap()
}

// HP A5-1, A5-2: an `entity_provider` record is `{id, entity}` in the type and in the generated Appendix B artifact.
#[test]
fn an_entity_provider_record_is_id_and_entity() {
    let schema = handler_result_schema("entity_provider");
    let ok = json!({"records": [{"id": "r1", "entity": {"title": "t", "items": [{"id": "nested", "label": "l"}]}}], "cursor": "c"});
    let parsed: EntityProviderResult = serde_json::from_value(ok.clone()).unwrap();
    assert_eq!(parsed.records[0].id, "r1");
    assert_eq!(serde_json::to_value(&parsed).unwrap(), ok);
    assert!(schema.is_valid(&ok));
    for bad in [
        json!({"records": [{"title": "flat"}]}),
        json!({"records": [{"id": 1, "entity": {}}]}),
        json!({"records": [{"id": "r1", "entity": "body"}]}),
        json!({"records": [{"id": "r1"}]}),
    ] {
        assert!(
            serde_json::from_value::<EntityProviderResult>(bad.clone()).is_err(),
            "{bad}"
        );
        assert!(!schema.is_valid(&bad), "{bad}");
    }
}

// HP A5-1: `entity.publish` carries the same pair; its `entity` is a JSON object.
#[test]
fn entity_publish_carries_an_object_entity() {
    let args = call_schema("entity.publish", "args");
    let ok = json!({"entity_type": "card", "type": "upsert", "id": "r1", "entity": {"title": "t"}});
    assert!(serde_json::from_value::<EntityPublishArgs>(ok.clone()).is_ok());
    assert!(args.is_valid(&ok));
    let bad = json!({"entity_type": "card", "type": "upsert", "id": "r1", "entity": "t"});
    assert!(serde_json::from_value::<EntityPublishArgs>(bad.clone()).is_err());
    assert!(!args.is_valid(&bad));
}

// HP A4-1, A8-2, A8-3: the codes these amendments publish are registered under their kinds.
#[test]
fn the_amendment_codes_are_registered() {
    for (kind, code) in [
        ("backpressured", "write_pending_full"),
        ("invalid_request", "interface_not_additive"),
        ("conflict", "breaking_not_accepted"),
    ] {
        assert!(
            CODES.iter().any(|s| s.kind == kind && s.code == code),
            "{kind}({code})"
        );
    }
    let safe_write = CALLS
        .iter()
        .find(|c| c.name == "sessions.safe_write")
        .unwrap();
    assert!(safe_write
        .failures
        .iter()
        .any(|f| f.kind == "backpressured"
            && f.codes == ["no_receipt_capacity", "write_pending_full"]));
}

#[test]
fn limits_bounds_are_inclusive_and_violations_name_both_limits() {
    // HP section 15, A2-52: each bound value is accepted, and one past it is refused.
    let mut l = sample_limits();
    assert_eq!(l.number("echo_deadline_ms"), Some(l.echo_deadline_ms));
    assert_eq!(l.number("probe_text"), None);
    l.echo_deadline_ms = 10_000;
    l.cleanup_window_ms = 10_000;
    l.write_max_wait_ms = l.write_client_quiet_ms;
    l.probe_width = MAX_KEY_REPEAT;
    l.max_relay_envelope_bytes = l.frame_bytes;
    assert_eq!(l.number("echo_deadline_ms"), Some(10_000));
    assert_eq!(l.validate(MAX_KEY_REPEAT), vec![]);
    l.echo_deadline_ms = 10_001;
    l.cleanup_window_ms = 10_001;
    assert_eq!(
        l.validate(MAX_KEY_REPEAT),
        vec![LimitViolation::OutOfRange {
            name: "echo_deadline_ms",
            value: 10_001
        }]
    );
    let mut l = sample_limits();
    l.cleanup_window_ms = l.echo_deadline_ms - 1;
    assert_eq!(
        l.validate(MAX_KEY_REPEAT),
        vec![LimitViolation::CrossLimit {
            name: "cleanup_window_ms",
            other: "echo_deadline_ms"
        }]
    );
    let mut l = sample_limits();
    l.write_client_quiet_ms = 5;
    l.write_max_wait_ms = 4;
    l.probe_width = MAX_KEY_REPEAT + 1;
    assert_eq!(
        l.validate(MAX_KEY_REPEAT),
        vec![
            LimitViolation::CrossLimit {
                name: "write_max_wait_ms",
                other: "write_client_quiet_ms"
            },
            LimitViolation::CrossLimit {
                name: "probe_width",
                other: "max_key_repeat"
            },
        ]
    );
}

#[test]
fn pd_3_provided_interface_needs_a_valid_name_and_a_plain_version() {
    assert!(ProvidedInterface::is_valid("my-api@1.2.3"));
    assert!(!ProvidedInterface::is_valid("My_Api@1.2.3"));
    assert!(!ProvidedInterface::is_valid("my-api@1.2.3-rc.1"));
    assert!(!ProvidedInterface::is_valid("my-api@1.2.3+build"));
    assert!(!ProvidedInterface::is_valid("my-api@1.2"));
}
