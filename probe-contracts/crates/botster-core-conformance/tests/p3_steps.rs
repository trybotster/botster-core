//! The steps that core-p3 added to the Core driver, one at a time against FakeCore and a wrapper of it: `collect_events`,
//! `forbid_events`, `tap_drain`, a typed `expect_refused`, the statement steps (the harness runs them, and the driver checks the
//! report), the `let` and `check` operations, and `pump_collect` with `ignore`. A step that did nothing would pass every transcript
//! that used it, so these tests carry the mutation job (BUILD.md, Testing).
//! Clause: Core A5-1 (`collect_events`, `run_deterministic`), A5-2 and E2-3 (`forbid_events`, `ignore`), A5-3 (`expect_refused`,
//! `error_codes_reachable`), A5-4 (`run_suite`), TP-1 (`tap_drain`), ST-3 and ST-7 (the relations).

use botster_conformance::{Bindings, Deadline, StepDriver};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    normalize_instances, run_script_events, ControlError, CoreDriver, CoreHarness, DataDirRef,
    OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The core of the wrapper: FakeCore, with the tap chunks that a test queued.
struct TapCore {
    inner: Box<dyn CoreApi>,
    chunks: VecDeque<TapChunk>,
    /// An event that the first batch with a completion ends with: a wait that returns after its wanted event leaves it buffered.
    extra: Option<Event>,
    /// How the tap hands over its bytes: the chunks come only after `gate` pumps, and the pump reports why it ran.
    gate: usize,
    pumps: usize,
    mode: Pace,
    wake: Wake3,
}

#[derive(Clone, Copy, PartialEq)]
enum Wake3 {
    Real,
    Always,
    Never,
}

/// What a pump reports while the tap's bytes are still on their way (Core TP-1: the control link has flow control).
#[derive(Clone, Copy, PartialEq)]
enum Pace {
    /// FakeCore's own report.
    Real,
    /// More work is runnable, and nothing was posted.
    More,
    /// Nothing more is runnable, and an event was posted.
    Posted,
}

/// A wake handle that is never woken: a wait on it times out at once, and Core is idle.
struct NeverWoken;

impl WakeHandle for NeverWoken {
    fn wait(&self, _timeout: Duration) -> Wake {
        Wake::TimedOut
    }
    fn fd(&self) -> std::os::fd::RawFd {
        -1
    }
}

/// A wake handle that is always woken: a wait on it never idles.
struct AlwaysWoken;

impl WakeHandle for AlwaysWoken {
    fn wait(&self, _timeout: Duration) -> Wake {
        Wake::Woken
    }
    fn fd(&self) -> std::os::fd::RawFd {
        -1
    }
}

impl CoreApi for TapCore {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.inner.begin(op)
    }
    fn pump(&mut self, now: Now) -> PumpReport {
        self.pumps += 1;
        let mut report = self.inner.pump(now);
        let pending = self.pumps <= self.gate;
        match self.mode {
            Pace::Real => {}
            Pace::More => {
                report.more = pending;
                report.events_posted = 0;
            }
            Pace::Posted => {
                report.more = false;
                report.events_posted = u32::from(pending);
            }
        }
        report
    }
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let mut events = self.inner.poll_events(max);
        // The extra event follows a completion in its batch, so a wait that wants that completion leaves the extra one buffered.
        if events.iter().any(|e| matches!(e, Event::Completed { .. })) {
            events.extend(self.extra.take());
        }
        events
    }
    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        match self.wake {
            Wake3::Always => Arc::new(AlwaysWoken),
            Wake3::Never => Arc::new(NeverWoken),
            Wake3::Real => self.inner.wake_handle(),
        }
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
        // Core TP-1: each read returns at most one chunk of at most `max` bytes, as the contract allows: the number of reads is the
        // host's to count, never the contract's.
        // Until `gate` pumps have run, the bytes are still in flight: a read finds nothing.
        match (self.pumps >= self.gate)
            .then(|| self.chunks.pop_front())
            .flatten()
        {
            Some(c) => Ok(c),
            None => self.inner.tap_read(session, max),
        }
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

/// A harness over FakeCore. It queues tap chunks, refuses the control `refusable` with a typed value, and answers a statement with a
/// report that the test gives (and records the statement that it was asked).
struct Wrap {
    inner: FakeCoreHarness,
    chunks: Vec<TapChunk>,
    extra: Option<Event>,
    gate: usize,
    mode: Pace,
    wake: Wake3,
    refusal: Option<Value>,
    report: Option<Value>,
    asked: Arc<Mutex<Vec<(String, Value)>>>,
}

impl Wrap {
    fn new() -> Self {
        Wrap {
            inner: FakeCoreHarness::new(0),
            chunks: vec![],
            extra: None,
            gate: 0,
            mode: Pace::Real,
            wake: Wake3::Real,
            refusal: None,
            report: None,
            asked: Arc::default(),
        }
    }
}

