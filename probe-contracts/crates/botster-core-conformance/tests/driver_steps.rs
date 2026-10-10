//! The steps of the Core driver, one at a time, against FakeCore and a counting wrapper of it (design 5.5, 6.2): each relation, binding and
//! wait does what its documentation says. A step that did nothing would pass every transcript that used it, so these tests carry the
//! mutation job (BUILD.md, Testing).

use botster_conformance::{Bindings, Deadline, StepDriver};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    ControlError, CoreDriver, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild,
    WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// FakeCore whose `pump` reports runnable work for its first three calls and delivers one synthetic event at the fourth poll.
struct Counting {
    inner: Box<dyn CoreApi>,
    pumps: Arc<AtomicUsize>,
    waits: Arc<AtomicUsize>,
    synthetic_sent: bool,
    /// The wake handle blocks for its whole timeout, as a real one does when nothing wakes it.
    blocking: bool,
    /// A poll returns at most this many events (0: no bound), as a partial poll batch does (Core A5-2).
    poll_batch: usize,
}

/// The wake handle of the wrapper counts the waits that the fences make on it.
struct CountingWake {
    inner: Arc<dyn WakeHandle>,
    waits: Arc<AtomicUsize>,
    blocking: bool,
}

impl WakeHandle for CountingWake {
    fn wait(&self, timeout: Duration) -> Wake {
        self.waits.fetch_add(1, Ordering::SeqCst);
        if self.blocking {
            // timer: deadline — a stand-in for a wake handle that blocks until its timeout
            std::thread::sleep(timeout);
            return Wake::TimedOut;
        }
        self.inner.wait(timeout)
    }
    fn fd(&self) -> std::os::fd::RawFd {
        self.inner.fd()
    }
}

impl CoreApi for Counting {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.inner.begin(op)
    }
    fn pump(&mut self, now: Now) -> PumpReport {
        let n = self.pumps.fetch_add(1, Ordering::SeqCst) + 1;
        let mut report = self.inner.pump(now);
        report.more = n <= 3;
        report.events_posted = 0;
        report
    }
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let max = if self.poll_batch == 0 {
            max
        } else {
            max.min(self.poll_batch)
        };
        let mut events = self.inner.poll_events(max);
        if self.pumps.load(Ordering::SeqCst) == 4 && !self.synthetic_sent {
            self.synthetic_sent = true;
            events.push(Event::RouteStalled { route: RouteId(9) });
        }
        events
    }
    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        Arc::new(CountingWake {
            inner: self.inner.wake_handle(),
            waits: self.waits.clone(),
            blocking: self.blocking,
        })
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

/// A route end that records which attach it belongs to when a control reaches it.
struct Tagged {
    n: usize,
    inner: Box<dyn RouteClient>,
    log: Arc<Mutex<Vec<(usize, String)>>>,
}

impl RouteClient for Tagged {
    fn write(&mut self, bytes: &[u8]) {
        self.inner.write(bytes)
    }
    fn read(
        &mut self,
        max: usize,
        deadline: &Deadline,
    ) -> botster_hub_conformance::route::RouteRead {
        self.inner.read(max, deadline)
    }
    fn control(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        self.log.lock().unwrap().push((self.n, op.to_string()));
        self.inner.control(op, args)
    }
    fn has_control(&self, op: &str) -> bool {
        self.inner.has_control(op)
    }
}

/// A harness: not a fake, with an injected clock, a worker that it offers under a file name, and optionally counting pumps.
struct Harness {
    /// `edges_quiet` answers `quiet` from this call on (0: never).
    quiet_from: usize,
    quiet_calls: Arc<AtomicUsize>,
    inner: FakeCoreHarness,
    pumps: Option<Arc<AtomicUsize>>,
    waits: Arc<AtomicUsize>,
    opened_with: Arc<Mutex<Vec<Option<String>>>>,
    route_controls: Arc<Mutex<Vec<(usize, String)>>>,
    attached: usize,
    attach_options: Arc<Mutex<Vec<AttachOptions>>>,
    real_clock: bool,
    /// `progress_is_injected`: false for a harness with an injected clock and real progress (R-46).
    progress_injected: bool,
    poll_batch: usize,
    /// `pty_input` answers one byte once the pump count is above this number, and nothing before (a worker PTY write that posts no host
    /// event). `None`: the harness's own answer.
    pty_after_pumps: Option<Arc<AtomicUsize>>,
}

impl Harness {
    fn new(pumps: Option<Arc<AtomicUsize>>) -> Self {
        Harness {
            quiet_from: 3,
            quiet_calls: Arc::default(),
            inner: FakeCoreHarness::new(0),
            pumps,
            waits: Arc::default(),
            opened_with: Arc::default(),
            route_controls: Arc::default(),
            attached: 0,
            attach_options: Arc::default(),
            real_clock: false,
            progress_injected: true,
            poll_batch: 0,
            pty_after_pumps: None,
        }
    }
}

impl CoreHarness for Harness {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        self.opened_with
            .lock()
            .unwrap()
            .push(spec.worker.as_ref().and_then(|w| w.file_name.clone()));
        let core = self.inner.open(spec)?;
        Ok(match &self.pumps {
            Some(pumps) => Box::new(Counting {
                inner: core,
                pumps: pumps.clone(),
                waits: self.waits.clone(),
                synthetic_sent: false,
                blocking: self.real_clock,
                poll_batch: self.poll_batch,
            }),
            None => core,
        })
    }
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        self.inner.data_dir(name)
    }
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        self.inner.worker(which)
    }
    fn worker_named(&self, file_name: &str) -> Option<WorkerRef> {
        Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: Some(file_name.to_string()),
        })
    }
    fn drop_handle(&mut self, handle: &str) {
        self.inner.drop_handle(handle)
    }
    fn injects_clock(&self) -> bool {
        !self.real_clock
    }
    fn progress_is_injected(&self) -> bool {
        self.progress_injected
    }
    fn has_control(&self, op: &str) -> bool {
        op == "edges_quiet" || self.inner.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        if op == "edges_quiet" {
            let n = self.quiet_calls.fetch_add(1, Ordering::SeqCst) + 1;
            return Ok(json!({ "quiet": self.quiet_from != 0 && n >= self.quiet_from }));
        }
        if let (true, Some(after), Some(pumps)) =
            (op == "pty_input", &self.pty_after_pumps, &self.pumps)
        {
            let pumped = pumps.load(Ordering::SeqCst) > after.load(Ordering::SeqCst);
            return Ok(json!({ "bytes": { "$bytes_hex": if pumped { "61" } else { "" } } }));
        }
        self.inner.control(handle, op, args)
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
        self.attach_options.lock().unwrap().push(options.clone());
        let (result, end) = self
            .inner
            .attach_stream(handle, core, client, session, options)?;
        self.attached += 1;
        let tagged = Tagged {
            n: self.attached,
            inner: end,
            log: self.route_controls.clone(),
        };
        Ok((result, Box::new(tagged)))
    }
}

struct Run {
    driver: CoreDriver,
    bindings: Bindings,
}

