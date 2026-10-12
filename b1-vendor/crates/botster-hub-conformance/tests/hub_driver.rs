//! The HC step driver against a loopback Hub that only records and answers (design 5.5, 6.4). It is the self-test of the DRIVER:
//! framing, partial reads, pumping, controls, connection names and the route handshake. It proves no Hub behavior and holds no clause
//! expectation; the Hub's behavior is the transcripts' run against the real Hub in Stage 2.

// Failure values carry whole JSON documents, as in `botster-conformance` (cold path).
#![allow(clippy::result_large_err)]

use botster_conformance::{
    parse_transcript, run_steps, Bindings, Deadline, Limits, Poll, StepDriver, StepError,
    StepFailure,
};
use botster_hub_conformance::hc::{
    driver_for, ConnectSpec, HcSchemas, HubDriver, HubRead, HubSubject, Pump,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Default)]
struct Log {
    writes: Vec<(u32, Vec<u8>)>,
    read_max: Vec<usize>,
    controls: Vec<(String, Value)>,
    closed: Vec<u32>,
    configured: Option<Value>,
    started: bool,
    clock: Vec<u64>,
    connects: Vec<(Option<String>, Option<u32>, Option<String>)>,
    pumps: usize,
}

#[derive(Default)]
struct Conn {
    inbox: Vec<u8>,
    pending: VecDeque<Vec<u8>>,
    out: Vec<u8>,
    eof: bool,
}

struct LoopHub {
    log: Rc<RefCell<Log>>,
    conns: Vec<Conn>,
    refuse: bool,
    no_clock: bool,
    spin: bool,
    max_frame: u64,
    pad: usize,
}

fn wrap(body: &[u8]) -> Vec<u8> {
    let mut v = (body.len() as u32).to_be_bytes().to_vec();
    v.extend(body);
    v
}

impl LoopHub {
    fn new() -> (Self, Rc<RefCell<Log>>) {
        let log = Rc::new(RefCell::new(Log::default()));
        (
            LoopHub {
                log: log.clone(),
                conns: vec![],
                refuse: false,
                no_clock: false,
                spin: false,
                max_frame: 4096,
                pad: 0,
            },
            log,
        )
    }
}

impl HubSubject for LoopHub {
    fn has_feature(&self, name: &str) -> bool {
        name == "ui_embed"
    }
    fn has_control(&self, op: &str) -> bool {
        matches!(op, "ping" | "emit" | "defer")
    }
    fn configure(&mut self, hub: &Value) -> Result<(), String> {
        self.log.borrow_mut().configured = Some(hub.clone());
        Ok(())
    }
    fn start(&mut self) -> Result<(), String> {
        self.log.borrow_mut().started = true;
        Ok(())
    }
    fn connect(&mut self, spec: &ConnectSpec) -> Result<Option<u32>, String> {
        self.log.borrow_mut().connects.push((
            spec.binding.clone(),
            spec.uid,
            spec.credential.clone(),
        ));
        if self.refuse {
            return Ok(None);
        }
        self.conns.push(Conn::default());
        Ok(Some(self.conns.len() as u32 - 1))
    }
    fn write(&mut self, conn: u32, bytes: &[u8]) {
        self.log.borrow_mut().writes.push((conn, bytes.to_vec()));
        let max_frame = self.max_frame;
        let pad = self.pad;
        let c = &mut self.conns[conn as usize];
        c.inbox.extend(bytes);
        while c.inbox.len() >= 4 {
            let len = u32::from_be_bytes([c.inbox[0], c.inbox[1], c.inbox[2], c.inbox[3]]) as usize;
            if c.inbox.len() < 4 + len {
                break;
            }
            let body: Vec<u8> = c.inbox[4..4 + len].to_vec();
            c.inbox.drain(..4 + len);
            let Ok(v) = serde_json::from_slice::<Value>(&body) else {
                continue;
            };
            let reply = match v["type"].as_str() {
                Some("hello") => Some(welcome(max_frame)),
                Some("req") => Some(
                    json!({"type": "res", "id": v["id"], "ok": {"id": v["target"], "pad": "x".repeat(pad)}}),
                ),
                _ => None,
            };
            if let Some(r) = reply {
                c.pending.push_back(wrap(&serde_json::to_vec(&r).unwrap()));
            }
        }
    }
    fn read(&mut self, conn: u32, max: usize, _deadline: &Deadline) -> HubRead {
        self.log.borrow_mut().read_max.push(max);
        let c = &mut self.conns[conn as usize];
        if !c.out.is_empty() {
            let n = max.min(c.out.len());
            return HubRead::Bytes(c.out.drain(..n).collect());
        }
        if c.eof {
            HubRead::Eof
        } else {
            HubRead::Empty
        }
    }
    fn close(&mut self, conn: u32) {
        self.log.borrow_mut().closed.push(conn);
        self.conns[conn as usize].eof = true;
    }
    fn advance_clock(&mut self, ms: u64) -> Result<(), String> {
        if self.no_clock {
            return Err("no injected clock".into());
        }
        self.log.borrow_mut().clock.push(ms);
        Ok(())
    }
    fn pump(&mut self) -> Pump {
        self.log.borrow_mut().pumps += 1;
        if self.spin {
            return Pump::Progress;
        }
        for c in &mut self.conns {
            if let Some(f) = c.pending.pop_front() {
                c.out.extend(f);
                return Pump::Progress;
            }
        }
        Pump::Quiet
    }
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        self.log
            .borrow_mut()
            .controls
            .push((op.to_string(), args.clone()));
        if op == "defer" {
            // The bytes wait in the Hub's scheduler: only a pump delivers them.
            let bytes =
                botster_route_codec::prelude::hex_decode(args["bytes_hex"].as_str().unwrap())
                    .unwrap();
            self.conns[args["conn"].as_u64().unwrap() as usize]
                .pending
                .push_back(bytes);
        }
        if op == "emit" {
            let bytes =
                botster_route_codec::prelude::hex_decode(args["bytes_hex"].as_str().unwrap())
                    .unwrap();
            self.conns[args["conn"].as_u64().unwrap() as usize]
                .out
                .extend(bytes);
        }
        Ok(json!({"pong": true, "conn": args["conn"]}))
    }
}

