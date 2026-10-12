//! The client-subject player (HC A5-2, A5-3, R-1.4) against a toy client. The toy is the self-test of the PLAYER: a minimal client that
//! obeys `welcome.limits`, falls back for a major mismatch and ignores unknown members, with one switch for each defect, so that a transcript
//! must pass on the toy and fail on each defective toy (design 7.2 rule 7: an observer must be able to change). It proves nothing about a real
//! client; a client repo runs these transcripts against its own `ClientSubject` in Phase 1 (`hc_client_conformance_tests!`).

// Failure values carry whole JSON documents, as in `botster-conformance` (cold path).
#![allow(clippy::result_large_err)]

use botster_conformance::{
    load_dir, parse_transcript, run_transcript, Bindings, Deadline, Limits, Outcome, Poll, SeedSet,
    StepDriver, StepError,
};
use botster_hub_conformance::hc::{
    player_for, ClientPlayer, ClientSubject, HcSchemas, Pump, HC_TRANSCRIPTS,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Clone, Copy, Default)]
struct Defects {
    /// Keeps fixed limits whatever the `welcome` says (HC BD-3).
    fixed_limits: bool,
    /// Binds a hand-built screen to a kind whatever its major (HC DS-4).
    major_blind: bool,
    /// Refuses a frame that carries an unknown member (HC EN-6).
    strict: bool,
    /// Sends a request that the script does not expect (HC A5-2).
    chatty: bool,
    /// Keeps the limits of the first `welcome` after a reconnect (HC BD-3: a changed value changes the behavior).
    sticky: bool,
}

#[derive(Default)]
struct Log {
    delivered: Vec<Vec<u8>>,
    frames: Vec<Value>,
    closed: bool,
}

struct Toy {
    defects: Defects,
    log: Rc<RefCell<Log>>,
    hand_built: BTreeMap<String, String>,
    limits: (usize, usize, usize),
    out: Vec<Vec<u8>>,
    next_id: u64,
    outstanding: BTreeSet<String>,
    watches: usize,
    max_sent: usize,
    demand: (usize, usize, usize),
    buf: Vec<u8>,
    describe_id: Option<String>,
    generic: BTreeSet<String>,
    delivered: BTreeSet<String>,
    accepted: bool,
    accepted_before: bool,
}

const DEFAULT_LIMITS: (usize, usize, usize) = (8, 64, 16384);

impl Toy {
    fn new(defects: Defects) -> (Self, Rc<RefCell<Log>>) {
        let log = Rc::new(RefCell::new(Log::default()));
        let toy = Toy {
            defects,
            log: log.clone(),
            hand_built: BTreeMap::new(),
            limits: DEFAULT_LIMITS,
            out: vec![],
            next_id: 1,
            outstanding: BTreeSet::new(),
            watches: 0,
            max_sent: 0,
            demand: (0, 0, 0),
            buf: vec![],
            describe_id: None,
            generic: BTreeSet::new(),
            delivered: BTreeSet::new(),
            accepted: false,
            accepted_before: false,
        };
        (toy, log)
    }

    fn write(&mut self, frame: &Value) {
        let body = serde_json::to_vec(frame).unwrap();
        self.max_sent = self.max_sent.max(body.len());
        let mut w = (body.len() as u32).to_be_bytes().to_vec();
        w.extend(body);
        self.out.push(w);
    }

    fn hello(&mut self) {
        self.write(&json!({"type": "hello", "protocol": "botster-hub", "versions": {"min": 1, "max": 2},
            "client": {"name": "toy", "version": "0"}, "features": [], "terminal_formats": ["ghostsnp/2+raw"]}));
    }

    fn id(&mut self) -> String {
        self.next_id += 1;
        (self.next_id - 1).to_string()
    }