impl Run {
    fn new(harness: Harness) -> Self {
        let mut run = Run {
            driver: CoreDriver::new(Box::new(harness)),
            bindings: Bindings::new(),
        };
        run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
        run
    }
    fn exec(&mut self, step: Value) -> Result<(), String> {
        self.driver
            .exec(&step, &mut self.bindings, &Deadline::none())
            .map_err(|e| format!("{e:?}"))
    }
    fn exec_within(&mut self, step: Value, deadline: &Deadline) -> Result<(), String> {
        self.driver
            .exec(&step, &mut self.bindings, deadline)
            .map_err(|e| format!("{e:?}"))
    }
    fn ok(&mut self, step: Value) {
        self.exec(step.clone())
            .unwrap_or_else(|e| panic!("{step}: {e}"));
    }
    fn err(&mut self, step: Value) {
        assert!(self.exec(step.clone()).is_err(), "{step} should fail");
    }
}

fn create(session: &str) -> Value {
    json!({"begin": {"op": {"Create": {"session": session, "request": {"program": [{"hold": {}}]}}}}, "bind": format!("$c_{session}")})
}

fn running(run: &mut Run, session: &str) {
    run.ok(create(session));
    run.ok(json!({"begin": {"op": {"Start": {"id": session}}}, "bind": "$s"}));
    run.ok(json!({"pump_until": {"event": {"SessionState": {"id": session, "state": "Running"}}}}));
}

#[test]
fn the_injected_clock_stamps_unix_time_from_the_monotonic_clock_and_from_a_jump() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"poll_all": {}}));
    // Without a jump: the unix time is the start (1000000) plus the whole seconds of the monotonic clock.
    run.ok(json!({"advance_clock": {"ms": 5500}}));
    run.ok(json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "78"}}));
    run.ok(json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output", "at": 1000005}}}}));
    // A jump of the unix clock alone, then more monotonic time: the unix time is the jump plus the seconds since it.
    run.ok(json!({"poll_all": {}}));
    run.ok(json!({"advance_clock": {"unix": 42}}));
    run.ok(json!({"advance_clock": {"ms": 3200}}));
    run.ok(json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "78"}}));
    run.ok(
        json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output", "at": 45}}}}),
    );
}

#[test]
fn a_worker_offered_under_a_file_name_reaches_open_and_a_harness_without_one_is_unsupported() {
    let harness = Harness::new(None);
    let opened = harness.opened_with.clone();
    let mut run = Run::new(harness);
    run.ok(json!({"open": {"as": "w", "data_dir": "e", "worker": {"file_name": "any.name"}}}));
    assert_eq!(
        opened.lock().unwrap().last().unwrap().as_deref(),
        Some("any.name")
    );
    run.err(json!({"open": {"as": "x", "data_dir": "f", "worker": {"no_name": 1}}}));
    // FakeCore's own harness cannot offer a named worker.
    let mut fake = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    let step = json!({"open": {"as": "w", "data_dir": "e", "worker": {"file_name": "any.name"}}});
    assert!(format!("{:?}", fake.exec(&step, &mut b, &Deadline::none())).contains("Unsupported"));
}

#[test]
fn a_begin_that_succeeds_fails_the_step_that_expected_an_error() {
    let mut run = Run::new(Harness::new(None));
    let mut step = create("s1");
    step["expect_error"] = json!({"code": "IdInUse"});
    run.err(step);
}

#[test]
fn a_call_that_succeeds_fails_the_step_that_expected_an_error() {
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1", "result": {}}}}}));
    run.err(json!({"call": {"method": "get", "args": {"id": "s1"}}, "expect_error": {"code": "UnknownSession"}}));
    run.ok(json!({"call": {"method": "get", "args": {"id": "nobody"}}, "expect_error": {"code": "UnknownSession"}}));
}

#[test]
fn call_and_control_results_are_bound_whole_and_by_field() {
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$c_s1", "result": {}}}}}));
    run.ok(json!({"call": {"method": "get", "args": {"id": "s1"}}, "bind": "$rec", "bind_fields": {"id": "$rid", "labels": "$lab"}}));
    assert_eq!(run.bindings["rid"], json!("s1"));
    assert_eq!(run.bindings["lab"], json!({}));
    assert_eq!(run.bindings["rec"]["state"], json!("Created"));
    // A field that the result lacks is a step error.
    run.err(
        json!({"call": {"method": "get", "args": {"id": "s1"}}, "bind_fields": {"nope": "$x"}}),
    );
    run.ok(json!({"pump": {}, "bind": "$report"}));
    assert!(run.bindings["report"].get("more").is_some());
}

#[test]
fn let_computes_values_from_bound_values() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"let": {"$a": {"add": [3, 4]}, "$b": {"sub": [3, 4]}}}));
    assert_eq!(
        (run.bindings["a"].clone(), run.bindings["b"].clone()),
        (json!(7), json!(-1))
    );
    run.ok(json!({"let": {"$e1": {"eq": [1, 1]}, "$e2": {"eq": [1, 2]}, "$c1": {"contains": [[1, 2], 2]}, "$c2": {"contains": [[1, 2], 3]}}}));
    assert_eq!(
        [
            &run.bindings["e1"],
            &run.bindings["e2"],
            &run.bindings["c1"],
            &run.bindings["c2"]
        ],
        [&json!(true), &json!(false), &json!(true), &json!(false)]
    );
    // The boundaries: a result that fits u64 but not i64 keeps its value; one outside both is a step error, never wrapped.
    run.ok(json!({"let": {"$big": {"add": [9223372036854775807u64, 1]}, "$top": {"add": [18446744073709551614u64, 1]}, "$low": {"sub": [-9223372036854775807i64, 1]}}}));
    assert_eq!(run.bindings["big"], json!(9223372036854775808u64));
    assert_eq!(run.bindings["top"], json!(u64::MAX));
    assert_eq!(run.bindings["low"], json!(i64::MIN));
    run.err(json!({"let": {"$x": {"add": [18446744073709551615u64, 1]}}}));
    run.err(json!({"let": {"$x": {"sub": [-9223372036854775808i64, 1]}}}));
    run.ok(json!({"let": {"$f": {"field": [{"k": 5}, "k"]}}}));
    assert_eq!(run.bindings["f"], json!(5));
    run.err(json!({"let": {"$x": {"field": [{"k": 5}, "missing"]}}}));
    run.err(json!({"let": {"$x": {"add": ["a", 1]}}}));
    run.err(json!({"let": {"$x": {"unknown": [1, 2]}}}));
}

#[test]
fn every_relation_of_check_holds_and_fails_as_it_says() {
    let mut run = Run::new(Harness::new(None));
    let rel = |r: Value| json!({"check": r});
    for holds in [
        json!({"eq": [1, 1]}),
        json!({"ne": [1, 2]}),
        json!({"lt": [1, 2]}),
        json!({"gt": [2, 1]}),
        json!({"mul_eq": [3, 4, 12]}),
        json!({"contains": [[1, 2], 2]}),
        json!({"subset_of": [[1], [1, 2]]}),
        json!({"subset_of": [[], [1]]}),
        json!({"hex_len_eq": ["aabb", 2]}),
        json!({"hex_prefix": ["aa", "aabb"]}),
        json!({"hex_ends_with": ["aabb", "bb"]}),
    ] {
        run.ok(rel(holds));
    }
    for fails in [
        json!({"eq": [1, 2]}),
        json!({"ne": [1, 1]}),
        json!({"lt": [2, 1]}),
        json!({"lt": [1, 1]}),
        json!({"gt": [1, 2]}),
        json!({"mul_eq": [3, 4, 13]}),
        json!({"contains": [[1, 2], 3]}),
        json!({"contains": ["x", 1]}),
        json!({"contains": [[1], 1, 5]}),
        json!({"subset_of": [[1, 3], [1, 2]]}),
        json!({"subset_of": ["x", [1]]}),
        json!({"subset_of": [[1], "x"]}),
        json!({"subset_of": [[1], [1], [1]]}),
        json!({"hex_len_eq": ["aabb", 3]}),
        json!({"hex_prefix": ["bb", "aabb"]}),
        json!({"hex_ends_with": ["aabb", "aa"]}),
        json!({"unknown": [1, 2]}),
    ] {
        run.err(rel(fails.clone()));
    }
}

