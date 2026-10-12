//! The structural check of the HC transcripts and the Hub-subject UI transcripts (BUILD.md "Phases": scripted players only, no stand-in Hub).
//!
//! No Hub exists in Phase 0, so a `hub` transcript gets no full run here: it gets this check, and its full run is the real Hub in Stage 2
//! (`hc_hub_conformance_tests!`). The check: the file parses and its id is the file name; every valid frame validates against the generated
//! schema; every typed negative fails at its named rule; every expectation cites its clause; the transcript's kind (HC A5-3) matches its
//! steps; every control it uses is documented as a harness obligation.

// Failure values carry whole JSON documents, as in `botster-conformance` (cold path).
#![allow(clippy::result_large_err)]

use botster_conformance::stimulus::prepare;
use botster_conformance::{load_dir, Bindings, SchemaResolver, StepError, Subject, Transcript};
use botster_hub_conformance::hc::{HcSchemas, HC_TRANSCRIPTS};
use botster_hub_conformance::UI_TRANSCRIPTS;
use serde_json::{json, Value};

fn hc() -> Vec<Transcript> {
    load_dir(&HC_TRANSCRIPTS).expect("the HC transcripts load")
}

fn ui_hub() -> Vec<Transcript> {
    load_dir(&UI_TRANSCRIPTS)
        .expect("the UI transcripts load")
        .into_iter()
        .filter(|t| t.subject == Subject::Hub)
        .collect()
}

/// Checks each send-class step before any subject sees it (design 5.4). A step that uses a variable that an earlier expectation binds
/// cannot be checked before a run, so it is counted and left to the run against the real Hub or the player.
fn structural_failures(t: &Transcript, schemas: &HcSchemas) -> (Vec<String>, usize, usize) {
    let (mut failures, mut checked, mut deferred) = (vec![], 0, 0);
    for (i, step) in t.steps.iter().enumerate() {
        match prepare(step, &Bindings::new(), schemas) {
            Ok(_) => checked += 1,
            Err(StepError::Failed { received, .. })
                if received
                    .as_str()
                    .is_some_and(|r| r.starts_with("unbound variable")) =>
            {
                deferred += 1
            }
            Err(e) => failures.push(format!("{} step {i}: {e:?}", t.id)),
        }
    }
    (failures, checked, deferred)
}

#[test]
fn every_hub_transcript_is_structurally_valid() {
    let schemas = HcSchemas::default();
    let all: Vec<Transcript> = hc().into_iter().chain(ui_hub()).collect();
    assert_eq!(all.len(), 60 + 6, "the landed HC ids and the Hub UI ids");
    let mut failures = vec![];
    let (mut checked, mut deferred) = (0, 0);
    for t in &all {
        let (f, c, d) = structural_failures(t, &schemas);
        failures.extend(f);
        checked += c;
        deferred += d;
    }
    assert!(failures.is_empty(), "{failures:#?}");
    // A check that skips everything proves nothing: most steps are checked, and the typed negatives are among them.
    assert!(
        checked > 500 && deferred < 30,
        "checked {checked}, deferred {deferred}"
    );
    let negatives: usize = all
        .iter()
        .flat_map(|t| &t.steps)
        .filter(|s| s.get("send_invalid").is_some())
        .count();
    assert_eq!(negatives, 3, "the typed negatives of the HC transcripts");
}

#[test]
fn the_structural_check_fails_a_bad_frame_and_a_negative_at_the_wrong_rule() {
    let schemas = HcSchemas::default();
    let t = hc()
        .into_iter()
        .find(|t| {
            t.steps
                .iter()
                .any(|s| s.get("send").is_some_and(|f| f["type"] == "hello"))
        })
        .expect("a transcript that says hello");
    let mut bad = t.clone();
    let i = bad
        .steps
        .iter()
        .position(|s| s.get("send").is_some_and(|f| f["type"] == "hello"))
        .expect("a hello");
    bad.steps[i]["send"]["versions"] = json!({"min": "one"});
    assert!(
        !structural_failures(&bad, &schemas).0.is_empty(),
        "a hello with a string version must fail ClientFrame"
    );

    let neg = hc()
        .into_iter()
        .flat_map(|t| t.steps)
        .find(|s| s.get("send_invalid").is_some())
        .expect("a typed negative");
    let mut wrong = neg.clone();
    wrong["violates"] = json!("hc/ClientFrame@/no/such/rule");
    let result = prepare(&wrong, &Bindings::new(), &schemas);
    assert!(
        matches!(result, Err(StepError::Failed { .. })),
        "a negative must fail at its named rule: {result:?}"
    );
}