impl CoreHarness for Wrap {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        Ok(Box::new(TapCore {
            inner: self.inner.open(spec)?,
            chunks: self.chunks.iter().cloned().collect(),
            extra: self.extra.clone(),
            gate: self.gate,
            pumps: 0,
            mode: self.mode,
            wake: self.wake,
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
        op == "refusable" || self.inner.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        if op == "refusable" {
            return match (&self.refusal, args.get("mode").and_then(Value::as_str)) {
                (Some(v), Some("typed")) => Err(ControlError::Refused(v.clone())),
                (_, Some("bad")) => Err(ControlError::Bad("the arguments are wrong".into())),
                _ => Ok(json!({ "scripted": true })),
            };
        }
        self.inner.control(handle, op, args)
    }
    fn statement(&mut self, kind: &str, spec: &Value) -> Result<Value, ControlError> {
        self.asked
            .lock()
            .unwrap()
            .push((kind.to_string(), spec.clone()));
        self.report.clone().ok_or(ControlError::Unsupported)
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

struct Run {
    driver: CoreDriver,
    bindings: Bindings,
}

impl Run {
    fn with(harness: Wrap) -> Self {
        let mut run = Run {
            driver: CoreDriver::new(Box::new(harness)),
            bindings: Bindings::new(),
        };
        run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
        run
    }
    fn new() -> Self {
        Run::with(Wrap::new())
    }
    fn exec(&mut self, step: Value) -> Result<(), String> {
        self.driver
            .exec(&step, &mut self.bindings, &Deadline::none())
            .map_err(|e| format!("{e:?}"))
    }
    /// Runs a step with a short deadline: a step that would wait for ever ends as `inconclusive`, so a failure that must be quick is
    /// told from a spin (a mutant that spins ends at the deadline, and its message differs).
    fn exec_within(&mut self, step: Value) -> Result<(), String> {
        self.driver
            .exec(
                &step,
                &mut self.bindings,
                &Deadline::after(Some(Duration::from_millis(300))),
            )
            .map_err(|e| format!("{e:?}"))
    }
    fn ok(&mut self, step: Value) {
        self.exec(step.clone())
            .unwrap_or_else(|e| panic!("{step}: {e}"));
    }
    fn err(&mut self, step: Value) -> String {
        self.exec(step.clone())
            .expect_err(&format!("{step} should fail"))
    }
}

fn create(session: &str) -> Value {
    json!({"begin": {"op": {"Create": {"session": session, "request": {"program": [{"hold": {}}]}}}}, "bind": format!("$c_{session}")})
}

fn chunk(bytes: &str, dropped: u64) -> TapChunk {
    chunk_of("i1", bytes, dropped)
}

fn chunk_of(instance: &str, bytes: &str, dropped: u64) -> TapChunk {
    TapChunk {
        instance: InstanceId(instance.into()),
        bytes: serde_json::from_value(json!({"$bytes_hex": bytes})).unwrap(),
        dropped_before: dropped,
    }
}

// ---- collect_events ----------------------------------------------------------------------------------------------------

#[test]
fn collect_events_binds_the_polled_events_and_leaves_none_queued() {
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(create("s2"));
    run.ok(json!({"collect_events": {}, "bind": "$e"}));
    let events = run
        .bindings
        .get("e")
        .and_then(Value::as_array)
        .expect("a list")
        .clone();
    // Both creations completed (OR-3 leaves their order open): the bound list holds both completions.
    let completed: Vec<&Value> = events
        .iter()
        .filter(|e| e.get("Completed").is_some())
        .collect();
    assert_eq!(completed.len(), 2, "{events:?}");
    let mut ops: Vec<Value> = completed
        .iter()
        .map(|e| e["Completed"]["op"].clone())
        .collect();
    ops.sort_by_key(|v| v.as_u64());
    assert_eq!(
        ops,
        vec![run.bindings["c_s1"].clone(), run.bindings["c_s2"].clone()]
    );
    // The events were consumed by the collection: a second collection and a poll find nothing.
    run.ok(json!({"collect_events": {}, "bind": "$again"}));
    assert_eq!(run.bindings["again"], json!([]));
    run.ok(json!({"poll_events": {"max": 64}, "expect": []}));
}

#[test]
fn collect_events_names_its_handle() {
    let mut run = Run::new();
    run.ok(json!({"open": {"as": "b", "data_dir": "d2"}}));
    run.ok(json!({"begin": {"handle": "b", "op": {"Create": {"session": "x", "request": {"program": [{"hold": {}}]}}}}, "bind": "$c"}));
    run.ok(json!({"collect_events": {}, "bind": "$e_h"}));
    assert_eq!(run.bindings["e_h"], json!([]));
    run.ok(json!({"collect_events": {"handle": "b"}, "bind": "$e_b"}));
    assert!(!run.bindings["e_b"].as_array().expect("a list").is_empty());
}

// ---- forbid_events -----------------------------------------------------------------------------------------------------

#[test]
fn a_forbidden_event_fails_the_wait_that_polls_it_and_clear_ends_it() {
    // The queue hands out a seed-chosen part of its events, so each wait polls until it has the completion that it waits for.
    for poll in [
        json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}),
        json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1"}}]}}),
        json!({"collect_events": {}}),
        json!({"drain": {}}),
        json!({"poll_all": {}}),
    ] {
        let mut run = Run::new();
        run.ok(create("s1"));
        run.ok(json!({"forbid_events": {"events": [{"Completed": {"op": "$c_s1"}}]}}));
        // `poll_all` does not pump: the events are posted first.
        run.ok(json!({"pump_idle": {}}));
        let why = run.err(poll.clone());
        assert!(why.contains("forbidden"), "{poll}: {why}");
    }
    // `poll_events` polls a part of the queue each time: some poll finds the forbidden event.
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"forbid_events": {"events": [{"Completed": {"op": "$c_s1"}}]}}));
    run.ok(json!({"pump_idle": {}}));
    let mut found = false;
    for _ in 0..20 {
        if let Err(why) = run.exec(json!({"poll_events": {"max": 64}, "expect": [{"$any": true}]}))
        {
            if why.contains("forbidden") {
                found = true;
                break;
            }
        }
    }
    assert!(found, "no poll found the forbidden event");
    // `clear` ends it, and an event that another matcher names is not forbidden.
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"forbid_events": {"events": [{"Completed": {"op": "$c_s1"}}]}}));
    run.ok(json!({"forbid_events": {"clear": true}}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"forbid_events": {"events": [{"Bell": {"id": "s1"}}]}}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
}