#[test]
fn pump_idle_pumps_until_no_work_is_runnable_and_polls_nothing() {
    // The wrapper reports runnable work for three pumps.
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut run = Run::new(Harness::new(Some(pumps.clone())));
    run.ok(create("s1"));
    run.ok(json!({"pump_idle": {}}));
    assert_eq!(pumps.load(Ordering::SeqCst), 4);
    // The events that the pumps posted are queued: nothing polled them.
    run.ok(json!({"poll_events": {"max": 64}, "expect": [{"$any": true}, {"$any": true}, {"$any": true}]}));
}

#[test]
fn drain_alternates_pumps_and_polls_until_a_pump_and_the_poll_after_it_find_nothing() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut run = Run::new(Harness::new(Some(pumps.clone())));
    run.ok(create("s2"));
    run.ok(json!({"drain": {}}));
    // Pumps 1 to 3 had work. Pump 4 had none, and the poll after it found the synthetic event: pump 5 confirms the rest.
    assert_eq!(pumps.load(Ordering::SeqCst), 5);
    run.ok(json!({"poll_events": {"max": 64}, "expect": []}));
    // The pumps created the session.
    run.ok(
        json!({"call": {"method": "get", "args": {"id": "s2"}}, "expect": {"state": "Created"}}),
    );
}

#[test]
fn an_exhaustive_collect_fails_on_an_event_that_no_matcher_wants() {
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    let both = json!([{"SessionState": {"id": "s1", "state": "Created"}}, {"Completed": {"op": "$c_s1", "result": {}}}]);
    run.ok(json!({"pump_collect": {"events": both, "exhaustive": true}}));
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    // The completion is not wanted.
    run.err(json!({"pump_collect": {"events": [{"SessionState": {"id": "s1", "state": "Created"}}], "exhaustive": true}}));
    // Without `exhaustive` the same step passes.
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(
        json!({"pump_collect": {"events": [{"SessionState": {"id": "s1", "state": "Created"}}]}}),
    );
}

#[test]
fn an_exhaustive_collect_fails_on_an_event_that_is_queued_after_the_last_match() {
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_idle": {}}));
    // The Completed event comes first and is matched; SessionState is then the one that no matcher wants.
    run.err(json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1", "result": {}}}], "exhaustive": true}}));
}

#[test]
fn poll_all_drops_queued_events_without_a_pump() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut run = Run::new(Harness::new(Some(pumps.clone())));
    run.ok(create("s1"));
    run.ok(json!({"pump": {}}));
    assert_eq!(pumps.load(Ordering::SeqCst), 1);
    run.ok(json!({"poll_all": {}}));
    assert_eq!(pumps.load(Ordering::SeqCst), 1);
    run.ok(json!({"poll_events": {"max": 64}, "expect": []}));
}

#[test]
fn collect_events_runs_until_a_pump_and_the_poll_after_it_find_nothing_and_keeps_every_event() {
    // The wrapper reports runnable work for three pumps and delivers one synthetic event at the fourth poll (Core A5-1).
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut run = Run::new(Harness::new(Some(pumps.clone())));
    run.ok(create("s2"));
    run.ok(json!({"collect_events": {}, "bind": "$e"}));
    // Pump 4 had no work and its poll found the synthetic event, so pump 5 confirms the rest.
    assert_eq!(pumps.load(Ordering::SeqCst), 5);
    let events = run.bindings["e"].as_array().expect("a list");
    assert!(
        events.iter().any(|e| e.get("RouteStalled").is_some()),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| e.get("Completed").is_some()),
        "{events:?}"
    );
}

#[test]
fn a_control_that_names_a_route_reaches_that_routes_endpoint_and_not_the_harness() {
    let harness = Harness::new(None);
    let log = harness.route_controls.clone();
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    running(&mut run, "s2");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    run.ok(json!({"attach_route": {"route": "r2", "session": "s2", "client": "c2"}}));
    run.ok(
        json!({"control": {"op": "input_blocked"}, "route": "r2", "expect": {"blocked": false}}),
    );
    run.ok(
        json!({"control": {"op": "input_blocked"}, "route": "r1", "expect": {"blocked": false}}),
    );
    // The second attach is endpoint 2: the route name picks the endpoint, in either order.
    let seen: Vec<(usize, String)> = log.lock().unwrap().clone();
    assert!(seen.contains(&(2, "input_blocked".into())), "{seen:?}");
    assert!(seen.contains(&(1, "input_blocked".into())), "{seen:?}");
    let first = seen.iter().position(|e| e.0 == 2).unwrap();
    let second = seen.iter().position(|e| e.0 == 1).unwrap();
    assert!(first < second, "r2 was asked first: {seen:?}");
}

#[test]
fn let_str_gives_the_decimal_string_of_a_number() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"let": {"$n": {"add": [41, 1]}}}));
    run.ok(json!({"let": {"$s": {"str": ["$n", 0]}}}));
    run.ok(json!({"check": {"eq": ["$s", "42"]}}));
    run.err(json!({"let": {"$bad": {"str": ["x", 0]}}}));
}

#[test]
fn await_quiet_pumps_without_polling_until_no_work_is_runnable_and_the_edges_are_quiet() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut harness = Harness::new(Some(pumps.clone()));
    harness.quiet_from = 4;
    let calls = harness.quiet_calls.clone();
    let mut run = Run::new(harness);
    run.ok(create("s1"));
    run.ok(json!({"await_quiet": {}}));
    // The wrapper reports runnable work for three pumps; the edges are quiet from the fourth answer on: four pumps, four answers.
    assert_eq!(pumps.load(Ordering::SeqCst), 4);
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    // Nothing was polled: the events of Create are still queued.
    run.ok(json!({"poll_events": {"max": 64}, "expect": [{"$any": true}, {"$any": true}, {"$any": true}]}));
}