fn driver(hub: LoopHub) -> HubDriver<LoopHub> {
    HubDriver::new(hub, botster_hub_conformance::hc::fixture)
}

/// A welcome that validates against `ServerFrame` (HC HS-3).
fn welcome(max_frame: u64) -> Value {
    json!({"type": "welcome", "hub_id": "hub-1", "incarnation": "i1", "version": 2, "features": [], "terminal_format": "ghostsnp/2+raw",
        "limits": {"max_frame_bytes": max_frame, "max_chunk_bytes": 1024, "max_outstanding": 8, "max_watches": 64, "route_connect_seconds": 10,
        "idempotency_window_seconds": 60, "cursor_idle_seconds": 30, "operation_retention_seconds": 600, "max_paste_bytes": 1024,
        "max_file_bytes": 1024, "query_client_deadline_ms": 100}})
}

fn exec(d: &mut HubDriver<LoopHub>, step: Value) -> Result<(), StepError> {
    d.exec(&step, &mut Bindings::new(), &Deadline::none())
}

fn next(d: &mut HubDriver<LoopHub>, on: &str) -> Poll {
    d.next(&json!(on), &Deadline::none())
}

/// Runs a whole transcript on a driver: its `setup`, then its steps.
fn run(hub: LoopHub, setup: Value, steps: Value) -> Result<(), StepFailure> {
    let text = json!({"id": "conf::t", "contract": "hc", "clause": "EN-1", "subject": "hub", "setup": setup, "steps": steps}).to_string();
    let t = parse_transcript("conf::t.json", &text).unwrap();
    let mut d = driver_for(hub);
    d.setup(&t.setup)
        .map_err(|error| StepFailure { step: 0, error })?;
    run_steps(
        &t.steps,
        d.as_mut(),
        &mut Bindings::new(),
        &HcSchemas::default(),
        &Limits::default(),
    )
}

fn hello() -> Value {
    json!({"send": {"type": "hello", "protocol": "botster-hub", "versions": {"min": 1, "max": 2}, "client": {"name": "t", "version": "0"}, "features": [], "terminal_formats": ["ghostsnp/2+raw"]}, "on": "c1"})
}

fn failed(r: &Result<(), StepFailure>) -> bool {
    matches!(
        r,
        Err(StepFailure {
            error: StepError::Failed { .. },
            ..
        })
    )
}

