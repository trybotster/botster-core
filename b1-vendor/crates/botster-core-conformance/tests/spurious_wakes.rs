//! Steward ruling R-43(5): the driver applies `no_spurious_wakes` at the start of a transcript that has a strict `Woken` wait,
//! when the harness has the control (`CoreDriver::start`). Core TH-2 allows a spurious `Woken`, and A5-2 seeds them, so without
//! the rule a strict `Woken` passes on a seeded harness although the call under test signals nothing.

use botster_conformance::load::parse_transcript;
use botster_conformance::{
    run_transcript, Limits, NoSchemas, Outcome, SeedSet, StepError, Transcript,
};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    driver_for, run_script_events, strict_woken_wait, ControlError, CoreHarness, DataDirRef,
    OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The wake of the seeded harness: a level flag, and seeded spurious wakes until `no_spurious_wakes`.
#[derive(Default)]
struct WakeState {
    signaled: bool,
    no_spurious: bool,
    /// The thread of each wait, in order.
    waiters: Vec<std::thread::ThreadId>,
}

/// The worst seed of A5-2: every wait with no signal gives a spurious `Woken`, until `no_spurious_wakes`.
struct SeededWake(Arc<Mutex<WakeState>>);

impl WakeHandle for SeededWake {
    fn wait(&self, _timeout: Duration) -> Wake {
        let mut w = self.0.lock().unwrap();
        w.waiters.push(std::thread::current().id());
        if w.signaled || !w.no_spurious {
            Wake::Woken
        } else {
            Wake::TimedOut
        }
    }
    fn fd(&self) -> std::os::fd::RawFd {
        -1
    }
}

/// FakeCore with the seeded wake. `begin` signals it (Core TM-6 (i)), unless `drop_signal` is set (the defect under test), and
/// `pump` leaves it set only while runnable work remains (TM-6 (ii)).
struct Seeded {
    inner: Box<dyn CoreApi>,
    wake: Arc<Mutex<WakeState>>,
    drop_signal: bool,
}

impl CoreApi for Seeded {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        let id = self.inner.begin(op)?;
        if !self.drop_signal {
            self.wake.lock().unwrap().signaled = true;
        }
        Ok(id)
    }
    fn pump(&mut self, now: Now) -> PumpReport {
        let report = self.inner.pump(now);
        self.wake.lock().unwrap().signaled = report.more;
        report
    }
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.inner.poll_events(max)
    }
    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        Arc::new(SeededWake(self.wake.clone()))
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.inner.cancel(op)
    }
    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.inner.get(id)
    }
    fn list(&self) -> Vec<SessionRecord> {
        self.inner.list()
    }
    fn status(&self) -> Status {
        self.inner.status()
    }
    fn diagnostics(&self) -> Value {
        self.inner.diagnostics()
    }
    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.inner.terminal_state(id)
    }
    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.inner.read_page(capture, page)
    }
    fn release(&mut self, capture: CaptureId) {
        self.inner.release(capture)
    }
    fn release_owner(&mut self, client: &ClientId) {
        self.inner.release_owner(client)
    }
    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.inner.snapshot_formats(session)
    }
    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.inner.shadow_answerable_kinds()
    }
    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        self.inner.attach(client, session, transport, options)
    }
    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.inner.tap_read(session, max)
    }
    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.inner.set_silence_threshold(session, threshold)
    }
    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.inner.service_send(id, lane, frame)
    }
    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.inner.service_recv(id, lane)
    }
    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.inner.service_report(id)
    }
    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.inner.service_log_tail(id, max)
    }
    fn features(&self) -> Features {
        self.inner.features()
    }
    fn limits(&self) -> CoreLimits {
        self.inner.limits()
    }
    fn worker_protocol(&self) -> u8 {
        self.inner.worker_protocol()
    }
    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.inner.adoptable_worker_protocols()
    }
    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.inner.worker_protocol_compatibility(protocol)
    }
    fn terminal_identity(&self) -> TerminalIdentity {
        self.inner.terminal_identity()
    }
}