#[test]
fn every_expectation_cites_its_clause() {
    let mut missing = vec![];
    for t in hc().iter().chain(&ui_hub()) {
        for step in botster_conformance::citation::missing_because(t) {
            missing.push(format!("{} step {step}", t.id));
        }
        // A client observation is an expectation too (HC A5-3: it cites the clause that obliges the client).
        for (i, s) in t.steps.iter().enumerate() {
            if let Some(r) = s.get("client_requests").and(Some(s)) {
                if !r["because"]
                    .as_str()
                    .is_some_and(botster_conformance::citation::is_citation)
                {
                    missing.push(format!("{} step {i} (client_requests)", t.id));
                }
            }
            if let Some(obs) = s.get("client_expect") {
                if !obs["because"]
                    .as_str()
                    .is_some_and(botster_conformance::citation::is_citation)
                {
                    missing.push(format!("{} step {i} (client_expect)", t.id));
                }
            }
        }
    }
    assert!(missing.is_empty(), "no `because`: {missing:#?}");
}

/// HC A5-3: a transcript declares its kind through its subject. A `hub` transcript is shared: it states Hub semantics and never observes a
/// client. A `client` transcript is client-only: the player sends server input and the transcript asserts only the client's response.
#[test]
fn a_transcript_declares_shared_or_client_only_and_its_steps_agree() {
    const CLIENT_ONLY_STEPS: [&str; 13] = [
        "client_act",
        "client_requests",
        "client_idle",
        "send",
        "send_raw",
        "send_wire",
        "disconnect",
        "client_expect",
        "expect_eventually",
        "expect_set",
        "expect_quiet_until",
        "expect_closed",
        "expect",
    ];
    let mut client_only = 0;
    for t in hc().iter().chain(&ui_hub()) {
        let has_client_step = t
            .steps
            .iter()
            .any(|s| s.get("client_expect").is_some() || s.get("client_requests").is_some());
        match t.subject {
            Subject::Hub => assert!(
                !has_client_step,
                "{}: a shared transcript observes no client",
                t.id
            ),
            Subject::Client => {
                client_only += 1;
                assert!(
                    t.fake == Some(botster_conformance::FakeMode::Structural)
                        && t.ruling.as_deref() == Some("R-1.4"),
                    "{}: client-only needs its ruling",
                    t.id
                );
                assert!(
                    has_client_step,
                    "{}: a client-only transcript asserts the client's response",
                    t.id
                );
                for s in &t.steps {
                    let key = s
                        .as_object()
                        .and_then(|o| o.keys().find(|k| CLIENT_ONLY_STEPS.contains(&k.as_str())))
                        .is_some();
                    assert!(
                        key,
                        "{}: a client-only transcript scripts server input only: {s}",
                        t.id
                    );
                    // No step of a client-only transcript is an assertion about the Hub: the player sends server frames, never client frames.
                    if let Some(f) = s.get("send") {
                        assert!(
                            matches!(
                                f["type"].as_str(),
                                Some("welcome" | "res" | "watch" | "event" | "close")
                            ),
                            "{}: not a server frame: {f}",
                            t.id
                        );
                    }
                }
            }
            other => panic!("{}: subject {other:?} is not an HC subject", t.id),
        }
    }
    assert_eq!(client_only, 3, "ds_4, bd_3 and en_6_unknown_optional are the client-only ids (HC A5-3 names unusual limits and an unknown optional field; DS-4 is a client rule)");
}

fn controls_of(t: &Transcript) -> Vec<String> {
    let mut ops: Vec<String> = t.requires.controls.clone();
    for s in &t.steps {
        if let Some(op) = s.pointer("/control/op").and_then(Value::as_str) {
            ops.push(op.to_string());
        }
    }
    ops
}