#[test]
fn a_frame_is_delimited_on_write_and_reassembled_from_small_reads() {
    let (mut hub, log) = LoopHub::new();
    hub.pad = 700;
    let steps = json!([
        {"connect": {"as": "c1"}}, hello(),
        {"expect": {"type": "welcome"}, "on": "c1", "because": "HC HS-3"},
        {"send": {"type": "req", "id": "1", "verb": "get", "kind": "session", "target": "s1"}, "on": "c1"},
        {"expect": {"type": "res", "id": "1", "ok": {"id": "s1"}}, "on": "c1", "because": "HC VB-2"}
    ]);
    run(hub, json!({}), steps).expect("passes");
    let log = log.borrow();
    // HC TR-1: `[u32 length][JSON]`, one write per frame.
    let first = &log.writes[0].1;
    let body = serde_json::to_vec(&hello()["send"]).unwrap();
    assert_eq!(first[..4], (body.len() as u32).to_be_bytes());
    assert_eq!(first[4..], body[..]);
    // The response is over 700 bytes, so it came in several reads of at most 256.
    assert!(log.read_max.iter().all(|m| *m == 256), "{:?}", log.read_max);
    assert!(log.read_max.len() >= 4, "{:?}", log.read_max);
    assert!(
        log.started,
        "the Hub starts unless the setup says start: false"
    );
}

#[test]
fn setup_merges_the_fixture_and_the_hub_override_and_start_false_leaves_the_hub_down() {
    let (hub, log) = LoopHub::new();
    run(
        hub,
        json!({"fixture": "base", "hub": {"limits": {"y": 2}}, "start": false}),
        json!([]),
    )
    .unwrap();
    let configured = log.borrow().configured.clone().expect("configured");
    assert!(
        configured["rows"]["session"].is_array(),
        "the fixture's rows: {configured}"
    );
    assert_eq!(
        configured["limits"],
        json!({"y": 2}),
        "the transcript's hub member overrides the fixture's"
    );
    assert!(!log.borrow().started);
    let (hub, _) = LoopHub::new();
    let r = run(hub, json!({"fixture": "missing"}), json!([]));
    assert!(failed(&r), "{r:?}");
}

#[test]
fn a_connect_is_refused_only_when_the_transcript_says_so() {
    let (mut hub, log) = LoopHub::new();
    hub.refuse = true;
    let spec = json!({"connect": {"as": "c1", "binding": "local-stream", "uid": 7, "credential": "k", "refused": true}});
    run(hub, json!({}), json!([spec])).expect("a refused connect that was expected");
    assert_eq!(
        log.borrow().connects,
        vec![(Some("local-stream".into()), Some(7), Some("k".into()))]
    );
    let (mut hub, _) = LoopHub::new();
    hub.refuse = true;
    assert!(
        failed(&run(hub, json!({}), json!([{"connect": {"as": "c1"}}]))),
        "nothing listens but an accepted connection was expected"
    );
    let (hub, _) = LoopHub::new();
    assert!(
        failed(&run(
            hub,
            json!({}),
            json!([{"connect": {"as": "c1", "refused": true}}])
        )),
        "the Hub accepted a connection that had to be refused"
    );
}

#[test]
fn a_step_on_a_connection_that_is_not_open_fails() {
    let (hub, _) = LoopHub::new();
    let mut d = driver(hub);
    assert!(matches!(
        exec(&mut d, json!({"send": {"type": "x"}, "on": "c9"})),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut d, json!({"disconnect": {}, "on": "c9"})),
        Err(StepError::Failed { .. })
    ));
    assert_eq!(next(&mut d, "c9"), Poll::Idle);
}