/// A Core with one deadline (a capture expiry, 60 s) and nothing runnable, and a `pump_until` that no event matches.
fn idle_with_a_deadline(harness: Harness) -> Run {
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    run.ok(json!({"begin": {"op": {"CaptureSnapshot": {"session": "s1", "owner": "c1"}}}, "bind": "$k"}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$k", "result": {}}}}}));
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": {"in_ms": 60000}}));
    run
}

const NO_EVENT: &str = r#"{"pump_until": {"event": {"RouteStalled": {"route": 77}}}}"#;

/// Steward ruling R-46: an idle wait jumps the injected clock to Core's next deadline only when the harness's progress is
/// injected too. Otherwise the clock moves only by `advance_clock`, and the wait ends at the step limit.
#[test]
fn an_idle_wait_jumps_the_injected_clock_only_when_progress_is_injected() {
    let never: Value = serde_json::from_str(NO_EVENT).unwrap();
    // The testkit and the fake: the jump expires the capture, and then the Core is idle.
    let mut run = idle_with_a_deadline(Harness::new(None));
    let r = run.exec(never.clone()).unwrap_err();
    assert!(r.contains("the Core is idle"), "{r}");
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": null}));

    // Real progress under an injected clock: no jump. The step limit ends the wait, and the deadline is still 60 s away.
    let mut harness = Harness::new(None);
    harness.progress_injected = false;
    let mut run = idle_with_a_deadline(harness);
    // timer: deadline — the step limit that ends the idle wait
    let limit = Deadline::after(Some(Duration::from_millis(50)));
    let r = run.exec_within(never.clone(), &limit).unwrap_err();
    assert!(
        r.contains("Inconclusive") && r.contains("step_timeout"),
        "{r}"
    );
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": {"in_ms": 60000}}));
    // Only `advance_clock` moves it.
    run.ok(json!({"advance_clock": {"ms": 60000}}));
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": {"in_ms": 0}}));
}

/// A real clock never jumps, and Core's deadline bounds its idle wait (Core TM-3): the wake handle blocks for its whole
/// timeout here, as a real one does when nothing wakes it. A Stop of a session that ignores the graceful request arms the
/// kill at the stop grace (100 ms); the kill completes the Stop well inside the step limit. A wait bounded only by the step
/// limit would sleep it out, and the step would end inconclusive.
#[test]
fn a_real_idle_wait_ends_at_cores_next_deadline() {
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    harness.real_clock = true;
    let mut run = Run {
        driver: CoreDriver::new(Box::new(harness)),
        bindings: Bindings::new(),
    };
    run.ok(json!({"open": {"as": "h", "data_dir": "d", "limits": {"stop_grace": 100}}}));
    run.ok(json!({"begin": {"op": {"Create": {"session": "s1", "request": {"program": [{"ignore_sigterm": {}}, {"hold": {}}]}}}}, "bind": "$c"}));
    run.ok(json!({"begin": {"op": {"Start": {"id": "s1"}}}, "bind": "$s"}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$s", "result": {}}}}}));
    run.ok(json!({"begin": {"op": {"Stop": {"id": "s1"}}}, "bind": "$p"}));
    // timer: deadline — the step limit; Core's 100 ms deadline must end the idle wait long before it
    let limit = Deadline::after(Some(Duration::from_secs(3)));
    run.exec_within(
        json!({"pump_until": {"event": {"Completed": {"op": "$p", "result": {"ok": {"end": {"Exited": {"cause": "Killed"}}}}}}}}),
        &limit,
    )
    .unwrap();
}

#[test]
fn await_quiet_ends_at_the_runner_limit_never_as_a_failure_and_never_moves_the_clock() {
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    harness.quiet_from = 0;
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    run.ok(json!({"begin": {"op": {"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}}, "bind": "$c"}));
    run.ok(json!({"begin": {"op": {"Start": {"id": "s1"}}}, "bind": "$s"}));
    run.ok(json!({"pump_until": {"event": {"SessionState": {"id": "s1", "state": "Running"}}}}));
    run.ok(json!({"begin": {"op": {"CaptureSnapshot": {"session": "s1", "owner": "c1"}}}, "bind": "$k"}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$k", "result": {}}}}}));
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": {"in_ms": 60000}}));
    // The edge never reports quiet: the wait ends as a runner limit. It is not a failure, and the injected clock stood still.
    let r = run.exec(json!({"await_quiet": {}})).unwrap_err();
    assert!(r.contains("Inconclusive") && r.contains("max_polls"), "{r}");
    // Every idle round waited on the wake handle: an edge announces its report there.
    assert!(
        waits.load(Ordering::SeqCst) >= 100,
        "{}",
        waits.load(Ordering::SeqCst)
    );
    run.ok(json!({"call": {"method": "next_deadline"}, "expect": {"in_ms": 60000}}));
    // A step deadline that has passed ends it as the other runner limit.
    let past = Deadline::after(Some(std::time::Duration::ZERO));
    let r = run
        .driver
        .exec(&json!({"await_quiet": {}}), &mut run.bindings, &past);
    assert!(format!("{r:?}").contains("step_timeout"), "{r:?}");
    // FakeCore's own harness has no such control.
    let mut fake = CoreDriver::new(Box::new(FakeCoreHarness::new(0)));
    let mut b = Bindings::new();
    fake.exec(
        &json!({"open": {"as": "h", "data_dir": "d"}}),
        &mut b,
        &Deadline::none(),
    )
    .unwrap();
    let r = fake.exec(&json!({"await_quiet": {}}), &mut b, &Deadline::none());
    assert!(format!("{r:?}").contains("Unsupported"));
}

#[test]
fn await_control_pumps_without_polling_until_the_control_answers_as_expected() {
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    harness.quiet_from = 3;
    let calls = harness.quiet_calls.clone();
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    run.ok(create("s1"));
    // `edges_quiet` answers quiet from its third call: the step pumps between the calls, and polls nothing.
    run.ok(json!({"await_control": {"control": {"op": "edges_quiet"}, "expect": {"quiet": true}}}));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    // The condition is checked before the first pump and after each pump: the step waited once, after the second answer, and not after the
    // pump that satisfied it.
    assert_eq!(waits.load(Ordering::SeqCst), 1);
    run.ok(json!({"poll_events": {"max": 64}, "expect": [{"SessionState": {"id": "s1", "state": "Created"}}]}));
    // A control that is not offered is unsupported; one that never answers as expected ends at the runner limit.
    run.err(json!({"await_control": {"control": {"op": "no_such_control"}, "expect": {}}}));
    let r = run.exec(json!({"await_control": {"control": {"op": "edges_quiet"}, "expect": {"quiet": "never"}}})).unwrap_err();
    assert!(r.contains("max_polls"), "{r}");
    run.err(json!({"await_control": {"expect": {}}}));
    run.err(json!({"await_control": {"control": {"op": "edges_quiet"}}}));
    // Bound values reach the control and the matcher binds.
    run.ok(json!({"let": {"$q": {"add": [0, 0]}}}));
    run.ok(
        json!({"await_control": {"control": {"op": "edges_quiet"}, "expect": {"quiet": "$got"}}}),
    );
    assert_eq!(run.bindings["got"], json!(true));
}

#[test]
fn await_get_pumps_without_polling_until_the_cached_state_matches() {
    let harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    run.ok(create("s1"));
    run.ok(json!({"await_get": {"id": "s1", "expect": {"state": "Created"}}}));
    // The event of Create is still queued: nothing polled.
    run.ok(json!({"poll_events": {"max": 64}, "expect": [{"SessionState": {"id": "s1", "state": "Created"}}]}));
    run.ok(json!({"await_get": {"id": "nobody", "expect_error": {"code": "UnknownSession"}}}));
    run.err(json!({"await_get": {"expect": {}}}));
    // A state that never comes ends at the runner limit, after a wait on the wake handle in every failed round.
    let before = waits.load(Ordering::SeqCst);
    let r = run
        .exec(json!({"await_get": {"id": "s1", "expect": {"state": "Running"}}}))
        .unwrap_err();
    assert!(r.contains("max_polls"), "{r}");
    assert!(waits.load(Ordering::SeqCst) - before >= 100);
    let r = run.exec(json!({"await_get": {"id": "s1"}})).unwrap_err();
    assert!(r.contains("max_polls"), "{r}");
}

#[test]
fn open_limits_receive_the_values_that_earlier_steps_bound() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"let": {"$cap": {"add": [1000, 2000]}}}));
    // `capture_ttl` 3000 ms is valid only if the variable was substituted: the text "$cap" is not a limit.
    run.ok(json!({"open": {"as": "w", "data_dir": "e", "limits": {"capture_ttl": "$cap"}}}));
    run.err(json!({"open": {"as": "x", "data_dir": "f", "limits": {"capture_ttl": "$unbound"}}}));
}

#[test]
fn a_fence_whose_condition_the_pump_satisfied_does_not_wait_on_the_wake_handle() {
    // The condition is checked after the pump and before the wait: a pump that satisfied it needs no wake (Core TM-6 clears the signal).
    let harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    run.ok(create("s1"));
    run.ok(json!({"await_get": {"id": "s1", "expect": {"state": "Created"}}}));
    assert_eq!(waits.load(Ordering::SeqCst), 0);
}

#[test]
fn assume_gives_inconclusive_when_the_precondition_fails_and_an_error_when_it_is_malformed() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"let": {"$n": {"add": [3, 4]}}}));
    run.ok(json!({"assume": {"lt": ["$n", 8]}}));
    // The precondition does not hold: the run proved nothing, which is not a failure.
    let r = run.exec(json!({"assume": {"lt": ["$n", 7]}})).unwrap_err();
    assert!(
        r.contains("Inconclusive") && r.contains("assumption"),
        "{r}"
    );
    // A relation that is not one, or a name that no step bound, is a transcript error.
    let r = run.exec(json!({"assume": {"nope": [1, 2]}})).unwrap_err();
    assert!(r.contains("Failed") && !r.contains("Inconclusive"), "{r}");
    let r = run
        .exec(json!({"assume": {"lt": ["$unbound", 1]}}))
        .unwrap_err();
    assert!(
        r.contains("unbound variable") && !r.contains("Inconclusive"),
        "{r}"
    );
}

#[test]
fn a_relation_with_the_wrong_argument_count_is_an_error_for_check_and_for_assume() {
    let mut run = Run::new(Harness::new(None));
    for malformed in [
        json!({"gt": [1]}),
        json!({"eq": []}),
        json!({"lt": [1, 2, 3]}),
        json!({"mul_eq": [1, 2]}),
        json!({"contains": [[1]]}),
        json!({"subset_of": [[1], [1], [1]]}),
        json!({"hex_len_eq": ["aa"]}),
        json!({"hex_prefix": []}),
        json!({"hex_ends_with": ["aa"]}),
        json!({"ne": [1]}),
    ] {
        for step in ["check", "assume"] {
            let r = run.exec(json!({ step: malformed.clone() })).unwrap_err();
            assert!(
                r.contains("takes") && !r.contains("Inconclusive") && !r.contains("does not hold"),
                "{step} {malformed}: {r}"
            );
        }
    }
    // A valid relation that is false stays a failure for `check` and `inconclusive` for `assume`.
    assert!(run
        .exec(json!({"check": {"gt": [1, 2]}}))
        .unwrap_err()
        .contains("does not hold"));
    assert!(run
        .exec(json!({"assume": {"gt": [1, 2]}}))
        .unwrap_err()
        .contains("Inconclusive"));
}

const DEFERRED_OK: &str = "# c\nconf::ad_4_previous_worker_version_adopts  worker-protocol-2  (Core A6-2)  until: the first release whose worker protocol T is 2 (more)\nconf::ad_4_missing_worker_capability_is_unsupported  w  (Core A6-2)  until: the first release whose protocol T adds a worker feature that the pinned T-1 worker lacks\nnot-applicable conf::dp_12_x  n  (Core A6-2)  because: z\n";

#[test]
fn a6_2_deferred_set_accepts_the_enumerated_set_and_the_repository_file() {
    botster_core_conformance::a6_2_deferred_set(DEFERRED_OK).unwrap();
    let repo = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/deferred.txt"
    ))
    .unwrap();
    botster_core_conformance::a6_2_deferred_set(&repo).unwrap();
}

#[test]
fn a6_2_deferred_set_rejects_a_third_id_a_missing_id_and_a_wrong_condition() {
    let f = botster_core_conformance::a6_2_deferred_set;
    let extra = format!("{DEFERRED_OK}conf::ad_9_other  r  (Core A6-2)  until: later\n");
    assert!(f(&extra).is_err());
    let missing: String = DEFERRED_OK
        .lines()
        .filter(|l| !l.contains("capability"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(f(&missing).is_err());
    let wrong = DEFERRED_OK.replace("worker protocol T is 2", "worker protocol T is 3");
    assert!(f(&wrong).is_err());
    let none = DEFERRED_OK.replace("until:", "since:");
    assert!(f(&none).is_err());
    // The capability id waits until the pinned T-1 worker LACKS the feature: a missing or an opposite qualifier is not that condition.
    let no_qualifier = DEFERRED_OK.replace(" that the pinned T-1 worker lacks", "");
    assert!(f(&no_qualifier).is_err());
    let opposite = DEFERRED_OK.replace("worker lacks", "worker has");
    assert!(f(&opposite).is_err());
}

#[test]
fn the_a6_2_type_check_runs_against_the_repository_deferred_file_and_an_unknown_check_is_unsupported(
) {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"type_check": "a6_2_deferred_set", "because": "Core A6-2"}));
    run.err(json!({"type_check": "no_such_check", "because": "Core A6-2"}));
}