#[test]
fn a_forbidden_event_of_another_handle_is_not_forbidden_here() {
    let mut run = Run::new();
    run.ok(json!({"open": {"as": "b", "data_dir": "d2"}}));
    run.ok(create("s1"));
    run.ok(json!({"forbid_events": {"handle": "b", "events": [{"$any": true}]}}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    let why = run.err(json!({"forbid_events": {"events": "not a list"}}));
    assert!(why.contains("list"), "{why}");
}

// ---- pump_collect ignore -----------------------------------------------------------------------------------------------

#[test]
fn an_exhaustive_collect_lets_an_ignored_event_pass_and_fails_on_any_other() {
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(create("s2"));
    // Two completions, nothing else wanted: an exhaustive collect that ignores nothing fails on the second completion's rival.
    let why = run.err(
        json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1"}}], "exhaustive": true}}),
    );
    assert!(why.contains("unexpected"), "{why}");
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(create("s2"));
    run.ok(
        json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1"}}], "exhaustive": true,
                  "ignore": [{"Completed": {"op": "$c_s2"}}, {"SessionState": {"$any": true}}]}}),
    );
}

// ---- expect_refused ----------------------------------------------------------------------------------------------------

#[test]
fn expect_refused_matches_the_typed_refusal_and_nothing_else() {
    let mut harness = Wrap::new();
    harness.refusal =
        Some(json!({"call": "Start", "code": "StartFailed", "reason": "not_in_sync_column"}));
    let mut run = Run::with(harness);
    run.ok(json!({"control": {"op": "refusable", "mode": "typed"}, "expect_refused": {"call": "Start", "code": "StartFailed"}}));
    // A refusal that names another code does not match.
    let why = run.err(json!({"control": {"op": "refusable", "mode": "typed"}, "expect_refused": {"code": "RegistryFailed"}}));
    assert!(why.contains("refused"), "{why}");
    // A control that the harness accepts fails the step; so do wrong arguments (not a typed refusal), and so does no refusal step
    // for a refused control.
    let why = run.err(json!({"control": {"op": "refusable", "mode": "none"}, "expect_refused": {"call": "Start"}}));
    assert!(why.contains("refused"), "{why}");
    let why = run.err(
        json!({"control": {"op": "refusable", "mode": "bad"}, "expect_refused": {"call": "Start"}}),
    );
    assert!(why.contains("typed refusal"), "{why}");
    let why = run.err(json!({"control": {"op": "refusable", "mode": "typed"}}));
    assert!(why.contains("refused"), "{why}");
    // A control that the harness does not have is `unsupported_control`.
    let why =
        run.err(json!({"control": {"op": "no_such_control"}, "expect_refused": {"call": "Start"}}));
    assert!(why.contains("Unsupported"), "{why}");
}

// ---- tap_drain and tap_read --------------------------------------------------------------------------------------------

#[test]
fn tap_read_calls_the_host_read_with_the_session_and_the_bound() {
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    run.ok(json!({"call": {"method": "tap_read", "args": {"session": "s1", "max": 16}},
                  "expect": {"bytes": {"$bytes_hex": ""}, "dropped_before": 0, "instance": {"$type": "string"}}}));
    run.err(json!({"call": {"method": "tap_read", "args": {"session": "s1"}}}));
    run.ok(
        json!({"call": {"method": "tap_read", "args": {"session": "nope", "max": 16}},
                  "expect_error": {"code": "UnknownSession"}}),
    );
}