#[test]
fn raw_and_wire_stimuli_reach_the_hub_byte_for_byte_and_disconnect_closes() {
    let (hub, log) = LoopHub::new();
    let mut d = driver(hub);
    exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
    exec(
        &mut d,
        json!({"send_raw": {"bytes_hex": "7b7b"}, "on": "c1"}),
    )
    .unwrap();
    exec(
        &mut d,
        json!({"send_wire": {"bytes_hex": "000000"}, "on": "c1"}),
    )
    .unwrap();
    exec(
        &mut d,
        json!({"send_invalid": {"type": "banana"}, "violates": "hc/ClientFrame", "on": "c1"}),
    )
    .unwrap();
    assert!(matches!(
        exec(&mut d, json!({"send_raw": {}, "on": "c1"})),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut d, json!({"send_raw": {"bytes_hex": "zz"}, "on": "c1"})),
        Err(StepError::Failed { .. })
    ));
    let w = log.borrow().writes.clone();
    assert_eq!(
        w[0].1,
        vec![0, 0, 0, 2, 0x7b, 0x7b],
        "send_raw adds the delimiter"
    );
    assert_eq!(w[1].1, vec![0, 0, 0], "send_wire does not");
    assert_eq!(
        w[2].1[4..],
        br#"{"type":"banana"}"#[..],
        "send_invalid goes out as one control frame"
    );
    exec(&mut d, json!({"disconnect": {}, "on": "c1"})).unwrap();
    assert_eq!(log.borrow().closed, vec![0]);
    // The first poll reads the end of the stream (progress), the next gives the close, once.
    assert_eq!(next(&mut d, "c1"), Poll::Pending);
    assert_eq!(next(&mut d, "c1"), Poll::Closed(json!({"eof": true})));
    assert_eq!(next(&mut d, "c1"), Poll::Idle, "the close is given once");
}

#[test]
fn the_driver_pumps_an_empty_read_and_reports_idle_when_the_hub_is_quiet() {
    let (hub, log) = LoopHub::new();
    let mut d = driver(hub);
    exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
    assert_eq!(next(&mut d, "c1"), Poll::Idle);
    assert_eq!(log.borrow().pumps, 1);
    exec(&mut d, hello()).unwrap();
    // The welcome waits in the Hub's scheduler: the first poll pumps it (progress), the next poll reads it.
    assert_eq!(next(&mut d, "c1"), Poll::Pending);
    assert!(matches!(next(&mut d, "c1"), Poll::Pending | Poll::Item(_)));
    let mut got = None;
    for _ in 0..8 {
        if let Poll::Item(v) = next(&mut d, "c1") {
            got = Some(v);
            break;
        }
    }
    assert_eq!(got.expect("the welcome")["type"], "welcome");
    // An expired deadline gives TimedOut, not a wait.
    assert_eq!(
        d.next(
            &json!("c1"),
            &Deadline::after(Some(std::time::Duration::ZERO))
        ),
        Poll::TimedOut
    );
}

#[test]
fn a_frame_over_the_welcome_bound_and_a_non_json_body_are_invalid_frames() {
    let (mut hub, _) = LoopHub::new();
    hub.max_frame = 64;
    hub.pad = 100;
    let mut d = driver(hub);
    exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
    exec(&mut d, hello()).unwrap();
    let mut seen = vec![];
    exec(&mut d, json!({"send": {"type": "req", "id": "1", "verb": "get", "kind": "session", "target": "s1"}, "on": "c1"})).unwrap();
    for _ in 0..40 {
        match next(&mut d, "c1") {
            Poll::Item(v) => seen.push(v),
            Poll::Idle => break,
            _ => {}
        }
    }
    assert_eq!(seen[0]["type"], "welcome");
    // HC EN-5: the Hub never sends a frame over max_frame_bytes: the driver reports it, and drops the rest of the buffer.
    assert!(
        seen[1]["invalid_frame"]
            .as_str()
            .unwrap()
            .contains("over max_frame_bytes"),
        "{seen:?}"
    );
    assert_eq!(seen.len(), 2);

    let (hub, _) = LoopHub::new();
    let mut d = driver(hub);
    exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
    exec(
        &mut d,
        json!({"control": {"op": "emit", "conn": "c1", "bytes_hex": "000000027b7b"}}),
    )
    .unwrap();
    assert!(matches!(next(&mut d, "c1"), Poll::Pending));
    let mut v = None;
    for _ in 0..4 {
        if let Poll::Item(x) = next(&mut d, "c1") {
            v = Some(x);
            break;
        }
    }
    assert!(v.expect("an item")["invalid_frame"].is_string());
}