#[test]
fn a_forbidden_event_fails_the_collect_and_an_absent_one_does_not() {
    let both = json!([{"SessionState": {"id": "s1", "state": "Created"}}, {"Completed": {"op": "$c_s1", "result": {}}}]);
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.err(json!({"pump_collect": {"events": both, "forbid": [{"SessionState": {"id": "s1", "state": "Created"}}]}}));
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_collect": {"events": both, "forbid": [{"SessionState": {"id": "s2"}}]}}));
}

#[test]
fn forbid_wins_over_ignore_in_the_final_drain_and_in_the_loop() {
    // The Completed event is wanted. The SessionState event is ignored by a broad matcher and forbidden by a narrow one.
    let ignore = json!([{"SessionState": {"id": "s1"}}]);
    let forbid = json!([{"SessionState": {"id": "s1", "state": "Created"}}]);
    // The wanted event comes first, so the SessionState is queued after the last match: the exhaustive drain sees it.
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_idle": {}}));
    run.err(json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1", "result": {}}}], "exhaustive": true, "ignore": ignore, "forbid": forbid}}));
    // Without the forbid the same step passes: the ignore alone lets the event through.
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"pump_idle": {}}));
    run.ok(json!({"pump_collect": {"events": [{"Completed": {"op": "$c_s1", "result": {}}}], "exhaustive": true, "ignore": ignore}}));
    // Both wanted: the SessionState arrives in the loop, where forbid also comes first.
    let both = json!([{"Completed": {"op": "$c_s1", "result": {}}}, {"SessionState": {"id": "s1", "state": "Created"}}]);
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.err(json!({"pump_collect": {"events": both, "exhaustive": true, "ignore": ignore, "forbid": forbid}}));
}