/// A seeded harness (not a fake): FakeCore with the seeded wake. With `offers_control`, it has `no_spurious_wakes` (the
/// testkit); without it, it is the real tier. `fail_control` is the answer of a control that fails.
struct Harness {
    inner: FakeCoreHarness,
    wake: Arc<Mutex<WakeState>>,
    drop_signal: bool,
    offers_control: bool,
    fail_control: Option<ControlError>,
    /// The controls that the harness ran, in order.
    controls: Arc<Mutex<Vec<String>>>,
}

impl Harness {
    fn new(drop_signal: bool, offers_control: bool) -> Self {
        Harness {
            inner: FakeCoreHarness::new(0),
            wake: Arc::default(),
            drop_signal,
            offers_control,
            fail_control: None,
            controls: Arc::default(),
        }
    }
}

impl CoreHarness for Harness {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        Ok(Box::new(Seeded {
            inner: self.inner.open(spec)?,
            wake: self.wake.clone(),
            drop_signal: self.drop_signal,
        }))
    }
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        self.inner.data_dir(name)
    }
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        self.inner.worker(which)
    }
    fn drop_handle(&mut self, handle: &str) {
        self.inner.drop_handle(handle)
    }
    fn injects_clock(&self) -> bool {
        true
    }
    fn has_control(&self, op: &str) -> bool {
        (self.offers_control && op == "no_spurious_wakes") || self.inner.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        if op != "no_spurious_wakes" {
            return self.inner.control(handle, op, args);
        }
        self.controls.lock().unwrap().push(op.to_string());
        if let Some(e) = self.fail_control.take() {
            return Err(e);
        }
        self.wake.lock().unwrap().no_spurious = true;
        Ok(Value::Null)
    }
    fn probe_binary(&self) -> String {
        self.inner.probe_binary()
    }
    fn attach_stream(
        &mut self,
        handle: &str,
        core: &mut dyn CoreApi,
        client: ClientId,
        session: &SessionId,
        options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        self.inner
            .attach_stream(handle, core, client, session, options)
    }
}