#[test]
fn a_control_maps_connection_names_to_handles_checks_its_expectation_and_reports_unsupported() {
    let (hub, log) = LoopHub::new();
    let mut d = driver(hub);
    exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
    exec(&mut d, json!({"connect": {"as": "c2"}})).unwrap();
    exec(
        &mut d,
        json!({"control": {"op": "ping", "conn": "c2"}, "expect": {"pong": true, "conn": 1}}),
    )
    .unwrap();
    assert_eq!(
        log.borrow().controls[0],
        ("ping".to_string(), json!({"op": "ping", "conn": 1}))
    );
    assert!(matches!(
        exec(
            &mut d,
            json!({"control": {"op": "ping", "conn": "c2"}, "expect": {"pong": false}})
        ),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut d, json!({"control": {"op": "ping", "conn": "c9"}})),
        Err(StepError::Failed { .. })
    ));
    assert!(
        matches!(exec(&mut d, json!({"control": {"op": "reboot"}})), Err(StepError::Unsupported { op }) if op == "reboot")
    );
    // A variable in a control argument is substituted before the call.
    let mut b = Bindings::new();
    b.insert("n".into(), json!(5));
    d.exec(
        &json!({"control": {"op": "ping", "k": "$n"}}),
        &mut b,
        &Deadline::none(),
    )
    .unwrap();
    assert_eq!(log.borrow().controls.last().unwrap().1["k"], 5);
    assert!(matches!(
        d.exec(
            &json!({"control": {"op": "ping", "k": "$missing"}}),
            &mut Bindings::new(),
            &Deadline::none()
        ),
        Err(StepError::Failed { .. })
    ));
}

#[test]
fn the_clock_moves_and_a_hub_without_an_injected_clock_is_unsupported() {
    let (hub, log) = LoopHub::new();
    let mut d = driver(hub);
    exec(&mut d, json!({"advance_clock": {"ms": 31000}})).unwrap();
    assert_eq!(log.borrow().clock, vec![31000]);
    assert!(matches!(
        exec(&mut d, json!({"advance_clock": {}})),
        Err(StepError::Failed { .. })
    ));
    let (mut hub, _) = LoopHub::new();
    hub.no_clock = true;
    let mut d = driver(hub);
    assert!(
        matches!(exec(&mut d, json!({"advance_clock": {"ms": 1}})), Err(StepError::Unsupported { op }) if op == "advance_clock")
    );
}

#[test]
fn the_driver_reports_the_features_and_controls_of_the_subject() {
    let (hub, _) = LoopHub::new();
    let d = driver(hub);
    assert!(d.has_feature("ui_embed") && !d.has_feature("other"));
    assert!(d.has_control("ping") && !d.has_control("reboot"));
    assert!(!d.is_fake(), "a real subject is never a fake");
}

#[test]
fn a_step_that_the_driver_does_not_know_is_unsupported() {
    let (hub, _) = LoopHub::new();
    let mut d = driver(hub);
    assert!(matches!(
        exec(&mut d, json!({"teleport": {}})),
        Err(StepError::Unsupported { .. })
    ));
    assert!(matches!(
        exec(&mut d, json!("not an object")),
        Err(StepError::Failed { .. })
    ));
}

#[test]
fn route_connect_does_the_handshake_in_control_framing_and_hands_the_route_to_the_route_steps() {
    let (hub, log) = LoopHub::new();
    let steps = json!([
        {"route_connect": {"as": "r1", "route_id": "r-1", "route_token": "t-1", "session": "s1", "uid": 5, "binding": "local-stream"}},
        // The route endpoint sends `live` (Codec TS-4: type 0x06): the route driver reads it from the connection that the Hub handed off.
        {"control": {"op": "emit", "conn": 0, "bytes_hex": "0000000106"}},
        {"expect_eventually": {"frame": "live"}, "ignore": [], "on": "r1", "because": "Codec TS-4"}
    ]);
    run(hub, json!({}), steps).expect("the route is connected and its frame is read");
    let l = log.borrow();
    assert_eq!(l.connects[0], (Some("local-stream".into()), Some(5), None));
    // HC TB-L2: the first frame of the new connection is `route_connect`, in control framing.
    let hs: Value = serde_json::from_slice(&l.writes[0].1[4..]).unwrap();
    assert_eq!(
        hs,
        json!({"frame": "route_connect", "route_id": "r-1", "route_token": "t-1"})
    );
    assert_eq!(
        l.writes[0].1[..4],
        ((l.writes[0].1.len() - 4) as u32).to_be_bytes()
    );
}