#[test]
fn real_wait_against_a_real_clock_lasts_its_interval_and_keeps_the_events() {
    let mut harness = Harness::new(None);
    harness.real_clock = true;
    let mut run = Run::new(harness);
    run.ok(create("s1"));
    let started = Instant::now();
    run.ok(json!({"real_wait": {"ms": 200}}));
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(200),
        "the wait ended after {took:?}"
    );
    assert!(took < Duration::from_secs(5), "the wait lasted {took:?}");
    run.ok(
        json!({"pump_collect": {"events": [{"SessionState": {"id": "s1", "state": "Created"}}]}}),
    );
}

#[test]
fn real_wait_waits_on_the_wake_handle_not_on_a_polling_period_and_obeys_the_step_deadline() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut harness = Harness::new(Some(pumps));
    harness.real_clock = true;
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    let before = waits.load(Ordering::SeqCst);
    run.ok(json!({"real_wait": {"ms": 300}}));
    let used = waits.load(Ordering::SeqCst) - before;
    // A 20 ms period would wait about fifteen times in 300 ms; an event-driven wait waits once for the interval, and once more for each wake.
    assert!(used <= 4, "{used} waits in one interval");
    // The step deadline ends a long interval: the step is inconclusive and returns at once.
    let mut harness = Harness::new(None);
    harness.real_clock = true;
    let mut run = Run::new(harness);
    let started = Instant::now();
    let r = run.driver.exec(
        &json!({"real_wait": {"ms": 60000}}),
        &mut run.bindings,
        &Deadline::after(Some(Duration::from_millis(100))),
    );
    assert!(format!("{r:?}").contains("step_timeout"), "{r:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

/// Core SV-3, A2-5, SV-9: the lane calls and the log read reach `CoreApi`, and a lane error is a value of the result, not a step error.
#[test]
fn the_lane_calls_return_their_typed_values_and_the_log_read_gives_its_length() {
    let mut run = Run::new(Harness::new(None));
    let id = "1111111111111111111111111111111111111111111111111111111111111111";
    let other = "2222222222222222222222222222222222222222222222222222222222222222";
    run.ok(json!({"begin": {"op": {"SpawnService": {"service_id": id}}}, "bind": "$sp"}));
    run.ok(json!({"pump_until": {"event": {"Completed": {"op": "$sp", "result": {}}}}}));
    let send = |service: &str| json!({"call": {"method": "service_send", "args": {"id": service, "lane": 0, "type": 1, "payload_hex": "aa"}}});
    let mut step = send(id);
    step["expect"] = json!({"error": "NotConnected"});
    run.ok(step);
    let mut step = send(other);
    step["expect"] = json!({"error": "UnknownService"});
    run.ok(step);
    // The expectation is checked: a value that the call did not return fails the step.
    let mut step = send(id);
    step["expect"] = json!({"error": "Backpressured"});
    run.err(step);
    run.ok(json!({"call": {"method": "service_recv", "args": {"id": id, "lane": 0}}, "expect": {"ok": null}}));
    run.ok(json!({"call": {"method": "service_recv", "args": {"id": other, "lane": 0}}, "expect": {"error": "UnknownService"}}));
    run.ok(json!({"call": {"method": "service_log_tail", "args": {"id": id, "max": 16}}, "expect": {"len": 0, "bytes": {"$bytes_hex": ""}}}));
    run.ok(json!({"call": {"method": "service_log_tail", "args": {"id": other, "max": 16}}, "expect_error": {"code": "UnknownService"}}));
    // A lane call without a lane, with a lane over 255, or with a payload that is not hex is a transcript error.
    run.err(json!({"call": {"method": "service_recv", "args": {"id": id}}}));
    run.err(json!({"call": {"method": "service_recv", "args": {"id": id, "lane": 256}}}));
    run.err(json!({"call": {"method": "service_send", "args": {"id": id, "lane": 0, "type": 1, "payload_hex": "zz"}}}));
    run.err(json!({"call": {"method": "service_send", "args": {"id": id, "lane": 0, "type": 1}}}));
    run.err(json!({"call": {"method": "service_send", "args": {"id": id, "lane": 0, "type": 1, "payload_hex": 7}}}));
    run.err(json!({"call": {"method": "service_send", "args": {"id": id, "lane": 0, "type": 256, "payload_hex": "aa"}}}));
    run.err(json!({"call": {"method": "service_send", "args": {"id": id, "lane": 0, "payload_hex": "aa"}}}));
}

/// The flood fills the route's queue with refusals until the stream takes no more input, and the check reads every refusal back in order
/// (Core DP-5). A queue bound of 3000 bytes, the stalled reader and the fake route's `input_blocked` stand in for the testkit's edge.
fn flooded_route() -> (Run, u64) {
    flooded_route_with(Harness::new(None))
}

fn flooded_route_with(harness: Harness) -> (Run, u64) {
    let mut run = Run::new(harness);
    run.ok(json!({"open": {"as": "f", "data_dir": "f", "limits": {"max_snapshot_bytes": 64, "snapshot_retained_bytes": 64, "route_queue_bytes": 3000}}}));
    run.ok(json!({"begin": {"op": {"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}, "handle": "f"}, "bind": "$c"}));
    run.ok(json!({"begin": {"op": {"Start": {"id": "s1"}}, "handle": "f"}, "bind": "$s"}));
    run.ok(
        json!({"pump_until": {"event": {"Completed": {"op": "$s", "result": {}}}, "handle": "f"}}),
    );
    run.ok(
        json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1", "handle": "f"}}),
    );
    run.ok(json!({"route_drain": true, "route": "r1"}));
    run.ok(json!({"control": {"op": "stall_reader", "on": true}, "route": "r1"}));
    run.ok(json!({"route_flood": {"route": "r1", "max": 5000}, "bind": "$n"}));
    let n = run
        .bindings
        .get("n")
        .and_then(Value::as_u64)
        .expect("the count is bound");
    (run, n)
}

#[test]
fn route_flood_stops_at_the_first_frame_that_blocks_the_stream_and_binds_the_count() {
    let (mut run, n) = flooded_route();
    assert!(n > 1, "{n}");
    // The count is the first blocking frame: the stream took the one before.
    run.ok(json!({"control": {"op": "input_blocked"}, "route": "r1", "expect": {"blocked": true}}));
    // A flood that cannot block within its bound fails.
    let mut again = Run::new(Harness::new(None));
    running(&mut again, "s2");
    again.ok(json!({"attach_route": {"route": "r9", "session": "s2", "client": "c9"}}));
    let capped = again.exec(json!({"route_flood": {"route": "r9", "max": 3}, "bind": "$m"}));
    assert!(
        capped
            .as_ref()
            .is_err_and(|e| e.contains("Inconclusive") && e.contains("route_flood_max")),
        "{capped:?}"
    );
}

#[test]
fn route_expect_refusals_reads_every_refusal_in_order_and_fails_on_a_missing_one() {
    let (mut run, n) = flooded_route();
    run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
    run.ok(json!({"route_expect_refusals": {"route": "r1", "count": n, "reason": {"unsupported": {"what": "utf8"}}}}));
    // One more than were sent never arrives.
    for more in [1, 2] {
        let (mut run, n) = flooded_route();
        run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
        let missing =
            run.exec(json!({"route_expect_refusals": {"route": "r1", "count": n + more}}));
        assert!(
            missing
                .as_ref()
                .is_err_and(|e| e.contains("Inconclusive") && e.contains("route_idle")),
            "{more}: {missing:?}"
        );
    }
    // A closed route says that no refusal will come: the step fails, and does not wait.
    let (mut run, n) = flooded_route();
    run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
    run.ok(json!({"control": {"op": "client_close"}, "route": "r1"}));
    let closed = run.exec(json!({"route_expect_refusals": {"route": "r1", "count": n + 1}}));
    assert!(
        closed
            .as_ref()
            .is_err_and(|e| e.contains("Failed") && e.contains("closed")),
        "{closed:?}"
    );
    // A reason that the refusals do not have fails.
    let (mut run, n) = flooded_route();
    run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
    let wrong = run
        .exec(json!({"route_expect_refusals": {"route": "r1", "count": n, "reason": "too_large"}}));
    assert!(
        wrong.as_ref().is_err_and(|e| e.contains("too_large")),
        "{wrong:?}"
    );
    // One refusal is asked for, and its reason is wrong: the step reads before it decides.
    let (mut run, _) = flooded_route();
    run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
    let first = run
        .exec(json!({"route_expect_refusals": {"route": "r1", "count": 1, "reason": "too_large"}}));
    assert!(
        first.as_ref().is_err_and(|e| e.contains("too_large")),
        "{first:?}"
    );
}

#[test]
fn poll_find_reads_the_queue_without_a_pump_takes_one_event_and_keeps_the_others() {
    let created = json!({"SessionState": {"id": "s1", "state": "Created"}});
    let done = json!({"Completed": {"op": "$c_s1", "result": {}}});
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    // Nothing was pumped: no event is queued, and the step does not pump to find one.
    run.err(json!({"poll_find": {"event": created}}));
    run.ok(json!({"pump_idle": {}}));
    run.ok(json!({"poll_find": {"event": created}}));
    // The matched event is taken, and the other stays: the step finds the completion once, and not twice.
    run.err(json!({"poll_find": {"event": created}}));
    run.ok(json!({"poll_find": {"event": done}}));
    run.err(json!({"poll_find": {"event": done}}));
    run.err(json!({"poll_find": {}}));
}

#[test]
fn poll_find_polls_every_batch_of_the_queue_not_only_the_first() {
    // A poll returns two events at most: the events of the last session come in a later batch.
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(100))));
    harness.poll_batch = 2;
    let mut run = Run::new(harness);
    for n in 1..=4 {
        run.ok(create(&format!("s{n}")));
    }
    run.ok(json!({"pump_idle": {}}));
    run.ok(json!({"poll_find": {"event": {"SessionState": {"id": "s4", "state": "Created"}}}}));
}

#[test]
fn let_joins_strings_and_measures_strings_and_hex_values() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"let": {"$p": {"concat": ["/tmp/a", " "]}}}));
    run.ok(json!({"check": {"eq": ["$p", "/tmp/a "]}}));
    run.ok(json!({"let": {"$n": {"str_len": ["$p", 0]}, "$h": {"hex_len": ["1b5b32", 0]}}}));
    run.ok(json!({"check": {"eq": ["$n", 7]}}));
    run.ok(json!({"check": {"eq": ["$h", 3]}}));
    run.ok(json!({"let": {"$b": {"hex_len": [{"$bytes_hex": "0102"}, 0]}}}));
    run.ok(json!({"check": {"eq": ["$b", 2]}}));
    run.ok(json!({"let": {"$r": {"repeat": ["ab", 3]}}}));
    run.ok(json!({"check": {"eq": ["$r", "ababab"]}}));
    run.ok(json!({"let": {"$z": {"repeat": ["ab", 0]}}}));
    run.ok(json!({"check": {"eq": ["$z", ""]}}));
    // The bound is 1 MiB: the count at the bound is accepted, and the next one is not.
    run.ok(json!({"let": {"$m": {"repeat": ["a", 1048576]}}}));
    run.err(json!({"let": {"$x": {"repeat": ["a", 1048577]}}}));
    run.err(json!({"let": {"$x": {"repeat": ["ab", 2000000]}}}));
    run.err(json!({"let": {"$x": {"repeat": [1, 2]}}}));
    run.err(json!({"let": {"$x": {"repeat": ["a", -1]}}}));
    run.err(json!({"let": {"$x": {"concat": ["a", 1]}}}));
    run.err(json!({"let": {"$x": {"str_len": [1, 0]}}}));
    run.err(json!({"let": {"$x": {"hex_len": [1, 0]}}}));
}

