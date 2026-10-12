//! The HP transcripts against the fakes (design 7.1: `plugin_hub` runs against the plugin-side FakeHub, `plugin_host` against FakePlugin). One `#[test]` per transcript id, under the run's seed set.

use botster_plugin_conformance::fake;

botster_plugin_conformance::hub_conformance_tests!(fake::hub);
botster_plugin_conformance::host_conformance_tests!(fake::host);

// Design 5.6, rule 6: every expectation of every HP transcript cites the clause that fixes its value.
#[test]
fn every_hp_expectation_cites_its_clause() {
    let mut missing = vec![];
    for t in botster_conformance::load_dir(&botster_plugin_conformance::HP_TRANSCRIPTS).unwrap() {
        for step in botster_conformance::citation::missing_because(&t) {
            missing.push(format!("{} step {step}", t.id));
        }
    }
    assert!(missing.is_empty(), "no `because`: {missing:#?}");
}

// Design 5.6, rule 6: every cited clause is a clause row of a current HP file (the main contract, A1, A2, A3 or A7).
#[test]
fn every_cited_clause_exists() {
    let files = [
        include_str!("../../../frozen/current/hub-plugin-contract-v1.1.joint-J4e.md"),
        include_str!("../../../frozen/current/hub-plugin-contract-v1.1.amendment-1-candidate5.md"),
        include_str!("../../../frozen/current/hub-plugin-contract-v1.1.amendment-2-candidate7.md"),
        include_str!("../../../frozen/current/hub-plugin-contract-v1.1.amendment-3-candidate1.md"),
        include_str!("../../../frozen/current/hub-plugin-contract-v1.1.amendment-7-candidate3.md"),
    ];
    let has = |id: &str| {
        files.iter().any(|f| {
            f.lines().any(|l| {
                l.starts_with(&format!("| {id} "))
                    || l.starts_with(&format!("| {id} ("))
                    || l.contains(&format!("**{id}"))
            })
        })
    };
    let lua_binding = include_str!("../../../frozen/current/hub-plugin-lua-binding-v1.1-r9.md");
    // Each distinct citation is looked up once: the files are large and the transcripts cite the same clauses thousands of times.
    let mut known: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
    let mut unknown = vec![];
    for t in botster_conformance::load_dir(&botster_plugin_conformance::HP_TRANSCRIPTS).unwrap() {
        for step in &t.steps {
            let Some(b) = step.get("because") else {
                continue;
            };
            let list: Vec<&str> = match b {
                serde_json::Value::String(s) => vec![s.as_str()],
                serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).collect(),
                _ => vec![],
            };
            for c in list {
                let found = *known
                    .entry(c.to_string())
                    .or_insert_with(|| {
                        c.strip_prefix("HP ").is_some_and(has)
                            // A Hub transcript of the Lua binding suite (lb-cli-suite, steward ruling R-23) cites the Lua binding clause that the Hub enforces.
                            || c.strip_prefix("LB ").is_some_and(|id| lua_binding.contains(&format!("**{id} ")))
                    });
                if !found {
                    unknown.push(format!("{}: {c}", t.id));
                }
            }
        }
    }
    assert!(unknown.is_empty(), "{unknown:#?}");
}