#[test]
fn tap_drain_without_accounted_ends_at_the_first_empty_read() {
    let mut harness = Wrap::new();
    harness.chunks = vec![
        chunk_of("i1", "6162", 5),
        chunk_of("i2", "63", 0),
        chunk("", 0),
        chunk("6465", 2),
    ];
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    run.ok(json!({"tap_drain": {"session": "s1"}, "bind": "$t"}));
    // The bytes are in order, the drops add up, each chunk with bytes names its instance, and the empty read ends the drain.
    assert_eq!(run.bindings["t"]["bytes"], json!({"$bytes_hex": "616263"}));
    assert_eq!(run.bindings["t"]["dropped"], json!(5));
    assert_eq!(run.bindings["t"]["instances"], json!(["i1", "i2"]));
    // A tap that holds nothing: no bytes, no drops, no instances.
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    run.ok(json!({"tap_drain": {"session": "s1", "max": 4}, "bind": "$t"}));
    assert_eq!(
        run.bindings["t"],
        json!({"bytes": {"$bytes_hex": ""}, "dropped": 0, "instances": []})
    );
    // The session must exist.
    run.err(json!({"tap_drain": {"session": "nope"}}));
}

#[test]
fn tap_drain_with_accounted_goes_on_past_an_empty_read_until_every_byte_is_accounted_for() {
    let mut harness = Wrap::new();
    harness.chunks = vec![chunk("61", 0), chunk("", 0), chunk("6263", 3)];
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    // 3 bytes read plus 3 bytes dropped are 6: the empty read in the middle is no barrier.
    run.ok(json!({"tap_drain": {"session": "s1", "accounted": 6}, "bind": "$t"}));
    assert_eq!(run.bindings["t"]["bytes"], json!({"$bytes_hex": "616263"}));
    assert_eq!(run.bindings["t"]["dropped"], json!(3));
    // Bytes that never come are a failure, not a pass and not a hang.
    let mut harness = Wrap::new();
    harness.chunks = vec![chunk("61", 0)];
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    let why = run
        .exec_within(json!({"tap_drain": {"session": "s1", "accounted": 5}}))
        .expect_err("never comes");
    assert!(
        why.contains("accounted") && why.contains("nothing more"),
        "{why}"
    );
    // With the count met at once, the drain ends at the first empty read.
    let mut run = Run::new();
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    run.ok(json!({"tap_drain": {"session": "s1", "accounted": 0}, "bind": "$t"}));
    assert_eq!(run.bindings["t"]["bytes"], json!({"$bytes_hex": ""}));
}

// ---- statements --------------------------------------------------------------------------------------------------------

fn statement(report: Value, step: Value) -> Result<Vec<(String, Value)>, String> {
    let mut harness = Wrap::new();
    harness.report = Some(report);
    let asked = harness.asked.clone();
    let mut run = Run::with(harness);
    run.exec(step)?;
    let got = asked.lock().unwrap().clone();
    Ok(got)
}

#[test]
fn a_harness_without_the_capability_gives_unsupported_for_every_statement() {
    for (statement, body) in [
        (
            "run_suite",
            json!({"subject": "core", "harnesses": ["testkit"]}),
        ),
        (
            "check_crates",
            json!({"crate": "x", "not_in": ["y"], "no_dependency_of": ["z"]}),
        ),
        ("error_codes_reachable", json!({"no_scenario": []})),
        ("run_deterministic", json!({"runs": 2, "steps": []})),
    ] {
        let mut run = Run::new();
        let why = run.err(json!({ statement: body }));
        assert!(
            why.contains("Unsupported") && why.contains(statement),
            "{statement}: {why}"
        );
    }
}