#[test]
fn await_pty_takes_a_bound_number_for_min_bytes_and_an_unbound_one_is_an_error() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"control": {"op": "pty_blocked", "session": "s1", "on": false}}));
    run.ok(json!({"begin": {"op": {"WriteInput": {"session": "s1", "payload": {"bytes": {"bytes": {"$bytes_hex": "6162"}}}}}}, "bind": "$w"}));
    run.ok(json!({"let": {"$n": {"hex_len": ["6162", 0]}}}));
    run.ok(json!({"await_pty": {"session": "s1", "min_bytes": "$n"}}));
    run.err(json!({"await_pty": {"session": "s1", "min_bytes": "$missing"}}));
}

// A worker PTY write posts no host event, and a pump can leave nothing more to run: the pump that wrote the byte reports no progress.
// The step reads the PTY again after that pump, before it asks whether the Core is idle (it failed with "the Core is idle" before).
#[test]
fn await_pty_reads_the_pty_again_after_a_pump_that_reports_no_progress() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let after = Arc::new(AtomicUsize::new(usize::MAX));
    let mut harness = Harness::new(Some(pumps.clone()));
    harness.pty_after_pumps = Some(after.clone());
    // The wake handle gives no signal when nothing is runnable (Core TM-6), as a real one does not.
    harness.real_clock = true;
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    // Nothing is left to run: the pump of the step reports no event and no more work.
    for _ in 0..4 {
        run.ok(json!({"pump": {}}));
    }
    run.ok(json!({"pump": {}, "bind": "$quiet"}));
    assert_eq!(run.bindings["quiet"]["more"], json!(false));
    assert_eq!(run.bindings["quiet"]["events_posted"], json!(0));
    // From here, the first pump of the step writes the byte.
    after.store(pumps.load(Ordering::SeqCst), Ordering::SeqCst);
    let waited = waits.load(Ordering::SeqCst);
    run.ok(json!({"await_pty": {"session": "s1", "min_bytes": 1}}));
    assert_eq!(
        pumps.load(Ordering::SeqCst),
        after.load(Ordering::SeqCst) + 1
    );
    // The byte came from that pump, so the step did not ask whether the Core is idle.
    assert_eq!(waits.load(Ordering::SeqCst), waited);
    // Two bytes never arrive: the Core is idle, and that is a failure, not an inconclusive run.
    let idle = run
        .exec(json!({"await_pty": {"session": "s1", "min_bytes": 2}}))
        .unwrap_err();
    assert!(idle.contains("the Core is idle"), "{idle}");
}