#[test]
fn a_handshake_only_route_connect_keeps_a_control_framed_connection() {
    let (hub, _) = LoopHub::new();
    let close =
        serde_json::to_vec(&json!({"type": "close", "reason": "route_token_invalid"})).unwrap();
    let hex = botster_route_codec::prelude::hex_encode(&wrap(&close));
    let steps = json!([
        {"route_connect": {"as": "r1", "route_id": "a", "route_token": "b", "handshake_only": true}},
        {"control": {"op": "emit", "conn": 0, "bytes_hex": hex}},
        // HC TB-L2: the answer of a refused handshake is a typed `close`, read as a control frame.
        {"expect": {"type": "close", "reason": "route_token_invalid"}, "on": "r1", "because": "HC HS-9"}
    ]);
    run(hub, json!({}), steps)
        .expect("the handshake answer is a control frame on the route connection");
}

#[test]
fn the_route_adapter_turns_route_scoped_controls_into_the_worker_control() {
    // `route_expect_pty` asks the route's worker what reached the PTY: the adapter sends the `worker` control with the route's ids.
    struct Pty(LoopHub);
    impl HubSubject for Pty {
        fn has_feature(&self, n: &str) -> bool {
            self.0.has_feature(n)
        }
        fn has_control(&self, op: &str) -> bool {
            op == "worker" || self.0.has_control(op)
        }
        fn configure(&mut self, h: &Value) -> Result<(), String> {
            self.0.configure(h)
        }
        fn start(&mut self) -> Result<(), String> {
            self.0.start()
        }
        fn connect(&mut self, s: &ConnectSpec) -> Result<Option<u32>, String> {
            self.0.connect(s)
        }
        fn write(&mut self, c: u32, b: &[u8]) {
            self.0.write(c, b)
        }
        fn read(&mut self, c: u32, m: usize, d: &Deadline) -> HubRead {
            self.0.read(c, m, d)
        }
        fn close(&mut self, c: u32) {
            self.0.close(c)
        }
        fn advance_clock(&mut self, m: u64) -> Result<(), String> {
            self.0.advance_clock(m)
        }
        fn pump(&mut self) -> Pump {
            self.0.pump()
        }
        fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
            self.0
                .log
                .borrow_mut()
                .controls
                .push((op.to_string(), args.clone()));
            Ok(
                json!({"bytes": {"$bytes_hex": "61"}, "segments": [{"op": null, "bytes": {"$bytes_hex": "61"}}]}),
            )
        }
    }
    let (hub, log) = LoopHub::new();
    let text = json!({"id": "conf::t", "contract": "hc", "clause": "EN-1", "subject": "hub", "steps": [
        {"route_connect": {"as": "r1", "route_id": "r-1", "route_token": "t-1", "session": "s1"}},
        {"route_expect_pty": [{"hex": "61"}], "route": "r1", "because": "Codec IN-1"}
    ]}).to_string();
    let t = parse_transcript("conf::t.json", &text).unwrap();
    let mut d = driver_for(Pty(hub));
    d.setup(&t.setup).unwrap();
    run_steps(
        &t.steps,
        d.as_mut(),
        &mut Bindings::new(),
        &HcSchemas::default(),
        &Limits::default(),
    )
    .expect("the pty bytes came through the worker control");
    let l = log.borrow();
    let (op, args) = l.controls.last().expect("a control");
    assert_eq!(op, "worker");
    assert_eq!(
        (
            args["session"].as_str(),
            args["route_id"].as_str(),
            args["inner"].as_str()
        ),
        (Some("s1"), Some("r-1"), Some("pty_input"))
    );
}

// The generated macros of a Phase 1 Hub repo (one `#[test]` per id) expand and type-check against a `HubSubject`. They are expanded inside a
// function, so that no test runs here: the real Hub runs them in Stage 2.
#[allow(dead_code, unnameable_test_items)]
fn the_hub_macros_expand() {
    fn make(_seed: u64) -> Box<dyn StepDriver> {
        driver_for(LoopHub::new().0)
    }
    botster_hub_conformance::hc_hub_conformance_tests!(make, HcSchemas::default());
    botster_hub_conformance::ui_hub_conformance_tests!(make, HcSchemas::default());
}