// Design 5.6, rules 2 and 4: a semantic call in a transcript has the arguments of Appendix A, and a scripted answer has the result shape of Appendix A (or a failure that the
// call lists), so that a conformant real Hub would accept the request and could give the answer.
#[test]
fn calls_and_scripts_follow_appendix_a() {
    use serde_json::{json, Value};
    let calls_doc: Value = serde_json::from_str(
        &botster_plugin_contract::prelude::artifact_files()
            .into_iter()
            .find(|(n, _)| *n == "plugin-calls.schema.json")
            .unwrap()
            .1,
    )
    .unwrap();
    let table: std::collections::BTreeMap<String, Value> = calls_doc["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["name"].as_str().unwrap().to_string(), c.clone()))
        .collect();
    let common: Vec<Value> = calls_doc["common_failures"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let templates: Value = serde_json::from_str(
        botster_plugin_conformance::HP_TRANSCRIPTS
            .get_file("fixtures/frames.json")
            .unwrap()
            .contents_utf8()
            .unwrap(),
    )
    .unwrap();
    let mut bad = vec![];
    let check = |schema: &Value, v: &Value| -> Result<(), String> {
        let validator = jsonschema::validator_for(schema).map_err(|e| e.to_string())?;
        validator.validate(v).map_err(|e| e.to_string())
    };
    for t in botster_conformance::load_dir(&botster_plugin_conformance::HP_TRANSCRIPTS).unwrap() {
        let steps: Vec<Value> = t
            .steps
            .iter()
            .map(|s| botster_plugin_conformance::expand::expand(s, &templates).unwrap())
            .collect();
        for s in &steps {
            // A step that marks `violates_args` sends arguments that the call refuses (a negative case).
            let Some(frame) = s
                .get("send_frame")
                .filter(|f| f["type"] == "HostCall" && s.get("violates_args").is_none())
            else {
                continue;
            };
            let call = frame["payload"]["call"].as_str().unwrap_or("");
            if let Some(spec) = table.get(call) {
                if let Err(e) = check(&spec["args"], &frame["payload"]["args"]) {
                    bad.push(format!("{}: {call} args: {e}", t.id));
                }
            }
        }
        for (call, entries) in t
            .setup
            .get("calls")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let Some(spec) = table.get(call) else {
                bad.push(format!(
                    "{}: scripted call {call} is not in Appendix A",
                    t.id
                ));
                continue;
            };
            for e in entries.as_array().cloned().unwrap_or_default() {
                let r = &e["result"];
                if r["ok"] == json!(true) {
                    if let Err(m) = check(&spec["result"], &r["value"]) {
                        bad.push(format!("{}: {call} result: {m}", t.id));
                    }
                } else {
                    let (kind, code) = (
                        r["error"]["kind"].as_str().unwrap_or(""),
                        r["error"]["detail"]["code"].as_str(),
                    );
                    let listed = |f: &Value| {
                        f["kind"] == kind
                            && code.is_none_or(|c| {
                                f["codes"]
                                    .as_array()
                                    .is_some_and(|a| a.iter().any(|x| x == c))
                            })
                    };
                    let allowed = spec["failures"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(common.iter())
                        .any(listed);
                    if !allowed {
                        bad.push(format!(
                            "{}: {call} fails {kind}/{code:?}, which the call does not list",
                            t.id
                        ));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// A package file that a transcript gives to `admit_manifest` is complete: it starts from the `manifest` or `accessory` template and changes only the field under test (design 5.6). The ones that
// must be admitted validate against their schema (the legacy `port_forward` marker of an accessory is in it: HP A2-40, CA-20).
#[test]
fn admitted_package_files_are_complete_and_valid() {
    use serde_json::Value;
    let templates: Value = serde_json::from_str(
        botster_plugin_conformance::HP_TRANSCRIPTS
            .get_file("fixtures/frames.json")
            .unwrap()
            .contents_utf8()
            .unwrap(),
    )
    .unwrap();
    let schema_of = |name: &str| -> Value {
        let file = match name {
            "botster-plugin.json" => "plugin-manifest.schema.json",
            _ => "accessory.schema.json",
        };
        serde_json::from_str(
            &botster_plugin_contract::prelude::artifact_files()
                .into_iter()
                .find(|(n, _)| *n == file)
                .unwrap()
                .1,
        )
        .unwrap()
    };
    let mut bad = vec![];
    let mut seen = 0;
    for t in botster_conformance::load_dir(&botster_plugin_conformance::HP_TRANSCRIPTS).unwrap() {
        for s in &t.steps {
            let Some(c) = s.get("control").filter(|c| c["op"] == "admit_manifest") else {
                continue;
            };
            seen += 1;
            let step = botster_plugin_conformance::expand::expand(s, &templates).unwrap();
            let content = &step["control"]["content"];
            let file = c["file"].as_str().unwrap_or("");
            let admitted = s["expect"]["error"] == serde_json::json!({"$absent": true});
            if admitted {
                let v = jsonschema::validator_for(&schema_of(file)).unwrap();
                if let Err(e) = v.validate(content) {
                    bad.push(format!("{}: {e}", t.id));
                }
            }
            if file == "botster-plugin.json" {
                for key in ["botster_plugin_api", "name", "version", "entrypoints"] {
                    if content.get(key).is_none() {
                        bad.push(format!("{}: the manifest lacks {key} (R-6.1)", t.id));
                    }
                }
            }
        }
    }
    assert!(seen > 0 && bad.is_empty(), "{bad:#?}");
}

// Design 7.2 rule 7: the redaction matchers of the structural px_2 id judge real samples, so a leak, a lost value and a permitted redaction text are told apart before a real Hub runs it.
#[test]
fn the_px_2_redaction_matchers_judge_samples() {
    use base64::Engine;
    use botster_conformance::{matcher::matches, Bindings, NoSchemas};
    use serde_json::{json, Value};
    let t = botster_conformance::load_dir(&botster_plugin_conformance::HP_TRANSCRIPTS)
        .unwrap()
        .into_iter()
        .find(|t| t.id == "conf::px_2_tails_redact_own_secrets_and_are_never_logged")
        .unwrap();
    let templates: Value = serde_json::from_str(
        botster_plugin_conformance::HP_TRANSCRIPTS
            .get_file("fixtures/frames.json")
            .unwrap()
            .contents_utf8()
            .unwrap(),
    )
    .unwrap();
    let steps: Vec<Value> = t
        .steps
        .iter()
        .map(|s| botster_plugin_conformance::expand::expand(s, &templates).unwrap())
        .collect();
    let tail = steps
        .iter()
        .find_map(|s| s.pointer("/expect/payload/payload/stdout_tail_b64"))
        .unwrap()
        .clone();
    let log = steps
        .iter()
        .find(|s| s["control"]["op"] == "hub_log")
        .unwrap()["expect"]["text"]
        .clone();
    let enc = |s: &str| json!(base64::engine::general_purpose::STANDARD.encode(s));
    let judge = |m: &Value, v: Option<Value>| {
        matches(m, v.as_ref(), &mut Bindings::new(), &NoSchemas).is_ok()
    };
    // a redaction text that holds no secret passes, whatever it looks like (here it carries the base64 fragment of the secret)
    assert!(judge(&tail, Some(enc("OUTPUT-7731 [redacted]"))));
    assert!(judge(&tail, Some(enc("OUTPUT-7731 [redacted:bnRl]\n"))));
    for leak in [
        "OUTPUT-7731 hunter2",
        "OUTPUT-7731 hunter2\n",
        "OUTPUT-7731 xhunter2y",
        "OUTPUT-7731 a hunter2",
    ] {
        assert!(!judge(&tail, Some(enc(leak))), "{leak}");
    }
    assert!(!judge(&tail, None), "a missing tail");
    assert!(
        !judge(&tail, Some(enc("something else"))),
        "the public output is lost"
    );
    assert!(judge(&log, Some(json!("hub started"))));
    assert!(!judge(&log, Some(json!("captured hunter2"))));
    assert!(!judge(&log, Some(json!("captured OUTPUT-7731"))));
}

// HP EV-3 (steward ruling R-21): the status table of the contract crate is the table of the clause. The live cases are the transcript `ev_3_status_maps_to_kind`;
// `rejected_foreign` and `rejected_audience` are reserved vocabulary in v1 and have no live stimulus.
#[test]
fn ev_3_status_table_matches_the_clause() {
    let text = include_str!("../../../frozen/current/hub-plugin-contract-v1.1.joint-J4e.md");
    let row = text
        .lines()
        .find(|l| l.starts_with("| EV-3 "))
        .expect("the EV-3 row exists");
    let start = row
        .find("`accepted`")
        .expect("the status list starts at accepted");
    let end = row[start..]
        .find("They map to kinds")
        .expect("the mapping sentence")
        + start;
    let named: Vec<&str> = row[start..end]
        .split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect();
    let table: Vec<&str> = botster_plugin_contract::prelude::INGRESS_STATUSES
        .iter()
        .map(|(s, _)| *s)
        .collect();
    assert_eq!(
        table, named,
        "the crate lists the statuses of the clause, in its order"
    );
    // "the first three to capability_denied; shed_ingress_full to backpressured; rejected_invalid to invalid_request" (the first three rejections, accepted is no refusal)
    let kind = |s: &str| {
        botster_plugin_contract::prelude::INGRESS_STATUSES
            .iter()
            .find(|(n, _)| *n == s)
            .map(|(_, k)| *k)
    };
    for s in [
        "rejected_undeclared",
        "rejected_foreign",
        "rejected_audience",
    ] {
        assert_eq!(kind(s), Some("capability_denied"), "{s}");
    }
    assert_eq!(kind("rejected_invalid"), Some("invalid_request"));
    assert_eq!(kind("shed_ingress_full"), Some("backpressured"));
}

// The resolver answers for a frame by its type and lane (HP section 2, R-4, Appendix B). Each of these would pass if its answer were fixed.
#[test]
fn the_hp_schema_resolver_accepts_a_valid_frame_and_names_what_is_wrong_with_an_invalid_one() {
    use botster_conformance::SchemaResolver;
    let schemas = botster_plugin_conformance::HpSchemas::new();
    let valid =
        serde_json::json!({"lane": "control", "type": "Activated", "payload": {"epoch": 1}});
    assert_eq!(schemas.validate("Activated", &valid), Ok(()));
    assert_eq!(schemas.violations("Activated", &valid), Ok(vec![]));
    // The payload lacks its required field.
    let invalid = serde_json::json!({"lane": "control", "type": "Activated", "payload": {}});
    assert!(schemas.validate("Activated", &invalid).is_err());
    let paths = schemas.violations("Activated", &invalid).unwrap();
    assert!(
        paths.iter().any(|p| p.contains("required")),
        "the violation names the schema keyword: {paths:?}"
    );
    // The frame travels on a lane that is not one of its lanes.
    let wrong_lane =
        serde_json::json!({"lane": "bulk", "type": "Activated", "payload": {"epoch": 1}});
    assert!(schemas.validate("Activated", &wrong_lane).is_err());
    // A name that is not a frame type has no schema.
    assert_eq!(
        schemas.validate("NoSuchFrame", &valid),
        Err("a NoSuchFrame frame does not travel on lane 'control'".to_string())
    );
    assert!(schemas.violations("NoSuchFrame", &valid).is_err());
}

// A generated test must fail when its transcript is missing or does not pass, not return silently.
#[test]
#[should_panic(expected = "no transcript with id conf::no_such_transcript")]
fn a_generated_test_of_an_unknown_id_panics() {
    botster_plugin_conformance::run_test(
        "botster-plugin-conformance",
        &botster_plugin_conformance::HP_TRANSCRIPTS,
        "conf::no_such_transcript",
        &fake::hub,
        &botster_conformance::NoSchemas,
    );
}
