//! The validation boundary (design 5.4), the empty-seed guard and the step deadline (design 6.1).

#![allow(clippy::result_large_err)]

use botster_conformance::stimulus::prepare;
use botster_conformance::{
    load::parse_transcript, run_step, run_transcript, Bindings, Deadline, Direction, Limits,
    NoSchemas, Outcome, Poll, SchemaRegistry, SeedSet, StepDriver, StepError,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Default)]
struct Rec {
    executed: Vec<Value>,
    items: VecDeque<Poll>,
    sleep_in_exec: Option<Duration>,
    sleep_in_next: Option<Duration>,
}

impl StepDriver for Rec {
    fn has_feature(&self, _: &str) -> bool {
        true
    }
    fn has_control(&self, _: &str) -> bool {
        true
    }
    fn setup(&mut self, _: &Value) -> Result<(), StepError> {
        Ok(())
    }
    fn exec(&mut self, step: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
        if let Some(d) = self.sleep_in_exec {
            // timer: deadline — a driver that takes longer than the step deadline (design 6.1)
            std::thread::sleep(d);
        }
        self.executed.push(step.clone());
        Ok(())
    }
    fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
        if let Some(d) = self.sleep_in_next {
            // timer: deadline — a driver that takes longer than the step deadline (design 6.1)
            std::thread::sleep(d);
        }
        self.items.pop_front().unwrap_or(Poll::Idle)
    }
}

fn registry() -> SchemaRegistry {
    let mut r = SchemaRegistry::new();
    r.add(
        "frame",
        &json!({"type":"object","required":["type"],"properties":{"type":{"type":"string"}}}),
    )
    .unwrap();
    r.set_frame_schema(Direction::Sent, "frame");
    r.set_frame_schema(Direction::Received, "frame");
    r
}

fn run(step: Value, rec: &mut Rec, binds: &mut Bindings) -> Result<(), StepError> {
    run_step(&step, rec, binds, &registry(), &Limits::default())
}

#[test]
fn a_valid_frame_is_substituted_before_it_is_sent() {
    let mut rec = Rec::default();
    let mut binds = Bindings::from([("id".to_string(), json!("7"))]);
    run(
        json!({"send": {"type": "req", "id": "$id"}}),
        &mut rec,
        &mut binds,
    )
    .unwrap();
    assert_eq!(
        rec.executed,
        vec![json!({"send": {"type": "req", "id": "7"}})]
    );
}