// Pumps that report more work are not an idle Core: the step goes on without asking, and reads the PTY after each pump.
#[test]
fn await_pty_does_not_ask_whether_the_core_is_idle_while_a_pump_reports_more_work() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let after = Arc::new(AtomicUsize::new(3));
    let mut harness = Harness::new(Some(pumps.clone()));
    harness.pty_after_pumps = Some(after);
    harness.real_clock = true;
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    // The first three pumps report more work; the byte is there after the fourth.
    run.ok(json!({"await_pty": {"session": "s1", "min_bytes": 1}}));
    assert_eq!(pumps.load(Ordering::SeqCst), 4);
    assert_eq!(waits.load(Ordering::SeqCst), 0);
}

// A step that runs out of time while the Core is idle proves nothing: it is inconclusive, never a failure.
#[test]
fn await_pty_is_inconclusive_when_the_step_ends_while_it_waits_for_an_idle_core() {
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    harness.pty_after_pumps = Some(Arc::new(AtomicUsize::new(usize::MAX)));
    harness.real_clock = true;
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    let step = json!({"await_pty": {"session": "s1", "min_bytes": 1}});
    let err = run
        .driver
        .exec(
            &step,
            &mut run.bindings,
            &Deadline::after(Some(Duration::from_millis(40))),
        )
        .unwrap_err();
    assert!(
        matches!(&err, botster_conformance::StepError::Inconclusive { limit } if limit == "step_timeout"),
        "{err:?}"
    );
}

#[test]
fn real_wait_with_an_injected_host_clock_waits_real_time_and_leaves_that_clock_alone() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"poll_all": {}}));
    let started = Instant::now();
    run.ok(json!({"real_wait": {"ms": 150}}));
    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "the interval was not real: {:?}",
        started.elapsed()
    );
    // The injected host clock did not move: the unix stamp is still the start of the clock plus no whole second.
    run.ok(json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "78"}}));
    run.ok(json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output", "at": 1000000}}}}));
    run.err(json!({"real_wait": {}}));
    // The observation keeps the events: a Created session's state is still there after the wait.
    let mut run = Run::new(Harness::new(None));
    run.ok(create("s1"));
    run.ok(json!({"real_wait": {"ms": 10}}));
    run.ok(
        json!({"pump_collect": {"events": [{"SessionState": {"id": "s1", "state": "Created"}}]}}),
    );
}

#[test]
fn a_duration_option_in_nanoseconds_reaches_the_attach_exactly_and_milliseconds_keep_their_meaning()
{
    let harness = Harness::new(None);
    let seen = harness.attach_options.clone();
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    let attach = |route: &str, query: Value| {
        json!({"attach_route": {"route": route, "session": "s1", "client": route,
            "options": {"answers_queries": true, "query_deadline": query, "stall_deadline": {"ns": 1_500_000_001u64}}}})
    };
    run.ok(attach("r1", json!({"ns": 500_000})));
    run.ok(attach("r2", json!(7)));
    let options = seen.lock().unwrap().clone();
    assert_eq!(options[0].query_deadline, Some(Duration::from_micros(500)));
    assert_eq!(
        options[0].stall_deadline,
        Some(Duration::from_nanos(1_500_000_001))
    );
    assert_eq!(options[1].query_deadline, Some(Duration::from_millis(7)));
}

#[test]
fn an_attach_option_that_names_a_bound_variable_reaches_the_attach_substituted() {
    let harness = Harness::new(None);
    let seen = harness.attach_options.clone();
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    run.bindings.insert("fmt".into(), json!("ghostsnp"));
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1",
        "options": {"terminal_formats": ["no_such_format", "$fmt"]}}}));
    let options = seen.lock().unwrap().clone();
    assert_eq!(
        options[0].terminal_formats,
        vec!["no_such_format", "ghostsnp"]
    );
    let unbound = run.exec(json!({"attach_route": {"route": "r2", "session": "s1",
        "options": {"terminal_formats": ["$never_bound"]}}}));
    assert!(unbound.unwrap_err().contains("unbound variable"));
}

#[test]
fn route_expect_refusals_reads_a_readable_route_while_the_host_stays_idle_and_never_infers_from_host_idleness(
) {
    // The route becomes readable (the reader is released) while the host posts nothing: the step reads every refusal, and never waits on
    // the host's wake handle (a pump of the handle, for the edges that deliver in a pump, is allowed).
    let harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    let waits = harness.waits.clone();
    let (mut run, n) = flooded_route_with(harness);
    run.ok(json!({"control": {"op": "stall_reader", "on": false}, "route": "r1"}));
    let w0 = waits.load(Ordering::SeqCst);
    run.ok(json!({"route_expect_refusals": {"route": "r1", "count": n}}));
    assert_eq!(
        waits.load(Ordering::SeqCst),
        w0,
        "the host's wake handle was waited on"
    );
    // An idle host and an unreadable route prove nothing: the step is inconclusive, and does not fail the worker on the host's idleness.
    let mut idle = Run::new(Harness::new(None));
    running(&mut idle, "s1");
    idle.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    idle.ok(json!({"route_drain": true, "route": "r1"}));
    let none = idle.exec(json!({"route_expect_refusals": {"route": "r1", "count": 1}}));
    assert!(
        none.as_ref()
            .is_err_and(|e| e.contains("Inconclusive") && e.contains("route_idle")),
        "{none:?}"
    );
}

#[test]
fn a_duration_step_argument_takes_milliseconds_or_the_exact_nanosecond_form() {
    // advance_clock: 5.5 s as nanoseconds moves the injected clock as 5500 ms does (the unix time is the start plus whole seconds).
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"poll_all": {}}));
    run.ok(json!({"advance_clock": {"ms": {"ns": 5_500_000_000u64}}}));
    run.ok(json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "78"}}));
    run.ok(json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output", "at": 1000005}}}}));
    // A form that is neither is zero, as a missing value is.
    run.ok(json!({"poll_all": {}}));
    run.ok(json!({"advance_clock": {"ms": {"seconds": 9}}}));
    run.ok(json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "78"}}));
    run.ok(json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output", "at": 1000005}}}}));
    // real_wait: the interval is real time, and 3 ms in nanoseconds lasts as long as 3 ms.
    let before = Instant::now();
    run.ok(json!({"real_wait": {"ms": {"ns": 3_000_000}}}));
    assert!(
        before.elapsed() >= Duration::from_millis(3),
        "{:?}",
        before.elapsed()
    );
    assert!(
        before.elapsed() < Duration::from_secs(2),
        "{:?}",
        before.elapsed()
    );
    // a real_wait with no usable interval is a malformed step
    run.err(json!({"real_wait": {"ms": {"seconds": 1}}}));
}

#[test]
fn route_expect_refusals_is_inconclusive_when_the_step_deadline_passes_and_ok_for_no_refusals() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    run.ok(json!({"route_drain": true, "route": "r1"}));
    // A deadline that has passed while the route is idle is the runner's limit: inconclusive, never a failure of the worker.
    let late = Deadline::after(Some(Duration::ZERO));
    let timed = run.exec_within(
        json!({"route_expect_refusals": {"route": "r1", "count": 1}}),
        &late,
    );
    assert!(
        timed
            .as_ref()
            .is_err_and(|e| e.contains("Inconclusive") && e.contains("step_timeout")),
        "{timed:?}"
    );
    // No refusal is asked for: nothing is read.
    run.ok(json!({"route_expect_refusals": {"route": "r1", "count": 0}}));
}