    fn on_frame(&mut self, v: &Value) {
        if self.defects.strict && v.get("x_future_field").is_some() {
            return;
        }
        match v["type"].as_str() {
            Some("welcome") => {
                self.accepted = true;
                if !(self.defects.fixed_limits || self.defects.sticky && self.accepted_before) {
                    let l = &v["limits"];
                    self.limits = (
                        l["max_outstanding"].as_u64().unwrap() as usize,
                        l["max_watches"].as_u64().unwrap() as usize,
                        l["max_frame_bytes"].as_u64().unwrap() as usize,
                    );
                }
                self.accepted_before = true;
            }
            Some("res") => {
                let id = v["id"].as_str().unwrap_or_default().to_string();
                self.outstanding.remove(&id);
                self.delivered.insert(id.clone());
                if self.describe_id.as_deref() == Some(id.as_str()) {
                    for k in v["ok"]["kinds"].as_array().into_iter().flatten() {
                        let kind = k["kind"].as_str().unwrap_or_default();
                        let major = k["schema_version"]
                            .as_str()
                            .unwrap_or_default()
                            .split('.')
                            .next();
                        let built = self.hand_built.get(kind).map(String::as_str);
                        if built != major && !self.defects.major_blind {
                            self.generic.insert(kind.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl ClientSubject for Toy {
    fn has_observation(&self, name: &str) -> bool {
        matches!(
            name,
            "generic_screen" | "notice_logged" | "response_delivered" | "welcome_accepted"
        )
    }
    fn configure(&mut self, client: &Value) -> Result<(), String> {
        for (k, v) in client["hand_built"].as_object().into_iter().flatten() {
            self.hand_built
                .insert(k.clone(), v.as_str().unwrap().to_string());
        }
        Ok(())
    }
    fn act(&mut self, action: &Value) -> Result<(), String> {
        if action.get("connect").is_some() {
            self.hello();
        } else if action.get("reconnect").is_some() {
            if !self.defects.sticky {
                self.limits = DEFAULT_LIMITS;
            }
            self.outstanding.clear();
            self.watches = 0;
            self.demand = (0, 0, 0);
            self.describe_id = None;
            self.hello();
        } else if action.get("describe").is_some() {
            let id = self.id();
            self.describe_id = Some(id.clone());
            self.write(&json!({"type": "req", "id": id, "verb": "describe", "kind": "kind"}));
        } else if let Some(d) = action.get("demand") {
            let n = |k: &str| d[k].as_u64().unwrap_or(0) as usize;
            self.demand = (n("requests"), n("watches"), n("payload_bytes"));
        } else if let Some(r) = action.get("request") {
            let id = self.id();
            let mut req = r.clone();
            req["type"] = json!("req");
            req["id"] = json!(id);
            self.outstanding.insert(id);
            self.write(&req);
        } else {
            return Err(format!("unknown action {action}"));
        }
        Ok(())
    }
    fn deliver(&mut self, bytes: &[u8]) {
        self.log.borrow_mut().delivered.push(bytes.to_vec());
        self.buf.extend(bytes);
        while self.buf.len() >= 4 {
            let len =
                u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
            if self.buf.len() < 4 + len {
                break;
            }
            let body: Vec<u8> = self.buf[4..4 + len].to_vec();
            self.buf.drain(..4 + len);
            if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                self.log.borrow_mut().frames.push(v.clone());
                self.on_frame(&v);
            }
        }
    }
    fn close(&mut self) {
        self.log.borrow_mut().closed = true;
    }
    fn pump(&mut self) -> Pump {
        let (limit, watch_limit, frame_limit) = self.limits;
        if self.defects.chatty && self.demand == (0, 0, 0) {
            self.defects.chatty = false;
            let id = self.id();
            self.write(&json!({"type": "req", "id": id, "verb": "delete", "kind": "session", "target": "s1"}));
            return Pump::Progress;
        }
        if self.demand.0 > 0 && self.outstanding.len() < limit {
            self.demand.0 -= 1;
            let id = self.id();
            let mut req =
                json!({"type": "req", "id": id, "verb": "get", "kind": "session", "target": "s1"});
            if self.demand.2 > 0 {
                req["args"] = json!({"pad": "x".repeat(self.demand.2)});
                self.demand.2 = 0;
            }
            if serde_json::to_vec(&req).unwrap().len() > frame_limit {
                return Pump::Progress;
            }
            self.outstanding.insert(id);
            self.write(&req);
            return Pump::Progress;
        }
        if self.demand.1 > 0 && self.watches < watch_limit {
            self.demand.1 -= 1;
            self.watches += 1;
            let id = self.id();
            self.write(&json!({"type": "req", "id": id, "verb": "watch", "kind": "hub"}));
            return Pump::Progress;
        }
        Pump::Quiet
    }
    fn take_written(&mut self) -> Vec<u8> {
        if self.out.is_empty() {
            vec![]
        } else {
            self.out.remove(0)
        }
    }
    fn observe(&mut self, observation: &Value) -> Result<Value, String> {
        let (name, arg) = observation.as_object().unwrap().iter().next().unwrap();
        let kind = arg["kind"].as_str().unwrap_or_default().to_string();
        Ok(match name.as_str() {
            "generic_screen" | "notice_logged" => json!(self.generic.contains(&kind)),
            "response_delivered" => json!(self
                .delivered
                .contains(arg["id"].as_str().unwrap_or_default())),
            "welcome_accepted" => json!(self.accepted),
            _ => json!(false),
        })
    }
}

fn transcript(id: &str) -> botster_conformance::Transcript {
    load_dir(&HC_TRANSCRIPTS)
        .unwrap()
        .into_iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("no {id}"))
}

fn run(t: &botster_conformance::Transcript, defects: Defects) -> (Outcome, Rc<RefCell<Log>>) {
    let (toy, log) = Toy::new(defects);
    let cell = RefCell::new(Some(toy));
    let make = move |_seed: u64| player_for(cell.borrow_mut().take().expect("one seed"));
    let seeds = SeedSet { seeds: vec![1] };
    (
        run_transcript(t, &make, &seeds, &HcSchemas::default(), &Limits::default()),
        log,
    )
}

const CLIENT_ONLY: [&str; 3] = [
    "conf::ds_4_major_mismatch_falls_back",
    "conf::bd_3_client_uses_only_welcome_limits",
    "conf::en_6_unknown_optional_ignored",
];

#[test]
fn the_client_only_transcripts_pass_on_a_conforming_toy_and_the_player_sends_their_frames_in_order()
{
    for id in CLIENT_ONLY {
        let t = transcript(id);
        let (outcome, log) = run(&t, Defects::default());
        assert_eq!(outcome, Outcome::Passed, "{id}");
        let log = log.borrow();
        let scripted: Vec<&Value> = t.steps.iter().filter_map(|s| s.get("send")).collect();
        assert_eq!(log.frames.len(), scripted.len(), "{id}");
        // Welcome frames go out exactly as scripted, in order (HC TR-1: `[u32 length][JSON]`, one delivery per frame).
        for (got, want) in log.frames.iter().zip(&scripted) {
            if want["type"] == "welcome" {
                assert_eq!(got, *want, "{id}");
            }
        }
        for (bytes, frame) in log.delivered.iter().zip(&log.frames) {
            assert_eq!(bytes[..4], ((bytes.len() - 4) as u32).to_be_bytes());
            assert_eq!(bytes[4..], serde_json::to_vec(frame).unwrap()[..]);
        }
    }
}

#[test]
fn each_defect_of_the_toy_fails_its_transcript() {
    let cases = [
        (
            "conf::bd_3_client_uses_only_welcome_limits",
            Defects {
                fixed_limits: true,
                ..Defects::default()
            },
        ),
        (
            "conf::bd_3_client_uses_only_welcome_limits",
            Defects {
                sticky: true,
                ..Defects::default()
            },
        ),
        (
            "conf::ds_4_major_mismatch_falls_back",
            Defects {
                major_blind: true,
                ..Defects::default()
            },
        ),
        (
            "conf::en_6_unknown_optional_ignored",
            Defects {
                strict: true,
                ..Defects::default()
            },
        ),
    ];
    for (id, defects) in cases {
        let (outcome, _) = run(&transcript(id), defects);
        assert!(
            matches!(outcome, Outcome::Failed { .. }),
            "{id}: {outcome:?}"
        );
    }
}

#[test]
fn a_client_frame_that_the_script_did_not_match_fails_the_test() {
    // HC A5-2: the toy sends an unscripted `delete` after the handshake; the script ends with `client_idle`.
    let t = transcript("conf::en_6_unknown_optional_ignored");
    let (outcome, _) = run(
        &t,
        Defects {
            chatty: true,
            ..Defects::default()
        },
    );
    assert!(matches!(outcome, Outcome::Failed { .. }), "{outcome:?}");
    // The same frame fails an observation too, not only the final check.
    let text = json!({"id": "conf::t", "contract": "hc", "clause": "EN-6", "subject": "client", "steps": [
        {"client_act": {"connect": true}},
        {"expect_eventually": {"type": "hello"}, "ignore": [], "on": "c1", "because": "HC HS-1"},
        {"send": {"type": "welcome", "hub_id": "h", "incarnation": "i", "version": 2, "features": [], "terminal_format": "x",
            "limits": {"max_frame_bytes": 4096, "max_chunk_bytes": 1024, "max_outstanding": 8, "max_watches": 64, "route_connect_seconds": 10,
            "idempotency_window_seconds": 60, "cursor_idle_seconds": 30, "operation_retention_seconds": 600, "max_paste_bytes": 1024,
            "max_file_bytes": 1024, "query_client_deadline_ms": 100}}, "on": "c1"},
        {"client_expect": {"observation": {"welcome_accepted": {}}, "because": "HC HS-3"}}]}).to_string();
    let t = parse_transcript("conf::t.json", &text).unwrap();
    let (outcome, _) = run(
        &t,
        Defects {
            chatty: true,
            ..Defects::default()
        },
    );
    assert!(
        matches!(outcome, Outcome::Failed { step: 3, .. }),
        "{outcome:?}"
    );
    let (outcome, _) = run(&t, Defects::default());
    assert_eq!(outcome, Outcome::Passed);
}

/// A client that always reports progress.
struct Busy(usize);
impl ClientSubject for Busy {
    fn has_observation(&self, _: &str) -> bool {
        true
    }
    fn configure(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn act(&mut self, _: &Value) -> Result<(), String> {
        Err("no".into())
    }
    fn deliver(&mut self, _: &[u8]) {}
    fn close(&mut self) {}
    fn pump(&mut self) -> Pump {
        self.0 += 1;
        Pump::Progress
    }
    fn take_written(&mut self) -> Vec<u8> {
        vec![]
    }
    fn observe(&mut self, _: &Value) -> Result<Value, String> {
        Ok(json!(true))
    }
}

#[test]
fn a_client_that_never_comes_to_quiet_ends_the_wait_instead_of_hanging_the_runner() {
    let observe = json!({"client_expect": {"observation": {"generic_screen": {}}}});
    let mut p = ClientPlayer::new(Busy(0));
    let r = p.exec(&observe, &mut Bindings::new(), &Deadline::none());
    assert!(matches!(r, Err(StepError::Inconclusive { .. })), "{r:?}");
    let r = p.exec(
        &json!({"client_idle": {}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Inconclusive { .. })), "{r:?}");
    assert_eq!(p.next(&json!("c1"), &Deadline::none()), Poll::TimedOut);
    // An expired deadline stops the wait before the client runs at all.
    let mut p = ClientPlayer::new(Busy(0));
    let expired = Deadline::after(Some(std::time::Duration::ZERO));
    let r = p.exec(&observe, &mut Bindings::new(), &expired);
    assert!(matches!(r, Err(StepError::Inconclusive { .. })), "{r:?}");
    assert_eq!(p.next(&json!("c1"), &expired), Poll::TimedOut);
}

fn player() -> (ClientPlayer<Toy>, Rc<RefCell<Log>>) {
    let (c, log) = Toy::new(Defects::default());
    (ClientPlayer::new(c), log)
}

fn exec(p: &mut ClientPlayer<Toy>, step: Value) -> Result<(), StepError> {
    p.exec(&step, &mut Bindings::new(), &Deadline::none())
}

// R-1.4: the transport drops mid-frame (TS-9), a malformed frame, and a cut at any byte.
#[test]
fn the_faults_of_the_player_deliver_what_the_script_says() {
    let (mut p, log) = player();
    exec(&mut p, json!({"send_raw": {"bytes_hex": "7b7b"}})).unwrap();
    exec(&mut p, json!({"send_wire": {"bytes_hex": "00000010"}})).unwrap();
    exec(&mut p, json!({"send_wire": {"bytes_hex": "7b22"}})).unwrap();
    assert_eq!(
        log.borrow().delivered,
        vec![
            vec![0, 0, 0, 2, 0x7b, 0x7b],
            vec![0, 0, 0, 0x10],
            vec![0x7b, 0x22]
        ]
    );
    assert!(!log.borrow().closed);
    exec(&mut p, json!({"disconnect": {}})).unwrap();
    assert!(
        log.borrow().closed,
        "the transport drops after half a frame"
    );
    assert!(
        matches!(
            exec(&mut p, json!({"send": {"type": "close", "reason": "busy"}})),
            Err(StepError::Failed { .. })
        ),
        "nothing is sent on a dropped transport"
    );
    assert!(matches!(
        exec(&mut p, json!({"send_wire": {"bytes_hex": "00"}})),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut p, json!({"send_raw": {}})),
        Err(StepError::Failed { .. })
    ));
    // After the drop the client's response to the loss is observed and checked for stray output.
    exec(&mut p, json!({"client_idle": {}})).unwrap();
    exec(
        &mut p,
        json!({"client_expect": {"observation": {"generic_screen": {}}}}),
    )
    .unwrap_err();
    assert!(matches!(
        exec(&mut p, json!({"teleport": 1})),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut p, json!(7)),
        Err(StepError::Failed { .. })
    ));
    assert!(matches!(
        exec(&mut p, json!({"client_act": {"levitate": true}})),
        Err(StepError::Failed { .. })
    ));
}

#[test]
fn the_frames_that_the_client_writes_are_reassembled_and_a_drop_gives_one_close() {
    let (mut p, _) = player();
    let none = Deadline::none();
    // `request` makes the toy write a frame; it comes out as one item, and the next poll is idle.
    exec(
        &mut p,
        json!({"client_act": {"request": {"verb": "get", "kind": "session", "target": "s1"}}}),
    )
    .unwrap();
    match p.next(&json!("c1"), &none) {
        Poll::Item(v) => assert_eq!(
            (v["type"].as_str(), v["verb"].as_str()),
            (Some("req"), Some("get"))
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(p.next(&json!("c1"), &none), Poll::Idle);
    exec(&mut p, json!({"disconnect": {}})).unwrap();
    assert_eq!(
        p.next(&json!("c1"), &none),
        Poll::Closed(json!({"eof": true}))
    );
    assert_eq!(
        p.next(&json!("c1"), &none),
        Poll::Idle,
        "the close is given once"
    );
    assert_eq!(
        p.next(
            &json!("c1"),
            &Deadline::after(Some(std::time::Duration::ZERO))
        ),
        Poll::TimedOut
    );
}

/// A client that writes in two halves, and a body that is not JSON.
struct Halves(Vec<Vec<u8>>);
impl ClientSubject for Halves {
    fn has_observation(&self, _: &str) -> bool {
        false
    }
    fn configure(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn act(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn deliver(&mut self, _: &[u8]) {}
    fn close(&mut self) {}
    fn pump(&mut self) -> Pump {
        if self.0.is_empty() {
            Pump::Quiet
        } else {
            Pump::Progress
        }
    }
    fn take_written(&mut self) -> Vec<u8> {
        if self.0.is_empty() {
            vec![]
        } else {
            self.0.remove(0)
        }
    }
    fn observe(&mut self, _: &Value) -> Result<Value, String> {
        Err("none".into())
    }
}

#[test]
fn a_frame_in_pieces_arrives_whole_and_a_body_that_is_not_json_is_an_invalid_frame() {
    let frame = br#"{"type":"req","id":"1"}"#;
    let mut wire = (frame.len() as u32).to_be_bytes().to_vec();
    wire.extend(frame);
    let (a, b) = wire.split_at(6);
    let mut p = ClientPlayer::new(Halves(vec![
        a.to_vec(),
        b.to_vec(),
        vec![0, 0, 0, 2, b'{', b'{'],
    ]));
    let none = Deadline::none();
    assert_eq!(
        p.next(&json!("c1"), &none),
        Poll::Item(json!({"type": "req", "id": "1"}))
    );
    match p.next(&json!("c1"), &none) {
        Poll::Item(v) => assert!(v["invalid_frame"].is_string(), "{v}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(p.next(&json!("c1"), &none), Poll::Idle);
}

#[test]
fn the_player_knows_the_controls_of_its_client_and_accepts_every_feature() {
    let (p, _) = player();
    assert!(p.has_control("generic_screen") && !p.has_control("other"));
    assert!(p.has_feature("anything"));
    assert!(!p.is_fake());
}

#[test]
fn an_observation_that_the_client_cannot_make_is_unsupported_never_a_pass() {
    let t = parse_transcript(
        "conf::t.json",
        &json!({"id": "conf::t", "contract": "hc", "clause": "DS-4", "subject": "client", "steps": [
            {"client_expect": {"observation": {"levitating": {}}, "because": "HC DS-4"}}]})
        .to_string(),
    )
    .unwrap();
    let (outcome, _) = run(&t, Defects::default());
    assert_eq!(
        outcome,
        Outcome::UnsupportedControl {
            op: "levitating".into()
        }
    );
    // A client that offers a name the player does not define is unsupported too.
    let mut p = ClientPlayer::new(Busy(0));
    let r = p.exec(
        &json!({"client_expect": {"observation": {"levitating": {}}}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(
        matches!(r, Err(StepError::Inconclusive { .. })),
        "settling comes first: {r:?}"
    );
    let r = p.exec(
        &json!({"client_expect": {}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Failed { .. })));
    let r = p.exec(
        &json!({"client_expect": {"observation": {}}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Failed { .. })));
}

#[test]
fn a_setup_failure_of_the_client_is_a_failed_step() {
    struct Bad;
    impl ClientSubject for Bad {
        fn has_observation(&self, _: &str) -> bool {
            true
        }
        fn configure(&mut self, _: &Value) -> Result<(), String> {
            Err("no such kind".into())
        }
        fn act(&mut self, _: &Value) -> Result<(), String> {
            Ok(())
        }
        fn deliver(&mut self, _: &[u8]) {}
        fn close(&mut self) {}
        fn pump(&mut self) -> Pump {
            Pump::Quiet
        }
        fn take_written(&mut self) -> Vec<u8> {
            vec![]
        }
        fn observe(&mut self, _: &Value) -> Result<Value, String> {
            Err("never".into())
        }
    }
    let mut p = ClientPlayer::new(Bad);
    assert!(matches!(
        p.setup(&json!({"client": {}})),
        Err(StepError::Failed { .. })
    ));
    let r = p.exec(
        &json!({"client_expect": {"observation": {"generic_screen": {}}}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Unsupported { .. })), "{r:?}");
}

// The macro of a Phase 1 client repo: one `#[test]` per client-only id, over a `ClientSubject`. The toy is a conforming client, so this checks
// the generated tests and the player wiring together, and nothing about a real client.
mod generated {
    use super::*;

    fn make(_seed: u64) -> Box<dyn StepDriver> {
        player_for(Toy::new(Defects::default()).0)
    }

    botster_hub_conformance::hc_client_conformance_tests!(make, HcSchemas::default());
}

// HC A5-2: bytes of a frame that the client started and did not finish are output that the script did not match.
#[test]
fn unfinished_unscripted_client_output_fails_the_idle_check() {
    let mut p = ClientPlayer::new(Halves(vec![vec![0, 0, 0, 100, 123]]));
    let r = p.exec(
        &json!({"client_idle": {}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Failed { .. })), "{r:?}");
    let mut p = ClientPlayer::new(Halves(vec![vec![0, 0, 0, 100, 123]]));
    let r = p.exec(
        &json!({"client_requests": {"at_most": 5}}),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Failed { .. })), "{r:?}");
}

// HC BD-3 fixes maxima only: a client that sends fewer, or none, passes; one frame over a bound, or one request over the count, fails.
#[test]
fn client_requests_checks_maxima_and_kinds_only() {
    let req = |id: &str, verb: &str, pad: usize| {
        let body = serde_json::to_vec(
            &json!({"type": "req", "id": id, "verb": verb, "kind": "hub", "pad": "x".repeat(pad)}),
        )
        .unwrap();
        let mut w = (body.len() as u32).to_be_bytes().to_vec();
        w.extend(&body);
        (w, body.len())
    };
    let run = |frames: Vec<Vec<u8>>, spec: Value| {
        let mut p = ClientPlayer::new(Halves(frames));
        p.exec(
            &json!({"client_requests": spec}),
            &mut Bindings::new(),
            &Deadline::none(),
        )
    };
    assert!(
        run(vec![], json!({"at_most": 0})).is_ok(),
        "no work is allowed"
    );
    // Design 5.4: a request without `id` or `kind` is refused before it is counted.
    let bad = serde_json::to_vec(&json!({"type": "req", "verb": "get"})).unwrap();
    let mut w = (bad.len() as u32).to_be_bytes().to_vec();
    w.extend(bad);
    assert!(
        matches!(
            run(vec![w], json!({"at_most": 1})),
            Err(StepError::Failed { .. })
        ),
        "a malformed request"
    );
    let (a, alen) = req("1", "get", 10);
    let (b, _) = req("2", "get", 10);
    assert!(run(vec![a.clone(), b.clone()], json!({"at_most": 2})).is_ok());
    assert!(matches!(
        run(vec![a.clone(), b], json!({"at_most": 1})),
        Err(StepError::Failed { .. })
    ));
    assert!(
        run(
            vec![a.clone()],
            json!({"at_most": 1, "max_frame_bytes": alen})
        )
        .is_ok(),
        "the bound is inclusive"
    );
    assert!(matches!(
        run(
            vec![a.clone()],
            json!({"at_most": 1, "max_frame_bytes": alen - 1})
        ),
        Err(StepError::Failed { .. })
    ));
    assert!(run(vec![a.clone()], json!({"at_most": 1, "verb": "get"})).is_ok());
    assert!(matches!(
        run(vec![a], json!({"at_most": 1, "verb": "watch"})),
        Err(StepError::Failed { .. })
    ));
    let hello = serde_json::to_vec(&json!({"type": "hello"})).unwrap();
    let mut w = (hello.len() as u32).to_be_bytes().to_vec();
    w.extend(hello);
    assert!(
        matches!(
            run(vec![w], json!({"at_most": 3})),
            Err(StepError::Failed { .. })
        ),
        "a frame that is not a request"
    );
    assert!(
        matches!(run(vec![], json!({})), Err(StepError::Failed { .. })),
        "needs at_most"
    );
}