#[test]
fn an_unbound_variable_never_reaches_the_subject() {
    let mut rec = Rec::default();
    let err = run(
        json!({"send": {"type": "req", "id": "$nope"}}),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap_err();
    assert!(matches!(err, StepError::Failed { .. }));
    assert!(rec.executed.is_empty());
}

#[test]
fn a_valid_frame_that_fails_its_schema_never_reaches_the_subject() {
    for kind in ["send", "send_frame", "route_send"] {
        let mut rec = Rec::default();
        let err = run(json!({ kind: {"id": "1"} }), &mut rec, &mut Bindings::new()).unwrap_err();
        assert!(matches!(err, StepError::Failed { .. }), "{kind}");
        assert!(rec.executed.is_empty(), "{kind}");
    }
}

#[test]
fn a_send_with_no_frame_schema_is_a_transcript_error() {
    let mut rec = Rec::default();
    let step = json!({"send": {"type": "req"}});
    let err = run_step(
        &step,
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &Limits::default(),
    )
    .unwrap_err();
    assert!(matches!(err, StepError::Failed { .. }));
    assert!(rec.executed.is_empty());
}

#[test]
fn a_negative_frame_must_fail_its_schema_at_the_named_rule() {
    let step = |violates: &str, value: Value| json!({"send_invalid": value, "violates": violates});
    // Passes the schema: a transcript error.
    let mut rec = Rec::default();
    assert!(run(
        step("frame", json!({"type": "req"})),
        &mut rec,
        &mut Bindings::new()
    )
    .is_err());
    assert!(rec.executed.is_empty());
    // Fails, but at /required and not at the type rule: a transcript error.
    let mut rec = Rec::default();
    assert!(run(
        step("frame@/properties/type/type", json!({})),
        &mut rec,
        &mut Bindings::new()
    )
    .is_err());
    assert!(rec.executed.is_empty());
    // Fails at the named rule: it is sent.
    let mut rec = Rec::default();
    run(
        step("frame@/required", json!({})),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap();
    assert_eq!(rec.executed.len(), 1);
    // Fails at the type rule.
    let mut rec = Rec::default();
    run(
        step("frame@/properties/type/type", json!({"type": 5})),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap();
    // No rule named: any failure is enough.
    let mut rec = Rec::default();
    run(step("frame", json!([])), &mut rec, &mut Bindings::new()).unwrap();
}

#[test]
fn raw_stimuli_and_wire_bytes_reach_the_driver_unchanged_one_write_each() {
    let mut rec = Rec::default();
    let mut binds = Bindings::from([("x".to_string(), json!("1"))]);
    let steps = [
        json!({"send_raw": {"bytes_hex": "ff00 $x"}}),
        json!({"route_send_raw": {"bytes_hex": "zz"}}),
        json!({"send_frame_raw": {"lane": "a", "type": "t", "payload_hex": "c3"}}),
        json!({"send_wire": {"bytes_hex": "00000005"}}),
        json!({"send_wire": {"bytes_hex": "6162"}}),
        json!({"route_send_wire": {"bytes_hex": "00"}}),
    ];
    for step in &steps {
        run(step.clone(), &mut rec, &mut binds).unwrap();
    }
    assert_eq!(rec.executed, steps.to_vec());
}

#[test]
fn a_received_frame_is_validated_before_it_is_matched() {
    let mut rec = Rec::default();
    rec.items.push_back(Poll::Item(json!({"nope": 1})));
    let err = run(
        json!({"expect": {"$any": true}}),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap_err();
    match err {
        StepError::Failed { expected, .. } => assert!(expected
            .as_str()
            .unwrap()
            .contains("validates against 'frame'")),
        other => panic!("{other:?}"),
    }
    // A valid frame is matched as usual.
    let mut rec = Rec::default();
    rec.items.push_back(Poll::Item(json!({"type": "res"})));
    run(
        json!({"expect": {"type": "res"}}),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap();
}

#[test]
fn a_step_may_allow_an_invalid_received_frame() {
    let mut rec = Rec::default();
    rec.items.push_back(Poll::Item(json!({"nope": 1})));
    run(
        json!({"expect": {"nope": 1}, "allow_invalid": true}),
        &mut rec,
        &mut Bindings::new(),
    )
    .unwrap();
}

#[test]
fn every_waiting_construct_validates_what_it_receives() {
    let steps = [
        json!({"expect_set": [{"$any": true}]}),
        json!({"expect_eventually": {"$any": true}}),
        json!({"expect_quiet_until": {"never": 1}, "barrier": {"send_raw": {"bytes_hex": "00"}}, "until": {"$any": true}}),
    ];
    for step in steps {
        let mut rec = Rec::default();
        rec.items.push_back(Poll::Item(json!({"nope": 1})));
        let err = run(step.clone(), &mut rec, &mut Bindings::new()).unwrap_err();
        assert!(matches!(err, StepError::Failed { .. }), "{step}");
    }
}

fn transcript_with(seeds: Option<Vec<u64>>) -> botster_conformance::Transcript {
    let mut t = parse_transcript(
        "conf::t.json",
        &json!({"id":"conf::t","contract":"hc","clause":"EN-8","subject":"hub","steps":[{"expect":{"a":1}}]}).to_string(),
    )
    .unwrap();
    t.seeds = seeds;
    t
}

#[test]
fn an_empty_seed_list_is_refused_at_load_and_never_passes_at_run() {
    let doc = json!({"id":"conf::t","contract":"hc","clause":"EN-8","subject":"hub","seeds":[]})
        .to_string();
    assert!(parse_transcript("conf::t.json", &doc).is_err());
    let make = |_: u64| -> Box<dyn StepDriver> { Box::new(Rec::default()) };
    for (t, run_seeds) in [
        (transcript_with(Some(vec![])), SeedSet::local()),
        (transcript_with(None), SeedSet { seeds: vec![] }),
    ] {
        let outcome = run_transcript(&t, &make, &run_seeds, &NoSchemas, &Limits::default());
        assert!(matches!(outcome, Outcome::Failed { .. }), "{outcome:?}");
    }
}

fn timed() -> Limits {
    Limits {
        step_timeout: Some(Duration::from_millis(5)),
        ..Limits::default()
    }
}

fn inconclusive(r: Result<(), StepError>) -> bool {
    matches!(r, Err(StepError::Inconclusive { ref limit }) if limit == "step_timeout")
}

#[test]
fn a_driver_that_reports_a_timeout_gives_inconclusive() {
    let mut rec = Rec::default();
    rec.items.push_back(Poll::TimedOut);
    let r = run_step(
        &json!({"expect": {"a": 1}}),
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &timed(),
    );
    assert!(inconclusive(r));
}

#[test]
fn a_late_answer_gives_inconclusive_never_failed() {
    let mut rec = Rec {
        sleep_in_next: Some(Duration::from_millis(30)),
        ..Rec::default()
    };
    rec.items.push_back(Poll::Item(json!({"a": 2})));
    let r = run_step(
        &json!({"expect": {"a": 1}}),
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &timed(),
    );
    assert!(
        inconclusive(r),
        "a wrong value that arrives after the deadline proves nothing"
    );
}

#[test]
fn a_slow_exec_gives_inconclusive() {
    let mut rec = Rec {
        sleep_in_exec: Some(Duration::from_millis(30)),
        ..Rec::default()
    };
    let r = run_step(
        &json!({"anything": 1}),
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &timed(),
    );
    assert!(inconclusive(r));
}

#[test]
fn no_timeout_means_no_deadline() {
    let mut rec = Rec {
        sleep_in_exec: Some(Duration::from_millis(10)),
        ..Rec::default()
    };
    run_step(
        &json!({"anything": 1}),
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &Limits::default(),
    )
    .unwrap();
}

#[test]
fn a_negative_frame_with_an_unknown_schema_never_reaches_the_subject() {
    let step = json!({"send_invalid": {}, "violates": "missing@/required"});
    // The default resolver cannot list violations.
    let mut rec = Rec::default();
    assert!(run_step(
        &step,
        &mut rec,
        &mut Bindings::new(),
        &NoSchemas,
        &Limits::default()
    )
    .is_err());
    assert!(rec.executed.is_empty());
    // A registry without that schema.
    let mut rec = Rec::default();
    assert!(run(step, &mut rec, &mut Bindings::new()).is_err());
    assert!(rec.executed.is_empty());
}

#[test]
fn the_named_rule_is_an_exact_schema_path() {
    let mut r = SchemaRegistry::new();
    r.add(
        "frame",
        &json!({"type":"object","required":["outer"],"properties":{"outer":{"type":"object","required":["inner"]}}}),
    )
    .unwrap();
    let step =
        |rule: &str| json!({"send_invalid": {"outer": {}}, "violates": format!("frame@{rule}")});
    let go = |rule: &str| {
        let mut rec = Rec::default();
        (
            run_step(
                &step(rule),
                &mut rec,
                &mut Bindings::new(),
                &r,
                &Limits::default(),
            ),
            rec.executed.len(),
        )
    };
    assert!(
        go("/required").0.is_err(),
        "a nested /properties/outer/required is not /required"
    );
    assert_eq!(go("/required").1, 0);
    assert!(go("/properties/outer/required").0.is_ok());
}

#[test]
fn a_datachannel_send_frame_is_checked_before_it_is_sent_and_other_controls_are_not() {
    // Core DP-3: the frame that `datachannel_send` carries must validate, as a `route_send` frame does.
    let send = |frame: Value| json!({"control": {"op": "datachannel_send", "route": 1, "frame": frame, "chunk_bytes": 4}});
    let mut rec = Rec::default();
    assert!(run(send(json!({"type": "req"})), &mut rec, &mut Bindings::new()).is_ok());
    assert_eq!(rec.executed.len(), 1);
    let mut rec = Rec::default();
    assert!(run(send(json!({})), &mut rec, &mut Bindings::new()).is_err());
    assert!(rec.executed.is_empty());
    // Another control with a `frame` argument reaches the driver unchecked.
    let other = json!({"control": {"op": "datachannel_close", "route": 1, "frame": {}}});
    let mut rec = Rec::default();
    assert!(run(other, &mut rec, &mut Bindings::new()).is_ok());
    assert_eq!(rec.executed.len(), 1);
    // Structural run: no bindings, so the route is unbound. A bad frame is still a transcript error that is not "unbound".
    let unbound_send = |frame: Value| json!({"control": {"op": "datachannel_send", "route": "$r1", "frame": frame, "chunk_bytes": 4}});
    let bad = prepare(&unbound_send(json!({})), &Bindings::new(), &registry()).unwrap_err();
    assert!(!format!("{bad:?}").contains("unbound variable"));
    let good = prepare(
        &unbound_send(json!({"type": "req"})),
        &Bindings::new(),
        &registry(),
    )
    .unwrap_err();
    assert!(format!("{good:?}").contains("unbound variable"));
}