/// The call under test: a `begin` that must signal the wake (Core TM-6 (i)).
fn begin() -> Value {
    json!({"begin": {"op": {"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}}})
}

fn transcript(steps: Value) -> Transcript {
    let doc = json!({"id": "conf::t", "contract": "core", "clause": "TM-6", "subject": "core", "steps": steps});
    parse_transcript("conf::t.json", &doc.to_string()).unwrap()
}

/// Runs the transcript through the runner (`run_transcript`, so `start` runs as in a suite) on the harness, and returns the
/// outcome and the controls that the harness ran.
fn run(t: &Transcript, harness: Harness) -> (Outcome, Vec<String>) {
    let controls = harness.controls.clone();
    let harness = Mutex::new(Some(harness));
    let make = |_: u64| driver_for(Box::new(harness.lock().unwrap().take().unwrap()));
    let outcome = run_transcript(
        t,
        &make,
        &SeedSet { seeds: vec![0] },
        &NoSchemas,
        &Limits::default(),
    );
    let controls = controls.lock().unwrap().clone();
    (outcome, controls)
}

fn strict() -> Transcript {
    transcript(
        json!([begin(), {"wait_wake": {"timeout_ms": 0}, "expect": "Woken", "because": "Core TM-6"}]),
    )
}

/// The negative proof: with seeded spurious wakes on and a Core whose `begin` drops its signal, the strict `Woken` step fails,
/// because the driver turned the seeded spurious wakes off before the first step.
#[test]
fn a_dropped_signal_fails_the_strict_woken_step_on_a_seeded_harness() {
    let (outcome, controls) = run(&strict(), Harness::new(true, true));
    assert_eq!(controls, vec!["no_spurious_wakes".to_string()]);
    match outcome {
        Outcome::Failed {
            step: 1, received, ..
        } => {
            assert_eq!(received["wake"], json!("TimedOut"), "{received}")
        }
        other => panic!("the strict Woken step must fail: {other:?}"),
    }
}

/// The same Core passes when its `begin` signals: the rule does not break a correct Core.
#[test]
fn a_kept_signal_passes_the_strict_woken_step_on_a_seeded_harness() {
    assert_eq!(
        run(&strict(), Harness::new(false, true)),
        (Outcome::Passed, vec!["no_spurious_wakes".to_string()])
    );
}

/// Without the control (the real tier) the driver applies and refuses nothing. The seeded spurious wake then passes the strict
/// step although `begin` dropped its signal: the false pass that the rule removes where the control exists, and the accepted
/// residual of the real tier.
#[test]
fn without_the_control_the_driver_applies_nothing_and_a_spurious_wake_passes() {
    assert_eq!(
        run(&strict(), Harness::new(true, false)),
        (Outcome::Passed, vec![])
    );
}

/// A wait with no `expect` is strict too: the step's default is `Woken`.
#[test]
fn a_wait_with_no_expect_is_strict() {
    let t = transcript(json!([begin(), {"wait_wake": {"timeout_ms": 0}}]));
    let (outcome, controls) = run(&t, Harness::new(true, true));
    assert_eq!(controls, vec!["no_spurious_wakes".to_string()]);
    assert!(
        matches!(outcome, Outcome::Failed { step: 1, .. }),
        "{outcome:?}"
    );
}

/// A transcript with no strict `Woken` wait keeps the seeded spurious wakes: the driver runs no control, and the spurious wake
/// still comes.
#[test]
fn a_transcript_with_no_strict_wait_keeps_the_seeded_spurious_wakes() {
    let t = transcript(json!([
        {"wait_wake": {"timeout_ms": 0}, "expect": {"$one_of": ["Woken", "TimedOut"]}, "because": "Core TH-2"},
        {"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut", "because": "Core TH-2"}
    ]));
    let (outcome, controls) = run(&t, Harness::new(false, true));
    assert_eq!(controls, Vec::<String>::new());
    assert!(
        matches!(&outcome, Outcome::Failed { step: 1, received, .. } if received["wake"] == json!("Woken")),
        "the seeded spurious wake must still come: {outcome:?}"
    );
}

/// A control that the harness has but that fails ends the run at step 0: the transcript never runs without the control.
#[test]
fn a_failed_control_fails_the_transcript_at_step_0() {
    let mut harness = Harness::new(false, true);
    harness.fail_control = Some(ControlError::Bad("no scheduler".into()));
    match run(&strict(), harness) {
        (
            Outcome::Failed {
                step: 0, received, ..
            },
            controls,
        ) => {
            assert_eq!(controls, vec!["no_spurious_wakes".to_string()]);
            assert!(received.to_string().contains("no scheduler"), "{received}");
        }
        other => panic!("the failed control must fail step 0: {other:?}"),
    }
    let mut harness = Harness::new(false, true);
    harness.fail_control = Some(ControlError::Unsupported);
    assert_eq!(
        run(&strict(), harness).0,
        Outcome::UnsupportedControl {
            op: "no_spurious_wakes".into()
        }
    );
}

/// `run_script_events` (Core A5-1's `run_deterministic`) builds its own driver with no runner: it applies the same rule.
#[test]
fn run_script_events_applies_the_rule() {
    let steps = [
        json!({"open": {"as": "h", "data_dir": "d"}}),
        begin(),
        json!({"wait_wake": {"timeout_ms": 0}, "expect": "Woken"}),
    ];
    let error = run_script_events(Box::new(Harness::new(true, true)), &steps).unwrap_err();
    assert!(
        matches!(&error, StepError::Failed { received, .. } if received["wake"] == json!("TimedOut")),
        "{error:?}"
    );
    let events = run_script_events(Box::new(Harness::new(true, false)), &steps);
    assert!(events.is_ok(), "{events:?}");
}

/// A script goes to `exec` step by step, with no order construct, so an `ack` in a script is never executed: it applies no
/// control.
#[test]
fn run_script_events_reads_only_direct_steps() {
    let harness = Harness::new(true, true);
    let controls = harness.controls.clone();
    let steps = [
        json!({"open": {"as": "h", "data_dir": "d"}}),
        json!({"expect_eventually": {"x": 1}, "ack": {"wait_wake": {"timeout_ms": 0}, "expect": "Woken"}}),
    ];
    // `exec` does not run an order construct; only the control matters here.
    let _ = run_script_events(Box::new(harness), &steps);
    assert_eq!(*controls.lock().unwrap(), Vec::<String>::new());
}

/// GS-R43-1 (017c): `exec` dispatches `wait_wake` before every other step kind, and the scan reads the same predicate. A step
/// with `wait_wake` and another step key runs the wait (here it fails with `TimedOut`, because the rule applied the control and
/// `begin` dropped its signal), and an `ack` on a driver step neither runs nor applies the control.
#[test]
fn the_scan_and_exec_agree_on_which_step_is_a_wait() {
    let t = transcript(json!([
        begin(),
        {"data_dir": "extra", "wait_wake": {"timeout_ms": 0}, "expect": "Woken", "because": "Core TM-6"}
    ]));
    let (outcome, controls) = run(&t, Harness::new(true, true));
    assert_eq!(controls, vec!["no_spurious_wakes".to_string()]);
    assert!(
        matches!(&outcome, Outcome::Failed { step: 1, received, .. } if received["wake"] == json!("TimedOut")),
        "{outcome:?}"
    );
    // 017c's example: the `ack` of a `data_dir` step is not executed, so no control, and the seeded wake stays.
    let t = transcript(json!([
        {"data_dir": "extra", "ack": {"wait_wake": {}, "expect": "Woken"}},
        {"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut", "because": "Core TH-2"}
    ]));
    let (outcome, controls) = run(&t, Harness::new(false, true));
    assert_eq!(controls, Vec::<String>::new());
    assert!(
        matches!(&outcome, Outcome::Failed { step: 1, received, .. } if received["wake"] == json!("Woken")),
        "the seeded spurious wake must still come: {outcome:?}"
    );
}

/// The scan reads the executable steps: a step, the `ack` of `expect_set` and `expect_eventually`, and the `barrier` of
/// `expect_quiet_until`, by the runner's own recognition of the construct. Matchers, payloads, and an `ack` or `barrier` field
/// that the runner does not execute are not steps.
#[test]
fn the_scan_finds_strict_waits_in_executable_steps_only() {
    let wait = |expect: Option<Value>| {
        let mut w = json!({"wait_wake": {"timeout_ms": 0}});
        if let Some(e) = expect {
            w["expect"] = e;
        }
        w
    };
    assert!(strict_woken_wait(&wait(Some(json!("Woken")))));
    assert!(strict_woken_wait(&wait(None)));
    assert!(!strict_woken_wait(&wait(Some(json!("TimedOut")))));
    assert!(!strict_woken_wait(&wait(Some(
        json!({"$one_of": ["Woken", "TimedOut"]})
    ))));
    assert!(!strict_woken_wait(&json!({"pump": {}})));
    // Nested executable steps.
    assert!(strict_woken_wait(
        &json!({"expect_eventually": {"x": 1}, "ack": wait(None)})
    ));
    assert!(strict_woken_wait(
        &json!({"expect_set": [{"x": 1}], "ack": wait(Some(json!("Woken")))})
    ));
    assert!(strict_woken_wait(
        &json!({"expect_quiet_until": {"x": 1}, "barrier": wait(Some(json!("Woken"))), "until": {"y": 1}})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_quiet_until": {"x": 1}, "barrier": wait(Some(json!("TimedOut"))), "until": {"y": 1}})
    ));
    // Matchers and payloads that hold the same text are not steps.
    assert!(!strict_woken_wait(&json!({"expect": wait(None)})));
    assert!(!strict_woken_wait(
        &json!({"expect_quiet_until": wait(None), "barrier": {"pump": {}}, "until": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"control": {"op": "echo", "step": wait(None)}})
    ));
    assert!(!strict_woken_wait(
        &json!({"pump": {}, "expect": {"steps": [wait(None)]}})
    ));
    // An `ack` or `barrier` field that the runner does not execute (GS-R43-R1-1): a driver step, or a construct that does not
    // take the field, is not an order construct with that nested step.
    assert!(!strict_woken_wait(&json!({"pump": {}, "ack": wait(None)})));
    assert!(!strict_woken_wait(
        &json!({"pump": {}, "barrier": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut", "ack": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect": {"x": 1}, "ack": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_set": [{"x": 1}], "barrier": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_eventually": {"x": 1}, "barrier": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_quiet_until": {"x": 1}, "ack": wait(None), "until": {"y": 1}})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_closed": null, "ack": wait(None)})
    ));
    // GS-R43-1 round 2 (017c): the runner gives an `ack` to `exec` directly, once for each item. An empty `expect_set` (or one
    // whose value is not a list) never executes it; an `ack` that is itself an order construct is not one to `exec`. A
    // `barrier` goes through `run_step`, so an order construct in it executes its own `ack`.
    assert!(!strict_woken_wait(
        &json!({"expect_set": [], "ack": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_set": {"x": 1}, "ack": wait(None)})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_set": [{"x": 1}], "ack": {"pump": {}}})
    ));
    assert!(!strict_woken_wait(
        &json!({"expect_eventually": {"x": 1}, "ack": {"expect_eventually": {"y": 1}, "ack": wait(None)}})
    ));
    assert!(strict_woken_wait(
        &json!({"expect_quiet_until": {"x": 1}, "barrier": {"expect_eventually": {"y": 1}, "ack": wait(None)}, "until": {"z": 1}})
    ));
}

/// GS-R43-1 round 2 (017c's example): an empty `expect_set` never executes its strict `ack`, so no control is applied and the
/// seeded wake stays.
#[test]
fn an_empty_expect_set_keeps_the_seeded_spurious_wakes() {
    let t = transcript(json!([
        {"expect_set": [], "ack": {"wait_wake": {}, "expect": "Woken"}},
        {"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut", "because": "Core TH-2"}
    ]));
    let (outcome, controls) = run(&t, Harness::new(false, true));
    assert_eq!(controls, Vec::<String>::new());
    assert!(
        matches!(&outcome, Outcome::Failed { step: 1, received, .. } if received["wake"] == json!("Woken")),
        "the seeded spurious wake must still come: {outcome:?}"
    );
}

/// GS-R43-R1-1: a strict wait in an `ack` field that the runner does not execute leaves the seeded spurious wakes on.
#[test]
fn a_strict_wait_in_a_field_that_is_not_executed_keeps_the_seeded_spurious_wakes() {
    let t = transcript(json!([
        {"pump": {}, "ack": {"wait_wake": {"timeout_ms": 0}, "expect": "Woken"}},
        {"wait_wake": {"timeout_ms": 0}, "expect": {"$one_of": ["Woken", "TimedOut"]}, "because": "Core TH-2"},
        {"wait_wake": {"timeout_ms": 0}, "expect": "TimedOut", "because": "Core TH-2"}
    ]));
    let (outcome, controls) = run(&t, Harness::new(false, true));
    assert_eq!(controls, Vec::<String>::new());
    assert!(
        matches!(&outcome, Outcome::Failed { step: 2, received, .. } if received["wake"] == json!("Woken")),
        "the seeded spurious wake must still come: {outcome:?}"
    );
}

/// Core TH-2: `"other_thread": true` waits on another thread while the owner thread holds the handle, and a wait without it
/// waits on the owner thread.
#[test]
fn other_thread_waits_on_another_thread() {
    let t = transcript(json!([
        {"wait_wake": {"timeout_ms": 0, "other_thread": true}, "expect": {"$one_of": ["Woken", "TimedOut"]}, "because": "Core TH-2"},
        {"wait_wake": {"timeout_ms": 0}, "expect": {"$one_of": ["Woken", "TimedOut"]}, "because": "Core TH-2"}
    ]));
    let harness = Harness::new(false, true);
    let wake = harness.wake.clone();
    assert_eq!(run(&t, harness), (Outcome::Passed, vec![]));
    let owner = std::thread::current().id();
    let waiters = wake.lock().unwrap().waiters.clone();
    assert_eq!(waiters.len(), 2, "{waiters:?}");
    assert_ne!(waiters[0], owner, "other_thread waits on another thread");
    assert_eq!(waiters[1], owner, "a plain wait waits on the owner thread");
}