/// Design 6.3: each control is a harness obligation that cites the clauses that need it, in `docs/hub-subject-controls.md`.
#[test]
fn every_control_is_documented_as_a_harness_obligation() {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/hub-subject-controls.md"
    ))
    .expect("docs/hub-subject-controls.md");
    let mut used = std::collections::BTreeMap::<String, Vec<String>>::new();
    for t in hc().iter().chain(&ui_hub()) {
        for op in controls_of(t) {
            used.entry(op).or_default().push(t.clause.clone());
        }
    }
    assert!(used.len() >= 14, "{used:?}");
    for (op, clauses) in &used {
        let row = doc.lines().find(|l| l.starts_with(&format!("| `{op}` |")));
        let row =
            row.unwrap_or_else(|| panic!("control `{op}` is not in docs/hub-subject-controls.md"));
        for c in clauses {
            assert!(
                row.contains(c.as_str()),
                "the row of `{op}` does not cite {c}: {row}"
            );
        }
    }
    // The other way: a documented control that no transcript uses is a stale row.
    let controls = doc
        .split("\n## Controls")
        .nth(1)
        .and_then(|c| c.split("\n## ").next())
        .expect("a Controls section");
    for line in controls.lines().filter(|l| l.starts_with("| `")) {
        let op = line
            .trim_start_matches("| `")
            .split('`')
            .next()
            .unwrap_or("");
        assert!(
            used.contains_key(op),
            "docs/hub-subject-controls.md documents `{op}`, which no transcript uses"
        );
    }
}

#[test]
fn the_in_memory_transport_is_never_a_binding() {
    // HC A5-1: the transcripts connect over the default transport unless they name a binding to test; none names the in-memory one.
    for t in hc() {
        for s in &t.steps {
            for key in ["connect", "route_connect"] {
                if let Some(b) = s
                    .pointer(&format!("/{key}/binding"))
                    .and_then(Value::as_str)
                {
                    assert_ne!(
                        b, "loopback",
                        "{}: the in-memory transport is not a binding",
                        t.id
                    );
                    assert_ne!(
                        b, "in-memory",
                        "{}: the in-memory transport is not a binding",
                        t.id
                    );
                }
            }
        }
    }
}

/// A typed result payload that a script sends is checked against its own schema, not only the unconstrained `ok` of `ServerFrame`.
#[test]
fn the_describe_results_that_a_client_only_script_sends_are_complete() {
    let schemas = HcSchemas::default();
    let mut checked = 0;
    for t in hc().iter().filter(|t| t.subject == Subject::Client) {
        for s in &t.steps {
            if let Some(ok) = s.pointer("/send/ok").filter(|ok| ok.get("kinds").is_some()) {
                schemas
                    .validate("hc/DescribeResult", ok)
                    .unwrap_or_else(|e| panic!("{}: {e}", t.id));
                checked += 1;
            }
        }
    }
    assert!(checked >= 1);
}

/// A resource that a client-only script sends is a complete row of its kind (R-11), not only a frame that `ServerFrame` accepts.
#[test]
fn the_session_rows_that_a_client_only_script_sends_are_complete() {
    let schemas = HcSchemas::default();
    let mut checked = 0;
    for t in hc().iter().filter(|t| t.subject == Subject::Client) {
        for s in &t.steps {
            if let Some(ok) = s.pointer("/send/ok").filter(|ok| {
                ok.get("lifecycle_class").is_some() || ok.get("session_type").is_some()
            }) {
                schemas
                    .validate("kind/session", ok)
                    .unwrap_or_else(|e| panic!("{}: {e}", t.id));
                checked += 1;
            }
        }
    }
    assert!(checked >= 1);
}

/// The source rows of a fixture, and of a setup that gives its own, are complete rows of their kind (R-11): the Hub reads them as the
/// state that the harness's real Core path produces, so a row that the schema refuses would hide a difference.
#[test]
fn the_source_rows_of_the_fixtures_and_setups_validate_against_their_kind_schemas() {
    let schemas = HcSchemas::default();
    let mut rows = 0;
    let mut check = |origin: &str, hub: &Value| {
        for (kind, list) in hub["rows"].as_object().into_iter().flatten() {
            for row in list.as_array().into_iter().flatten() {
                schemas
                    .validate(&format!("kind/{kind}"), row)
                    .unwrap_or_else(|e| panic!("{origin}: a {kind} row: {e}"));
                rows += 1;
            }
        }
    };
    for name in ["base", "plugin"] {
        check(
            name,
            &botster_hub_conformance::hc::fixture(name).expect("fixture"),
        );
    }
    for t in hc().iter().chain(&ui_hub()) {
        check(&t.id, &t.setup["hub"]);
    }
    assert!(rows >= 10, "{rows}");
}
