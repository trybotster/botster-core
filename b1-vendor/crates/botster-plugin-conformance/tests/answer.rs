//! `answer_host_call` answers a `HostCall` and leaves every other item alone, so one transcript accepts a host that decides an outcome locally and a host that forwards the call
//! (steward ruling R-27). The tests drive FakePlugin, a frame-level stand-in: `host_calls` forwards a call, `host_echo` makes none.

use botster_conformance::{parse_transcript, run_transcript, Limits, Outcome, SeedSet, StepDriver};
use botster_plugin_conformance::{expand::expand, fake, HpDriver, HpSchemas, HP_TRANSCRIPTS};
use serde_json::{json, Value};

fn run(fixture: &str, answer: Value, tail: Value) -> Outcome {
    let result_frame = json!({"lane": "invoke", "type": "Result", "payload": {"epoch": 7, "request_id": 5, "result": {"ok": true}}});
    let ignore = json!([{"lane": "call", "type": "HostCall", "payload": {"call": "store.get"}}, {"lane": "call", "type": "HostCall", "payload": {"call": "hub.identity"}}]);
    let mut steps = vec![
        json!({"send_frame": {"lane": "control", "type": "Hello", "payload": {"$ref": "hello", "epoch": 7}}}),
        json!({"expect": {"lane": "control", "type": "Ready", "payload": {"epoch": 7}}, "because": "HP HS-1"}),
        json!({"send_frame": {"lane": "control", "type": "Activated", "payload": {"epoch": 7}}}),
        json!({"send_frame": {"lane": "invoke", "type": "Invoke", "payload": {"$ref": "invoke", "epoch": 7, "request_id": 5, "handler": "h1"}}}),
        json!({"expect_eventually": result_frame, "ignore": ignore, "ack": {"answer_host_call": answer}, "because": "HP CA-1"}),
    ];
    steps.extend(tail.as_array().unwrap().iter().cloned());
    let doc = json!({"id": "conf::self_test", "contract": "hp", "clause": "CA-1", "subject": "plugin_host",
        "requires": {"controls": ["settle", "counters"]}, "setup": {"fixture": fixture}, "steps": steps});
    let mut t = parse_transcript("conf::self_test.json", &doc.to_string()).unwrap();
    let templates: Value = serde_json::from_str(
        HP_TRANSCRIPTS
            .get_file("fixtures/frames.json")
            .unwrap()
            .contents_utf8()
            .unwrap(),
    )
    .unwrap();
    t.steps = t
        .steps
        .iter()
        .map(|s| expand(s, &templates).unwrap())
        .collect();
    let make = |seed: u64| -> Box<dyn StepDriver> {
        Box::new(HpDriver::new(fake::host(seed), &HP_TRANSCRIPTS))
    };
    run_transcript(
        &t,
        &make,
        &SeedSet { seeds: vec![0] },
        &HpSchemas::new(),
        &Limits::default(),
    )
}

fn answer() -> Value {
    json!({"item": {"$item": ""}, "result": {"ok": true, "value": {"record": null}}})
}

/// What the host received on the call lane after the step: one HostResult per answered call, and nothing else.
fn results(n: usize) -> Value {
    let one = json!({"type": "HostResult"});
    json!([{"control": {"op": "received", "lane": "call"}, "expect": vec![one; n], "because": "HP CA-1"}])
}

// host_calls: h1 forwards two calls (store.get, hub.identity) and then returns. Both are answered by the ack, so the handler finishes, and the host received exactly two answers.
#[test]
fn a_forwarded_call_is_answered_and_the_handler_finishes() {
    assert_eq!(run("host_calls", answer(), results(2)), Outcome::Passed);
}

// A call that nothing answers leaves the handler waiting, so the step does not see the Result.
#[test]
fn an_unanswered_call_leaves_the_handler_waiting() {
    let mut a = answer();
    a["item"] = json!({"type": "Other"});
    assert!(matches!(
        run("host_calls", a, results(0)),
        Outcome::Failed { .. }
    ));
}

// host_echo makes no call: the ack gets only the Result item and writes nothing, so the host received no answer at all.
#[test]
fn a_host_that_forwards_nothing_passes_the_same_step() {
    assert_eq!(run("host_echo", answer(), results(0)), Outcome::Passed);
}

// The step itself, against a subject that records what is written: a HostCall is answered with the same epoch and call id, and any other item writes nothing.
mod step {
    use botster_conformance::{Bindings, Deadline, StepDriver};
    use botster_plugin_conformance::{
        Backpressured, HpDriver, HpSubject, Unsupported, HP_TRANSCRIPTS,
    };
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Writes(Rc<RefCell<Vec<Vec<u8>>>>);

    impl HpSubject for Writes {
        fn has_feature(&self, _: &str) -> bool {
            false
        }
        fn has_control(&self, _: &str) -> bool {
            false
        }
        fn configure(&mut self, _: &Value) -> Result<(), String> {
            Ok(())
        }
        fn write(&mut self, _: &str, bytes: &[u8]) -> Result<(), Backpressured> {
            self.0.borrow_mut().push(bytes.to_vec());
            Ok(())
        }
        fn read(&mut self, _: &str) -> Option<Vec<u8>> {
            None
        }
        fn conns(&self) -> Vec<String> {
            vec![]
        }
        fn closed(&self, _: &str) -> Option<String> {
            None
        }
        fn close_channel(&mut self, _: &str) {}
        fn advance_clock(&mut self, _: u64) {}
        fn control(&mut self, _: &Value) -> Result<Value, Unsupported> {
            Err(Unsupported(String::new()))
        }
    }

    fn exec(item: Value) -> Vec<Vec<u8>> {
        let writes = Rc::new(RefCell::new(vec![]));
        let mut d = HpDriver::new(Box::new(Writes(writes.clone())), &HP_TRANSCRIPTS);
        let step = json!({"answer_host_call": {"item": item, "result": {"ok": false, "error": {"kind": "invalid_request", "message": "m", "retryable": false}}}});
        d.exec(&step, &mut Bindings::new(), &Deadline::none())
            .unwrap();
        let out = writes.borrow().clone();
        out
    }

    #[test]
    fn a_host_call_is_answered_with_its_epoch_and_call_id() {
        let out = exec(
            json!({"lane": "call", "type": "HostCall", "payload": {"epoch": 7, "call_id": 41, "call": "x.y", "args": {}}}),
        );
        assert_eq!(out.len(), 1);
        let text = String::from_utf8_lossy(&out[0]).to_string();
        for part in [
            r#""epoch":7"#,
            r#""call_id":41"#,
            r#""kind":"invalid_request""#,
            r#""effect":"none""#,
        ] {
            assert!(text.contains(part), "{part} in {text}");
        }
        // The frame is a HostResult on the call lane (lane byte 2, type byte of HostResult).
        assert_eq!(out[0][4], 2);
    }

    #[test]
    fn any_other_item_writes_nothing() {
        let result = json!({"lane": "invoke", "type": "Result", "payload": {"epoch": 7, "request_id": 5, "result": {"ok": true, "value": {}}}});
        assert!(exec(result).is_empty());
        assert!(exec(json!({"settled": true})).is_empty());
    }
}