#[test]
fn run_suite_needs_the_same_complete_passing_selection_on_every_harness() {
    // The Core suite selection: every transcript with the filter, except the statements themselves.
    // Only the files of the selection are parsed (the file name is the id): parsing every transcript made this test grow with the suite.
    let mut ids: Vec<String> = botster_core_conformance::CORE_TRANSCRIPTS
        .files()
        .filter(|f| {
            f.path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("conf::lc_"))
        })
        .map(|f| serde_json::from_slice::<serde_json::Value>(f.contents()).unwrap())
        .filter(|t| {
            !t["steps"]
                .as_array()
                .is_some_and(|steps| steps.iter().any(|s| s.get("run_suite").is_some()))
        })
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    assert!(ids.len() > 3, "the selection is a real suite");
    let step = json!({"run_suite": {"subject": "core", "harnesses": ["real_worker_processes", "testkit"], "filter": "conf::lc_", "under": ["seeds"]}});
    let run = |h: &str, selected: &[String], passed: &[String], failed: Value| json!({"harness": h, "selected": selected, "passed": passed, "failed": failed});
    let good = |a: Value, b: Value, under: Value| json!({"runs": [a, b], "under": under});
    let ok_a = run("testkit", &ids, &ids, json!([]));
    let ok_b = run("real_worker_processes", &ids, &ids, json!([]));
    let asked = statement(
        good(ok_a.clone(), ok_b.clone(), json!(["seeds", "shuttle"])),
        step.clone(),
    )
    .expect("a good report passes");
    assert_eq!(asked[0].0, "run_suite");
    assert_eq!(
        asked[0].1["harnesses"],
        json!(["real_worker_processes", "testkit"])
    );
    let fewer: Vec<String> = ids[1..].to_vec();
    let mut extra = ids.clone();
    extra.push("conf::lc_zz_not_a_transcript".into());
    for (what, bad) in [
        (
            "a harness is missing",
            json!({"runs": [ok_a.clone()], "under": ["seeds"]}),
        ),
        (
            "no ids selected",
            good(
                run("testkit", &[], &[], json!([])),
                run("real_worker_processes", &[], &[], json!([])),
                json!(["seeds"]),
            ),
        ),
        (
            "an omitted transcript on both",
            good(
                run("testkit", &fewer, &fewer, json!([])),
                run("real_worker_processes", &fewer, &fewer, json!([])),
                json!(["seeds"]),
            ),
        ),
        (
            "an id that is no transcript",
            good(
                run("testkit", &extra, &extra, json!([])),
                run("real_worker_processes", &extra, &extra, json!([])),
                json!(["seeds"]),
            ),
        ),
        (
            "a different set on one harness",
            good(
                ok_a.clone(),
                run("real_worker_processes", &fewer, &fewer, json!([])),
                json!(["seeds"]),
            ),
        ),
        (
            "one id did not pass",
            good(
                run("testkit", &ids, &fewer, json!([])),
                ok_b.clone(),
                json!(["seeds"]),
            ),
        ),
        (
            "a failure",
            good(
                run("testkit", &ids, &ids, json!([ids[0]])),
                ok_b.clone(),
                json!(["seeds"]),
            ),
        ),
        (
            "no failed list",
            good(
                json!({"harness": "testkit", "selected": ids, "passed": ids}),
                ok_b.clone(),
                json!(["seeds"]),
            ),
        ),
        (
            "no ids at all",
            json!({"runs": [{"harness": "testkit", "ran": 3, "failed": []}, {"harness": "real_worker_processes", "ran": 3, "failed": []}], "under": ["seeds"]}),
        ),
        (
            "a variation is missing",
            good(ok_a.clone(), ok_b.clone(), json!([])),
        ),
        ("no runs", json!({"under": ["seeds"]})),
    ] {
        statement(bad.clone(), step.clone()).expect_err(&format!("{what}: {bad} should fail"));
    }
    // Without `under`, the report needs none; another subject has no known ids, but it still needs one non-empty identical selection.
    let hub = json!({"run_suite": {"subject": "hub", "harnesses": ["testkit"]}});
    let hid = vec!["conf::h1".to_string()];
    statement(
        json!({"runs": [run("testkit", &hid, &hid, json!([]))]}),
        hub.clone(),
    )
    .expect("no variation asked");
    statement(
        json!({"runs": [run("testkit", &hid, &[], json!([]))]}),
        hub.clone(),
    )
    .expect_err("an id that did not pass");
    // The statement transcripts are not selected: a selection that includes this very kind of transcript fails.
    let with_statement: Vec<String> = botster_core_conformance::CORE_TRANSCRIPTS
        .files()
        .filter_map(|f| f.path().file_name().and_then(|n| n.to_str()))
        .filter(|n| n.starts_with("conf::a5_4_"))
        .map(|n| n.trim_end_matches(".json").to_string())
        .collect();
    let step_a54 = json!({"run_suite": {"subject": "core", "harnesses": ["testkit"], "filter": "conf::a5_4_"}});
    statement(
        json!({"runs": [run("testkit", &with_statement, &with_statement, json!([]))]}),
        step_a54,
    )
    .expect_err("a statement transcript is not part of a selection");
}

#[test]
fn check_crates_needs_both_lists_to_be_empty() {
    let step = json!({"check_crates": {"crate": "botster-core-testkit", "not_in": ["f"], "no_dependency_of": ["ffi"]}});
    statement(json!({"found_in": [], "depended_on_by": []}), step.clone())
        .expect("empty lists pass");
    for bad in [
        json!({"found_in": ["botster_core::prelude"], "depended_on_by": []}),
        json!({"found_in": [], "depended_on_by": ["botster-core-ffi"]}),
        json!({"found_in": []}),
    ] {
        statement(bad.clone(), step.clone()).expect_err(&format!("{bad} should fail"));
    }
}

#[test]
fn error_codes_reachable_needs_every_code_reached_and_the_no_scenario_list_as_stated() {
    let step =
        json!({"error_codes_reachable": {"no_scenario": [{"code": "Internal", "because": "x"}]}});
    statement(
        json!({"not_reached": [], "no_scenario": [{"code": "Internal", "because": "x"}]}),
        step.clone(),
    )
    .expect("passes");
    for bad in [
        json!({"not_reached": ["PendingLimit"], "no_scenario": [{"code": "Internal", "because": "x"}]}),
        json!({"not_reached": [], "no_scenario": []}),
        json!({"no_scenario": [{"code": "Internal", "because": "x"}]}),
    ] {
        statement(bad.clone(), step.clone()).expect_err(&format!("{bad} should fail"));
    }
}