#[test]
fn a_frame_of_exactly_max_frame_bytes_is_valid_and_one_more_is_not() {
    // HC EN-5: the bound is inclusive.
    let body = |pad: usize| {
        serde_json::to_vec(
            &json!({"type": "res", "id": "1", "ok": {"id": "s1", "pad": "x".repeat(pad)}}),
        )
        .unwrap()
        .len() as u64
    };
    let len = body(10);
    for (max, valid) in [(len, true), (len - 1, false)] {
        let (mut hub, _) = LoopHub::new();
        hub.max_frame = max;
        hub.pad = 10;
        let mut d = driver(hub);
        exec(&mut d, json!({"connect": {"as": "c1"}})).unwrap();
        // The welcome itself is over the small bound only after it is read, so the first frame passes.
        exec(&mut d, hello()).unwrap();
        exec(&mut d, json!({"send": {"type": "req", "id": "1", "verb": "get", "kind": "session", "target": "s1"}, "on": "c1"})).unwrap();
        let mut items = vec![];
        for _ in 0..60 {
            match next(&mut d, "c1") {
                Poll::Item(v) => items.push(v),
                Poll::Idle => break,
                _ => {}
            }
        }
        assert_eq!(
            items[1].get("invalid_frame").is_none(),
            valid,
            "max {max}: {items:?}"
        );
    }
}

#[test]
fn route_writes_reach_the_hub_and_a_missing_worker_control_fails_the_pty_step() {
    let (hub, log) = LoopHub::new();
    let steps = json!([
        {"route_connect": {"as": "r1", "route_id": "r-1", "route_token": "t-1", "session": "s1"}},
        {"route_send_wire": {"bytes_hex": "00010203"}, "route": "r1"}
    ]);
    run(hub, json!({}), steps).expect("the route accepts a wire write");
    assert_eq!(log.borrow().writes.last().unwrap(), &(0, vec![0, 1, 2, 3]));
    // `route_expect_pty` needs the `worker` control: a Hub without it gives no segments, and the step fails.
    let (hub, _) = LoopHub::new();
    let steps = json!([
        {"route_connect": {"as": "r1", "route_id": "r-1", "route_token": "t-1", "session": "s1"}},
        {"route_expect_pty": [{"hex": "61"}], "route": "r1", "because": "Codec IN-1"}
    ]);
    let r = run(hub, json!({}), steps);
    assert!(failed(&r), "{r:?}");
}

// HC A5-1, A5-4: a route frame that waits in the Hub's scheduler arrives only if a read of the route pumps the subject.
#[test]
fn a_route_read_pumps_the_hub_scheduler() {
    let (hub, log) = LoopHub::new();
    let steps = json!([
        {"route_connect": {"as": "r1", "route_id": "r-1", "route_token": "t-1", "session": "s1"}},
        {"control": {"op": "defer", "conn": 0, "bytes_hex": "0000000106"}},
        {"expect_eventually": {"frame": "live"}, "ignore": [], "on": "r1", "because": "Codec TS-4"}
    ]);
    run(hub, json!({}), steps).expect("the frame was pumped out of the scheduler");
    assert!(log.borrow().pumps > 0);
}

// Design 6.1: a Hub that always reports progress ends a route read at the bound, or at once when the deadline has passed.
#[test]
fn a_route_read_of_a_spinning_hub_is_bounded() {
    for (deadline, most) in [
        (Deadline::none(), 10_001),
        (Deadline::after(Some(std::time::Duration::ZERO)), 3),
    ] {
        let (mut hub, log) = LoopHub::new();
        hub.spin = true;
        let mut d = driver(hub);
        let mut b = Bindings::new();
        d.exec(&json!({"route_connect": {"as": "r1", "route_id": "r", "route_token": "t", "session": "s1"}}), &mut b, &Deadline::none()).unwrap();
        let before = log.borrow().pumps;
        let r = d.exec(
            &json!({"route_drain": true, "route": "r1"}),
            &mut b,
            &deadline,
        );
        // Design 6.1: an execution bound that expires is `inconclusive`, never a pass and never an idle route.
        assert!(matches!(r, Err(StepError::Inconclusive { .. })), "{r:?}");
        let pumped = log.borrow().pumps - before;
        assert!(pumped >= 1 && pumped <= most, "{pumped} pumps");
        if deadline.remaining().is_none() {
            assert!(pumped >= 10_000, "{pumped}");
        }
    }
}