#[test]
fn run_deterministic_compares_fresh_runs_by_the_first_appearance_of_each_instance() {
    let step = json!({"run_deterministic": {"runs": 2, "steps": []}});
    let run = |inst_a: &str, inst_b: &str, extra: Value| {
        json!([
            {"SessionState": {"id": "s1", "instance": inst_a, "state": "Created"}},
            {"SessionState": {"id": "s2", "instance": inst_b, "state": "Created"}},
            {"Completed": {"op": 1, "result": extra}},
        ])
    };
    // Different instance names with the same pattern are the same events.
    statement(
        json!({"runs": [run("a1", "a2", json!({})), run("b7", "b9", json!({}))]}),
        step.clone(),
    )
    .expect("same events");
    for bad in [
        // another event
        json!({"runs": [run("a1", "a2", json!({})), run("b7", "b9", json!({"err": 1}))]}),
        // an instance that two sessions share
        json!({"runs": [run("a1", "a1", json!({})), run("b7", "b7", json!({}))]}),
        // another number of runs, and fewer than two
        json!({"runs": [run("a1", "a2", json!({}))]}),
        json!({"runs": [run("a1", "a2", json!({})), run("b7", "b9", json!({})), run("c1", "c2", json!({}))]}),
        // an instance pattern that differs: one run reuses the first instance for the second session's event
        json!({"runs": [run("a1", "a2", json!({})), json!([
            {"SessionState": {"id": "s1", "instance": "b1", "state": "Created"}},
            {"SessionState": {"id": "s2", "instance": "b1", "state": "Created"}},
            {"Completed": {"op": 1, "result": {}}}])]}),
    ] {
        statement(bad.clone(), step.clone()).expect_err(&format!("{bad} should fail"));
    }
    // One run is not a comparison.
    statement(
        json!({"runs": [run("a1", "a2", json!({}))]}),
        json!({"run_deterministic": {"runs": 1, "steps": []}}),
    )
    .expect_err("a single run proves nothing");
}

#[test]
fn run_script_events_runs_the_script_on_a_fresh_harness_and_returns_its_events() {
    let script = vec![
        json!({"open": {"as": "h", "data_dir": "d"}}),
        create("s1"),
        create("s2"),
        json!({"pump_idle": {}}),
    ];
    let a = run_script_events(Box::new(FakeCoreHarness::new(3)), &script).expect("a run");
    let b = run_script_events(Box::new(FakeCoreHarness::new(3)), &script).expect("a run");
    let events = a.as_array().expect("a list");
    assert_eq!(
        events
            .iter()
            .filter(|e| e.get("Completed").is_some())
            .count(),
        2,
        "{events:?}"
    );
    // The same seed and script give the same events (FakeCore's instance names are its own).
    assert_eq!(a, b);
    // A failing step is the failure of the run.
    run_script_events(Box::new(FakeCoreHarness::new(3)), &[json!({"pump": {}})])
        .expect_err("no handle is open");
}

// ---- let and check -----------------------------------------------------------------------------------------------------

#[test]
fn the_let_operations_of_core_p3_compute_what_their_clauses_say() {
    let mut run = Run::new();
    run.ok(json!({"let": {"$rt": {"rtrim_spaces": ["a b  "]}, "$n": {"utf8_len": ["\u{e9}a"]}}}));
    assert_eq!(run.bindings["rt"], json!("a b"));
    assert_eq!(run.bindings["n"], json!(3));
    run.ok(json!({"let": {"$gt": {"gt": [5, 3]}, "$lt": {"lt": [5, 3]}, "$gte": {"gt": [3, 3]}}}));
    assert_eq!(
        (
            run.bindings["gt"].clone(),
            run.bindings["lt"].clone(),
            run.bindings["gte"].clone()
        ),
        (json!(true), json!(false), json!(false))
    );
    run.ok(json!({"let": {"$p": {"record_payloads": [[
        {"record": {"payload": {"$bytes_hex": "6162"}}},
        {"status_update": {"updates": 1}},
        {"record": {"kind": "focus"}},
        {"record": {"payload": {"$bytes_hex": "63"}}}]]}}}));
    assert_eq!(run.bindings["p"], json!(["6162", "", "63"]));
    run.ok(json!({"let": {"$s": {"sum_hex_bytes": [["6162", "", "63"]]}, "$j": {"hex_concat": [{"$bytes_hex": "61"}, "6263"]}}}));
    assert_eq!(run.bindings["s"], json!(3));
    assert_eq!(run.bindings["j"], json!({"$bytes_hex": "616263"}));
    // `without` drops one field, present or absent (Core A18-1).
    run.ok(json!({"let": {"$w": {"without": [{"a": 1, "b": 2}, "a"]}, "$v": {"without": [{"b": 2}, "a"]}}}));
    assert_eq!(run.bindings["w"], json!({"b": 2}));
    assert_eq!(run.bindings["v"], json!({"b": 2}));
    // The operations check their arguments.
    for bad in [
        json!({"let": {"$x": {"rtrim_spaces": [1]}}}),
        json!({"let": {"$x": {"utf8_len": [1]}}}),
        json!({"let": {"$x": {"gt": ["a", 1]}}}),
        json!({"let": {"$x": {"record_payloads": ["no"]}}}),
        json!({"let": {"$x": {"sum_hex_bytes": ["no"]}}}),
        json!({"let": {"$x": {"hex_concat": [1, "6162"]}}}),
        json!({"let": {"$x": {"rtrim_spaces": ["a", "b"]}}}),
        json!({"let": {"$x": {"add": [1]}}}),
        json!({"let": {"$x": {"without": [[1], "a"]}}}),
        json!({"let": {"$x": {"without": [{"a": 1}, 1]}}}),
    ] {
        run.err(bad);
    }
}

#[test]
fn the_check_relations_of_core_p3_hold_and_fail_as_they_say() {
    let mut run = Run::new();
    for good in [
        json!({"check": {"prefix_of": ["ab", "abc"]}}),
        json!({"check": {"prefix_of": [[1, 2], [1, 2, 3]]}}),
        json!({"check": {"suffix_of": ["bc", "abc"]}}),
        json!({"check": {"suffix_of": [[2, 3], [1, 2, 3]]}}),
        json!({"check": {"le": [3, 3]}}),
        json!({"check": {"le": [2, 3]}}),
    ] {
        run.ok(good);
    }
    for bad in [
        json!({"check": {"prefix_of": ["bc", "abc"]}}),
        json!({"check": {"prefix_of": [[2], [1, 2]]}}),
        json!({"check": {"prefix_of": [1, 2]}}),
        json!({"check": {"suffix_of": ["ab", "abc"]}}),
        json!({"check": {"suffix_of": [[1], [1, 2]]}}),
        json!({"check": {"suffix_of": [1, 2]}}),
        json!({"check": {"le": [4, 3]}}),
    ] {
        run.err(bad);
    }
}

#[test]
fn instances_are_numbered_by_their_first_appearance_and_one_instance_names_one_session() {
    let run = json!([
        {"SessionState": {"id": "s1", "instance": "a1", "state": "Created"}},
        {"SessionState": {"id": "s2", "instance": "a2", "state": "Created"}},
        {"SessionState": {"id": "s1", "instance": "a1", "state": "Running"}},
        {"Activity": {"instance": "a2"}},
        {"Completed": {"op": 1, "result": {}}},
        {"Nested": {"inner": {"id": "s3", "instance": "a3"}}},
    ]);
    // The numbers are the first appearances, a repeat of an instance keeps its number, and an event with no `id` may name an instance.
    let want = json!([
        {"SessionState": {"id": "s1", "instance": "instance#0", "state": "Created"}},
        {"SessionState": {"id": "s2", "instance": "instance#1", "state": "Created"}},
        {"SessionState": {"id": "s1", "instance": "instance#0", "state": "Running"}},
        {"Activity": {"instance": "instance#1"}},
        {"Completed": {"op": 1, "result": {}}},
        {"Nested": {"inner": {"id": "s3", "instance": "instance#2"}}},
    ]);
    assert_eq!(normalize_instances(&run).expect("a run"), want);
    // An instance that two sessions share is not unique (Core ID-2, EV-9). A repeat that has no `id` is not a second session.
    let shared = json!([
        {"SessionState": {"id": "s1", "instance": "a1"}},
        {"SessionState": {"id": "s2", "instance": "a1"}},
    ]);
    normalize_instances(&shared).expect_err("two sessions share an instance");
    let anonymous = json!([
        {"SessionState": {"id": "s1", "instance": "a1"}},
        {"Activity": {"instance": "a1"}},
        {"Activity": {"id": "s1", "instance": "a1"}},
    ]);
    normalize_instances(&anonymous).expect("one session, repeated");
    let first_anonymous = json!([
        {"Activity": {"instance": "a1"}},
        {"SessionState": {"id": "s1", "instance": "a1"}},
        {"SessionState": {"id": "s1", "instance": "a1"}},
    ]);
    normalize_instances(&first_anonymous).expect("an anonymous first appearance names no session");
}

#[test]
fn a_harness_that_does_not_override_statement_is_unsupported() {
    // FakeCoreHarness keeps the default `statement`: no capability is never a pass.
    let mut driver = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut bindings = Bindings::new();
    let step = json!({"run_suite": {"subject": "core", "harnesses": ["testkit"]}});
    let why = format!(
        "{:?}",
        driver
            .exec(&step, &mut bindings, &Deadline::none())
            .expect_err("unsupported")
    );
    assert!(why.contains("Unsupported"), "{why}");
}

#[test]
fn an_event_that_a_wait_left_in_the_buffer_is_checked_when_the_observer_starts_and_is_collected() {
    // The first poll hands out the completion and one more event. `pump_collect` returns after the completion, and the other event
    // stays in the driver's buffer.
    let extra = Event::RouteStalled { route: RouteId(9) };
    let build = || {
        let mut harness = Wrap::new();
        harness.extra = Some(extra.clone());
        let mut run = Run::with(harness);
        run.ok(create("s1"));
        run.ok(json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1"}}]}}));
        run
    };
    // An observer that starts now still sees it: the step fails, and `drain` cannot hide it first.
    let mut run = build();
    let why = run.err(json!({"forbid_events": {"events": [{"RouteStalled": {"route": 9}}]}}));
    assert!(why.contains("forbidden"), "{why}");
    // An observer for another event passes, and the buffered event is then collected, not lost.
    let mut run = build();
    run.ok(json!({"forbid_events": {"events": [{"Bell": {"id": "s1"}}]}}));
    run.ok(json!({"collect_events": {}, "bind": "$e"}));
    let events = run.bindings["e"].as_array().expect("a list");
    assert!(
        events.iter().any(|e| e.get("RouteStalled").is_some()),
        "{events:?}"
    );
}

#[test]
fn tap_drain_accounts_with_the_bytes_read_plus_the_bytes_dropped_and_no_more() {
    let drain = |chunks: Vec<TapChunk>, accounted: u64| {
        let mut harness = Wrap::new();
        harness.chunks = chunks;
        let mut run = Run::with(harness);
        run.ok(create("s1"));
        run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
        let r = run.exec_within(
            json!({"tap_drain": {"session": "s1", "accounted": accounted}, "bind": "$t"}),
        );
        (r, run.bindings.get("t").cloned())
    };
    // Dropped bytes alone account for a window: `5 dropped + 0 read = 5`.
    let (r, t) = drain(vec![chunk("", 5)], 5);
    r.expect("the dropped bytes are counted");
    assert_eq!(t.unwrap()["dropped"], json!(5));
    // Two bytes read account for two, not for four (the hex digits) and not for more.
    let why3 = drain(vec![chunk("6162", 0)], 3)
        .0
        .expect_err("2 bytes do not account for 3");
    assert!(why3.contains("nothing more"), "{why3}");
    drain(vec![chunk("6162", 0)], 2)
        .0
        .expect("2 bytes account for 2");
    // Read bytes and dropped bytes add: `2 + 1 = 3`.
    drain(vec![chunk("6162", 1)], 3)
        .0
        .expect("2 read + 1 dropped");
    let why4 = drain(vec![chunk("6162", 1)], 4)
        .0
        .expect_err("2 read + 1 dropped is not 4");
    assert!(why4.contains("nothing more"), "{why4}");
}

#[test]
fn tap_drain_goes_on_while_the_pump_runs_work_and_retries_one_stall() {
    // The bytes come after three pumps. A pump that reports runnable work, or a posted event, is progress and not a stall.
    for mode in [Pace::More, Pace::Posted] {
        for wake in [Wake3::Real, Wake3::Always, Wake3::Never] {
            let mut harness = Wrap::new();
            harness.chunks = vec![chunk("61", 0)];
            harness.gate = 3;
            harness.mode = mode;
            harness.wake = wake;
            let mut run = Run::with(harness);
            run.ok(create("s1"));
            run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
            run.ok(json!({"tap_drain": {"session": "s1", "accounted": 1}, "bind": "$t"}));
            assert_eq!(run.bindings["t"]["bytes"], json!({"$bytes_hex": "61"}));
        }
    }
    // A wait that is always woken is progress too: the bytes come after the gate even when no pump reports work.
    let mut harness = Wrap::new();
    harness.chunks = vec![chunk("61", 0)];
    harness.gate = 3;
    harness.wake = Wake3::Always;
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1"}}}}));
    run.ok(json!({"tap_drain": {"session": "s1", "accounted": 1}, "bind": "$t"}));
    // One stall with Core idle is retried, and a read after it may find the bytes; two in a row are a failure.
    let mut harness = Wrap::new();
    harness.chunks = vec![chunk("61", 0), chunk("", 0), chunk("62", 0)];
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"collect_events": {}}));
    run.ok(json!({"wait_wake": {"timeout_ms": 0}, "expect": {"$one_of": ["Woken", "TimedOut"]}}));
    run.ok(json!({"wait_wake": {"timeout_ms": 0}, "expect": {"$one_of": ["Woken", "TimedOut"]}}));
    run.ok(json!({"tap_drain": {"session": "s1", "accounted": 2}, "bind": "$t"}));
    assert_eq!(run.bindings["t"]["bytes"], json!({"$bytes_hex": "6162"}));
}

#[test]
fn tap_drain_that_runs_out_of_time_is_inconclusive_and_not_failed() {
    let mut harness = Wrap::new();
    harness.chunks = vec![chunk("61", 0)];
    let mut run = Run::with(harness);
    run.ok(create("s1"));
    run.ok(json!({"collect_events": {}}));
    let step = json!({"tap_drain": {"session": "s1", "accounted": 5}});
    let why = format!(
        "{:?}",
        run.driver
            .exec(
                &step,
                &mut run.bindings,
                &Deadline::after(Some(Duration::ZERO))
            )
            .expect_err("no time")
    );
    assert!(why.contains("Inconclusive"), "{why}");
}
