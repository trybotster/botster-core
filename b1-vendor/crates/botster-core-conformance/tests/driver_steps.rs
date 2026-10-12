//! The steps of the Core driver, one at a time, against FakeCore and a counting wrapper of it (design 5.5, 6.2): each relation, binding and
//! wait does what its documentation says. A step that did nothing would pass every transcript that used it, so these tests carry the
//! mutation job (BUILD.md, Testing).

use botster_conformance::{Bindings, Deadline, StepDriver};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    driver_for, ControlError, CoreDriver, CoreHarness, CoreSchemas, DataDirRef, OpenSpec,
    RouteClient, WorkerBuild, WorkerRef,
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
    /// Events that the next poll returns after Core's own (a host event that the test scripts).
    inject: Arc<Mutex<Vec<Event>>>,
    /// `terminal_state` shows the title "sentinel" once the pump count is above this number (R-57's `await_title`). `None`: the
    /// inner Core's title.
    title_after_pumps: Option<Arc<AtomicUsize>>,
    /// The calls of `poll_events`.
    polls: Arc<AtomicUsize>,
    /// Every pump whose count is a multiple of this number reports runnable work too (0: none).
    more_every: usize,
    /// Every pump whose count is a multiple of this number reports one posted event (0: none).
    posted_every: usize,
    /// See `Harness::deadline_ahead`.
    deadline_ahead: bool,
    /// The calls of `next_deadline` while `deadline_ahead` is set.
    deadline_calls: Arc<AtomicUsize>,
    /// See `Harness::always_woken`.
    always_woken: bool,
}

/// The wake handle of the wrapper counts the waits that the fences make on it.
struct CountingWake {
    inner: Arc<dyn WakeHandle>,
    waits: Arc<AtomicUsize>,
    blocking: bool,
    /// Every wait returns `Woken` at once, with no work behind it (a spurious wake).
    woken: bool,
}

impl WakeHandle for CountingWake {
    fn wait(&self, timeout: Duration) -> Wake {
        self.waits.fetch_add(1, Ordering::SeqCst);
        if self.woken {
            return Wake::Woken;
        }
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
        report.more = n <= 3 || (self.more_every != 0 && n.is_multiple_of(self.more_every));
        report.events_posted =
            u32::from(self.posted_every != 0 && n.is_multiple_of(self.posted_every));
        report
    }
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.polls.fetch_add(1, Ordering::SeqCst);
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
        events.append(&mut self.inject.lock().unwrap());
        events
    }
    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        Arc::new(CountingWake {
            inner: self.inner.wake_handle(),
            waits: self.waits.clone(),
            blocking: self.blocking,
            woken: self.always_woken,
        })
    }
    fn next_deadline(&self) -> Option<Instant> {
        if self.deadline_ahead {
            // Each call names a deadline one hour after the one before, so it is always ahead of the jumped clock.
            let n = self.deadline_calls.fetch_add(1, Ordering::SeqCst) as u32 + 1;
            return Some(Instant::now() + Duration::from_secs(3600) * n);
        }
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
        let mut state = self.inner.terminal_state(id)?;
        if let Some(after) = &self.title_after_pumps {
            if self.pumps.load(Ordering::SeqCst) > after.load(Ordering::SeqCst) {
                state.title = Some("sentinel".into());
            }
        }
        Ok(state)
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
    /// Once set, the stream ends, and the end names a cause of its own (`handoff_failed`) that the driver must not read (R-58).
    eof: Arc<std::sync::atomic::AtomicBool>,
    /// See `Harness::stream_room`.
    room: Option<usize>,
    /// The bytes that the client wrote and that wait for room (R-86).
    unsent: Vec<u8>,
    /// See `Harness::client_busy`.
    busy: bool,
}

impl Tagged {
    /// Gives the inner end the waiting bytes that the stream has room for now. True when it gave a byte.
    fn flush(&mut self) -> bool {
        let n = self.room.unwrap_or(usize::MAX).min(self.unsent.len());
        if n == 0 {
            return false;
        }
        let now: Vec<u8> = self.unsent.drain(..n).collect();
        self.inner.write(&now);
        true
    }
}

impl RouteClient for Tagged {
    fn write(&mut self, bytes: &[u8]) {
        self.unsent.extend_from_slice(bytes);
        self.flush();
    }
    fn progress(&mut self) -> bool {
        let inner = self.inner.progress();
        self.flush() | inner | self.busy
    }
    fn read(
        &mut self,
        max: usize,
        deadline: &Deadline,
    ) -> botster_hub_conformance::route::RouteRead {
        if self.eof.load(Ordering::SeqCst) {
            return botster_hub_conformance::route::RouteRead::Eof {
                ended: Some("handoff_failed".into()),
            };
        }
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
    inject: Arc<Mutex<Vec<Event>>>,
    eof: Arc<std::sync::atomic::AtomicBool>,
    /// See `Counting::title_after_pumps`.
    title_after_pumps: Option<Arc<AtomicUsize>>,
    polls: Arc<AtomicUsize>,
    /// See `Counting::more_every`.
    more_every: usize,
    /// See `Counting::posted_every`.
    posted_every: usize,
    /// The bytes that a route's stream takes from its client per write or per `progress` call (R-86). `None`: all of them.
    stream_room: Option<usize>,
    /// Every route's client reports progress on every call, as a client that never runs out of work does (R-89).
    client_busy: bool,
    /// Core always names a next deadline ahead of the clock, so an idle wait under a jumping clock jumps on every pass (R-89).
    deadline_ahead: bool,
    /// The wake handle always returns `Woken`, with no work behind it (R-89).
    always_woken: bool,
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
            inject: Arc::default(),
            eof: Arc::default(),
            title_after_pumps: None,
            polls: Arc::default(),
            more_every: 0,
            posted_every: 0,
            stream_room: None,
            client_busy: false,
            deadline_ahead: false,
            always_woken: false,
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
                inject: self.inject.clone(),
                title_after_pumps: self.title_after_pumps.clone(),
                polls: self.polls.clone(),
                more_every: self.more_every,
                posted_every: self.posted_every,
                deadline_ahead: self.deadline_ahead,
                deadline_calls: Arc::default(),
                always_woken: self.always_woken,
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
            eof: self.eof.clone(),
            room: self.stream_room,
            unsent: Vec::new(),
            busy: self.client_busy,
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

const A20_ROW: &str = "| A20-1 | **A proof.**<br>- On the real tier.<br>- The class has **one member**: `conf::dp_3_frame_limit_checked_before_allocation`. A new member needs a Core amendment that names it.<br>- The real tier still runs `conf::dp_3_other`. | `conf::a20_1_x` |";

#[test]
fn a20_1_testkit_proven_is_exactly_the_named_member_of_the_frozen_text() {
    let f = botster_core_conformance::a20_1_testkit_proven;
    f(A20_ROW, botster_core_conformance::TESTKIT_PROVEN).unwrap();
    let repo = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../frozen/current/core-contract-v1.17-amendment-20-candidate1.md"
    ))
    .unwrap();
    f(&repo, botster_core_conformance::TESTKIT_PROVEN).unwrap();
    assert_eq!(
        botster_core_conformance::TESTKIT_PROVEN,
        ["conf::dp_3_frame_limit_checked_before_allocation"]
    );
}

#[test]
fn a20_1_testkit_proven_rejects_an_extra_a_missing_and_a_duplicate_id() {
    let f = botster_core_conformance::a20_1_testkit_proven;
    let named = "conf::dp_3_frame_limit_checked_before_allocation";
    // Another DP-3 id of the row is not a member: only the "class has" item names members.
    assert!(f(A20_ROW, &[named, "conf::dp_3_other"]).is_err());
    assert!(f(A20_ROW, &[]).is_err());
    assert!(f(A20_ROW, &[named, named]).is_err());
    assert!(f(A20_ROW, &["conf::dp_3_other"]).is_err());
    // A text with no A20-1 row, or with no member item, proves nothing.
    assert!(f("| A19-1 | x |", &[named]).is_err());
    assert!(f(&A20_ROW.replace("The class has", "The set has"), &[named]).is_err());
}

#[test]
fn a20_1_testkit_proven_accepts_a_class_of_two_members_in_any_order() {
    let f = botster_core_conformance::a20_1_testkit_proven;
    let (a, b) = (
        "conf::dp_3_frame_limit_checked_before_allocation",
        "conf::dp_3_other",
    );
    let two = A20_ROW.replace("`. A new member", &format!("` and `{b}`. A new member"));
    f(&two, &[a, b]).unwrap();
    f(&two, &[b, a]).unwrap();
    assert!(f(&two, &[a]).is_err());
}

#[test]
fn a20_1_a_pending_or_withdrawn_id_has_no_testkit_proof() {
    let f = botster_core_conformance::a20_1_proven_ids_are_active;
    let id = "conf::dp_3_frame_limit_checked_before_allocation";
    f(&[id], "# c\nconf::other\n", "# c\n").unwrap();
    assert!(f(&[id], &format!("# c\n{id}\n"), "").is_err());
    assert!(f(
        &[id],
        "",
        &format!("{id}  withdrawn by Core A20-1  replaced by none\n")
    )
    .is_err());
    // A comment that names the id is not an entry.
    f(&[id], &format!("# {id}\n"), "").unwrap();
}

#[test]
fn the_a20_1_type_check_runs_against_the_repository_files() {
    let mut run = Run::new(Harness::new(None));
    run.ok(json!({"type_check": "a20_1_testkit_proven", "because": "Core A20-1"}));
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

// Steward ruling R-86: the client is an independent actor. A send larger than the stream's room leaves client bytes that
// wait; each wait loop moves them as room appears, so `await_pty` gets the full count. Without `progress()` in the wait
// loops, only the first write's bytes reach the worker, the frame never completes, and the Core is idle.
#[test]
fn a_wait_loop_moves_the_clients_waiting_bytes_until_the_pty_has_them_all() {
    let hex = "61".repeat(4096);
    let t: botster_conformance::Transcript = serde_json::from_value(json!({
        "id": "conf::r86_self_test", "contract": "core", "clause": "DP-5", "subject": "core",
        "steps": [
            create("s1"),
            {"begin": {"op": {"Start": {"id": "s1"}}}, "bind": "$s"},
            {"pump_until": {"event": {"SessionState": {"id": "s1", "state": "Running"}}}},
            {"control": {"op": "pty_blocked", "session": "s1", "on": false}},
            {"attach_route": {"route": "r1", "session": "s1", "client": "c1"}},
            {"expect_eventually": {"frame": "live"}, "ignore": [{"$any": true}], "on": "r1"},
            {"pump_idle": {}},
            {"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": hex}}},
            // The `pump` step is no wait loop: it reports Core's own pump, though client bytes wait.
            {"pump": {}, "expect": {"more": false, "events_posted": 0}},
            {"await_pty": {"session": "s1", "min_bytes": 4096}}
        ]
    }))
    .unwrap();
    let make = |seed: u64| {
        let mut harness = Harness::new(None);
        harness.inner = FakeCoreHarness::new(seed);
        harness.stream_room = Some(16);
        driver_for(Box::new(harness))
    };
    assert_eq!(
        botster_conformance::run_transcript(
            &t,
            &make,
            &botster_conformance::SeedSet::parse("0").unwrap(),
            &CoreSchemas,
            &botster_conformance::Limits::default(),
        ),
        botster_conformance::Outcome::Passed
    );
}

// R-86 in a route wait (R-68): an unknown frame after a send larger than the stream's room closes the route only after the
// worker read the whole send. The route wait moves the client's waiting bytes, so the close comes; without `progress()` in
// the route wait, the unknown frame never reaches the worker.
#[test]
fn a_route_wait_moves_the_clients_waiting_bytes_until_the_route_closes() {
    let hex = "61".repeat(4096);
    let t: botster_conformance::Transcript = serde_json::from_value(json!({
        "id": "conf::r86_route_wait_self_test", "contract": "core", "clause": "DP-5", "subject": "core",
        "steps": [
            create("s1"),
            {"begin": {"op": {"Start": {"id": "s1"}}}, "bind": "$s"},
            {"pump_until": {"event": {"SessionState": {"id": "s1", "state": "Running"}}}},
            {"control": {"op": "pty_blocked", "session": "s1", "on": false}},
            {"attach_route": {"route": "r1", "session": "s1", "client": "c1"}},
            {"expect_eventually": {"frame": "live"}, "ignore": [{"$any": true}], "on": "r1"},
            {"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": hex}}},
            {"route_send_raw": {"bytes_hex": "9000"}, "route": "r1"},
            {"expect_eventually": {"frame": "route_closed", "reason": {"protocol_error": {"code": "unknown_frame"}}},
             "ignore": [{"$any": true}], "on": "r1"}
        ]
    }))
    .unwrap();
    let make = |seed: u64| {
        let mut harness = Harness::new(None);
        harness.inner = FakeCoreHarness::new(seed);
        harness.stream_room = Some(16);
        driver_for(Box::new(harness))
    };
    assert_eq!(
        botster_conformance::run_transcript(
            &t,
            &make,
            &botster_conformance::SeedSet::parse("0").unwrap(),
            &CoreSchemas,
            &botster_conformance::Limits::default(),
        ),
        botster_conformance::Outcome::Passed
    );
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

/// R-58: a run with one route `r1` on a running session, whose host events and end of stream the test scripts.
struct Ending {
    run: Run,
    inject: Arc<Mutex<Vec<Event>>>,
    eof: Arc<std::sync::atomic::AtomicBool>,
}

impl Ending {
    fn new() -> Self {
        let harness = Harness::new(Some(Arc::default()));
        let inject = harness.inject.clone();
        let eof = harness.eof.clone();
        let mut run = Run::new(harness);
        running(&mut run, "s1");
        run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
        Ending { run, inject, eof }
    }

    fn route(&self) -> RouteId {
        RouteId(self.run.bindings["r1"].as_u64().unwrap())
    }

    /// Core posts `RouteClosed{r1, reason}`, and a `pump_until` takes it, as the transcripts do before `expect_closed`.
    fn host_closes(&mut self, reason: Value) {
        let reason: RouteCloseReason = serde_json::from_value(reason).unwrap();
        let event = Event::RouteClosed {
            route: self.route(),
            reason,
            route_tag: None,
        };
        let matcher = serde_json::to_value(&event).unwrap();
        self.inject.lock().unwrap().push(event);
        self.run.ok(json!({"pump_until": {"event": matcher}}));
    }

    fn stream_ends(&self) {
        self.eof.store(true, Ordering::SeqCst);
    }

    /// The step through the order runner, as a transcript runs it.
    fn expect_closed(&mut self, matcher: Value) -> Result<(), String> {
        let step = json!({"expect_closed": matcher, "on": "r1"});
        botster_conformance::run_step(
            &step,
            &mut self.run.driver,
            &mut self.run.bindings,
            &botster_conformance::NoSchemas,
            &botster_conformance::Limits::default(),
        )
        .map_err(|e| format!("{e:?}"))
    }
}

/// The failed-route reasons of OU-2b that Core's `RouteCloseReason` has, with their `route_ended` reasons.
const CORE_FAILED: &[(&str, &str)] = &[
    ("HandoffFailed", "handoff_failed"),
    ("PeerClosed", "transport_lost"),
    ("WriteFailed", "write_failed"),
    ("StallTimeout", "stalled"),
    ("SessionLost", "session_lost"),
];

#[test]
fn expect_closed_ended_passes_on_the_end_of_the_stream_and_the_hosts_reason() {
    for (reason, ended) in CORE_FAILED {
        let mut e = Ending::new();
        e.host_closes(json!(reason));
        e.stream_ends();
        e.expect_closed(json!({"ended": ended}))
            .unwrap_or_else(|f| panic!("{reason}: {f}"));
    }
}

#[test]
fn expect_closed_ended_fails_when_the_host_reason_maps_to_another_ended() {
    for (reason, ended) in CORE_FAILED {
        // Each reason against the `ended` of the next one in the table: a failed reason, but not this one.
        let other = CORE_FAILED
            .iter()
            .map(|(_, e)| *e)
            .find(|e| e != ended)
            .unwrap();
        let mut e = Ending::new();
        e.host_closes(json!(reason));
        e.stream_ends();
        let f = e.expect_closed(json!({"ended": other})).unwrap_err();
        assert!(f.contains(ended) && f.contains(reason), "{reason}: {f}");
    }
}

#[test]
fn expect_closed_ended_fails_on_a_healthy_reason_and_names_both_reasons() {
    for ended in ["handoff_failed", "session_lost"] {
        let mut e = Ending::new();
        e.host_closes(json!("Detached"));
        e.stream_ends();
        let f = e.expect_closed(json!({"ended": ended})).unwrap_err();
        assert!(f.contains("Detached") && f.contains(ended), "{f}");
    }
}

#[test]
fn expect_closed_ended_fails_on_an_end_of_stream_with_no_host_event() {
    for (_, ended) in CORE_FAILED {
        let mut e = Ending::new();
        // The route end names a cause itself (`handoff_failed`): the driver never reads it.
        e.stream_ends();
        let f = e.expect_closed(json!({"ended": ended})).unwrap_err();
        assert!(f.contains("host_reason"), "{ended}: {f}");
    }
}

#[test]
fn expect_closed_ended_fails_when_the_stream_does_not_end() {
    for (reason, ended) in CORE_FAILED {
        let mut e = Ending::new();
        e.host_closes(json!(reason));
        assert!(
            e.expect_closed(json!({"ended": ended})).is_err(),
            "{reason}: the host's event alone is no end of the stream"
        );
    }
}

#[test]
fn the_host_record_keeps_the_first_reason_of_a_route_and_reads_events_that_a_step_took() {
    let mut e = Ending::new();
    e.host_closes(json!("WriteFailed"));
    // A second report for the same route does not replace the first (A2-3: the first reason wins).
    e.host_closes(json!("SessionLost"));
    e.stream_ends();
    e.expect_closed(json!({"ended": "write_failed"})).unwrap();
}

#[test]
fn a_null_expect_closed_needs_only_the_end_of_the_stream() {
    let mut e = Ending::new();
    e.stream_ends();
    e.expect_closed(Value::Null).unwrap();
}

/// Every name of `RouteCloseReason`, from the type's generated schema (`cargo xtask codegen`: the names that the lenient form
/// excludes from `Other`). `RouteCloseReason` is `#[non_exhaustive]`, so a match outside its crate cannot list them.
fn every_reason_name() -> Vec<String> {
    let schema: Value =
        serde_json::from_str(include_str!("../../../schemas/core/event.schema.json")).unwrap();
    schema["$defs"]["RouteCloseReason"]["anyOf"][1]["not"]["enum"]
        .as_array()
        .expect("the variant names of RouteCloseReason")
        .iter()
        .map(|n| n.as_str().unwrap().to_string())
        .collect()
}

/// The JSON form of each variant, as an event carries it.
fn reason_form(name: &str) -> Value {
    let form = match name {
        "SessionEnded" => json!({"SessionEnded": {"exit": {"code": 0, "cause": "Normal"}}}),
        "BadFrame" => json!({"BadFrame": {"code": "malformed_frame"}}),
        _ => json!(name),
    };
    let reason: RouteCloseReason = serde_json::from_value(form).unwrap();
    serde_json::to_value(reason).unwrap()
}

/// OU-2b's reasons that close a healthy route with `route_closed`.
const HEALTHY: &[&str] = &[
    "Detached",
    "Replaced",
    "Revoked",
    "SessionEnded",
    "SessionRemoved",
    "BadFrame",
    "SnapshotTooLarge",
];

#[test]
fn route_ended_maps_each_failed_reason_no_healthy_reason_and_an_unknown_name_as_peer_closed() {
    use botster_core_conformance::route_ended;
    for (reason, ended) in CORE_FAILED {
        assert_eq!(route_ended(&json!(reason)), Some(*ended));
    }
    for reason in HEALTHY {
        assert_eq!(route_ended(&json!(reason)), None, "{reason}");
    }
    assert_eq!(
        route_ended(&json!({"SessionEnded": {"exit": {"code": 0}}})),
        None
    );
    assert_eq!(
        route_ended(&json!({"BadFrame": {"code": "unknown_frame"}})),
        None
    );
    // Core A2-3: a host maps an unknown value to `PeerClosed`. Core A17 removed `NotConnected` and `BadPeer`, so they are
    // unknown to Core's host too, and never `not_connected` (steward, R-58 correction).
    for unknown in ["Other", "NotAReason", "NotConnected", "BadPeer"] {
        assert_eq!(
            route_ended(&json!(unknown)),
            Some("transport_lost"),
            "{unknown}"
        );
    }
    assert_eq!(route_ended(&Value::Null), None);
    // The object form names the reason by its one key, whatever its fields; an object with two keys names no reason.
    assert_eq!(
        route_ended(&json!({"HandoffFailed": {}})),
        Some("handoff_failed")
    );
    assert_eq!(
        route_ended(&json!({"HandoffFailed": {}, "SessionLost": {}})),
        None
    );
}

/// The lead's precision on R-58: the table is checked against every variant of `RouteCloseReason`. Each name must have a
/// class here (a failed reason, a healthy one, or `Other`, A2-3's unknown value), so a new variant fails this test and not the
/// transcripts.
#[test]
fn route_ended_is_checked_against_every_route_close_reason_variant() {
    let names = every_reason_name();
    assert_eq!(
        names.len(),
        CORE_FAILED.len() + HEALTHY.len() + 1,
        "{names:?}"
    );
    for name in &names {
        let expected = match CORE_FAILED.iter().find(|(core, _)| core == name) {
            Some((_, ended)) => Some(*ended),
            None if HEALTHY.contains(&name.as_str()) => None,
            None if name == "Other" => Some("transport_lost"),
            None => panic!("RouteCloseReason::{name} has no OU-2b class in this test"),
        };
        let form = reason_form(name);
        assert_eq!(
            botster_core_conformance::route_ended(&form),
            expected,
            "{form}"
        );
    }
}

/// Core A2-3 through the whole driver: a reason that the typed reader does not know arrives as `Other`, and the host maps it to
/// `PeerClosed`, so the route ended `transport_lost`.
#[test]
fn expect_closed_ended_maps_an_unknown_reason_to_transport_lost() {
    let mut e = Ending::new();
    e.host_closes(json!("FutureCloseReason"));
    e.stream_ends();
    e.expect_closed(json!({"ended": "transport_lost"})).unwrap();
}

/// Steward's R-58 correction: no Core reason maps to `not_connected` (Core A17), so `expect_closed {ended: not_connected}` fails
/// for every one.
#[test]
fn expect_closed_ended_not_connected_fails_for_every_core_reason() {
    for name in every_reason_name() {
        let mut e = Ending::new();
        e.host_closes(reason_form(&name));
        e.stream_ends();
        assert!(
            e.expect_closed(json!({"ended": "not_connected"})).is_err(),
            "{name}"
        );
    }
}

/// Core ID-1, ID-2: a `RouteId` is unique only for the lifetime of the `Core` handle. A handle that is dropped and opened again
/// under the same name can mint the same number, and the record of the old route must not answer for the new one.
#[test]
fn a_route_of_a_reopened_handle_never_reads_the_record_of_an_old_route_with_the_same_id() {
    let mut e = Ending::new();
    let old = e.route();
    e.host_closes(json!("WriteFailed"));
    e.run.ok(json!({"drop": {"handle": "h"}}));
    e.run.ok(json!({"open": {"as": "h", "data_dir": "d2"}}));
    running(&mut e.run, "s1");
    e.run
        .ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    assert_eq!(e.route(), old, "the new handle mints the same RouteId");
    e.stream_ends();
    let f = e
        .expect_closed(json!({"ended": "write_failed"}))
        .unwrap_err();
    assert!(f.contains(r#""host_reason": Null"#), "{f}");
}

// Steward ruling R-57: `await_title` pumps until the cached title equals the sentinel. It reads the title after each pump, and it never
// polls, so the queued events stay for the later steps.
#[test]
fn await_title_pumps_without_polling_until_the_title_shows() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let after = Arc::new(AtomicUsize::new(usize::MAX));
    let mut harness = Harness::new(Some(pumps.clone()));
    harness.title_after_pumps = Some(after.clone());
    let polls = harness.polls.clone();
    let waits = harness.waits.clone();
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    // The step's pumps are the third to the sixth: the third reports more work (`Counting::pump`), the later ones none, and the
    // sixth shows the title.
    assert_eq!(pumps.load(Ordering::SeqCst), 2);
    after.store(5, Ordering::SeqCst);
    let polled = polls.load(Ordering::SeqCst);
    let waited = waits.load(Ordering::SeqCst);
    run.ok(json!({"await_title": {"session": "s1", "title": "sentinel"}}));
    assert_eq!(pumps.load(Ordering::SeqCst), 6);
    assert_eq!(polls.load(Ordering::SeqCst), polled, "the step polled");
    // It waits on the wake handle only after a pump that found no title and nothing more to run: the fourth and the fifth.
    assert_eq!(waits.load(Ordering::SeqCst), waited + 2);
    // A title that shows before the step ends it at once, with no pump.
    let pumped = pumps.load(Ordering::SeqCst);
    run.ok(json!({"await_title": {"session": "s1", "title": "sentinel"}}));
    assert_eq!(pumps.load(Ordering::SeqCst), pumped);
    run.err(json!({"await_title": {"title": "sentinel"}}));
    run.err(json!({"await_title": {"session": "s1"}}));
}

// At the step limit, `await_title` fails (it is not inconclusive) and names the last title that it read.
#[test]
fn await_title_fails_at_the_step_limit_with_the_last_title() {
    let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
    harness.title_after_pumps = Some(Arc::new(AtomicUsize::new(0)));
    let mut run = Run::new(harness);
    running(&mut run, "s1");
    let step = json!({"await_title": {"session": "s1", "title": "never"}});
    let err = run
        .driver
        .exec(
            &step,
            &mut run.bindings,
            &Deadline::after(Some(Duration::from_millis(50))),
        )
        .unwrap_err();
    let text = format!("{err:?}");
    assert!(
        matches!(err, botster_conformance::StepError::Failed { .. }),
        "{text}"
    );
    assert!(text.contains("sentinel"), "{text}");
}

/// A route end that never gives a byte, with a readiness descriptor that is readable or not as the test sets it.
#[cfg(unix)]
struct Silent {
    readiness: std::os::unix::net::UnixStream,
    /// False: the end gives no readiness descriptor (an in-memory end).
    has_readiness: bool,
    /// The time left of the deadline of each read.
    reads: Arc<Mutex<Vec<Option<Duration>>>>,
    /// Every read whose count is a multiple of this number gives one byte of a frame that never completes (0: none).
    dribble: usize,
    /// Every read after this many reads gives the end of the stream (0: never).
    eof_after: usize,
    /// True: a read with nothing to give is `Blocked` (a stalled reader), not `Empty`.
    blocked: bool,
}

/// The bytes that a dribbling `Silent` end gives, one per read: the head of a 60 000-byte frame (`[u32 length][type]`),
/// then its zero body. The frame needs far more reads than a test makes, so it never completes.
#[cfg(unix)]
fn dribbled(n: usize) -> u8 {
    const HEAD: [u8; 5] = [0, 0, 0xEA, 0x60, 0x01];
    HEAD.get(n).copied().unwrap_or(0)
}

#[cfg(unix)]
impl RouteClient for Silent {
    fn write(&mut self, _bytes: &[u8]) {}
    fn progress(&mut self) -> bool {
        false
    }
    fn read(
        &mut self,
        _max: usize,
        deadline: &Deadline,
    ) -> botster_hub_conformance::route::RouteRead {
        let mut reads = self.reads.lock().unwrap();
        reads.push(deadline.remaining());
        let n = reads.len();
        if self.eof_after != 0 && n > self.eof_after {
            return botster_hub_conformance::route::RouteRead::Eof { ended: None };
        }
        if self.dribble != 0 && n.is_multiple_of(self.dribble) {
            return botster_hub_conformance::route::RouteRead::Bytes(vec![dribbled(
                n / self.dribble - 1,
            )]);
        }
        if self.blocked {
            return botster_hub_conformance::route::RouteRead::Blocked;
        }
        botster_hub_conformance::route::RouteRead::Empty
    }
    fn control(&mut self, op: &str, _args: &Value) -> Result<Value, String> {
        Err(format!("no control {op}"))
    }
    fn has_control(&self, _op: &str) -> bool {
        false
    }
    fn readiness(&self) -> Option<std::os::fd::BorrowedFd<'_>> {
        use std::os::fd::AsFd;
        self.has_readiness.then(|| self.readiness.as_fd())
    }
}

/// A harness whose wake descriptor and route readiness descriptors are the read ends of socket pairs. A byte written to the
/// other end makes a descriptor readable, and nothing reads it, so it stays readable (steward ruling R-68, follow-up 3).
#[cfg(unix)]
struct Descriptors {
    inner: Harness,
    wake: (
        std::os::unix::net::UnixStream,
        std::os::unix::net::UnixStream,
    ),
    route_readable: bool,
    /// The write ends of the route pairs, kept open so that a readiness descriptor is not readable by its end of file.
    writers: Vec<std::os::unix::net::UnixStream>,
    /// See `Silent`.
    has_readiness: bool,
    reads: Arc<Mutex<Vec<Option<Duration>>>>,
    dribble: usize,
    eof_after: usize,
    blocked: bool,
}

#[cfg(unix)]
impl Descriptors {
    fn new(wake_readable: bool, route_readable: bool) -> Self {
        Self::with(Harness::new(None), wake_readable, route_readable)
    }

    fn with(inner: Harness, wake_readable: bool, route_readable: bool) -> Self {
        use std::io::Write;
        let (mut writer, reader) = std::os::unix::net::UnixStream::pair().unwrap();
        if wake_readable {
            writer.write_all(b"w").unwrap();
        }
        Descriptors {
            inner,
            wake: (writer, reader),
            route_readable,
            writers: Vec::new(),
            has_readiness: true,
            reads: Arc::default(),
            dribble: 0,
            eof_after: 0,
            blocked: false,
        }
    }
}

#[cfg(unix)]
impl CoreHarness for Descriptors {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        self.inner.open(spec)
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
    fn wake_fd(&self, _handle: &str) -> Option<std::os::fd::OwnedFd> {
        use std::os::fd::AsFd;
        Some(self.wake.1.as_fd().try_clone_to_owned().unwrap())
    }
    fn injects_clock(&self) -> bool {
        false
    }
    fn has_control(&self, op: &str) -> bool {
        self.inner.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
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
        use std::io::Write;
        let (result, _end) = self
            .inner
            .attach_stream(handle, core, client, session, options)?;
        let (mut writer, readiness) = std::os::unix::net::UnixStream::pair().unwrap();
        if self.route_readable {
            writer.write_all(b"r").unwrap();
        }
        self.writers.push(writer);
        Ok((
            result,
            Box::new(Silent {
                readiness,
                has_readiness: self.has_readiness,
                reads: self.reads.clone(),
                dribble: self.dribble,
                eof_after: self.eof_after,
                blocked: self.blocked,
            }),
        ))
    }
}

/// `expect_closed` on the route of a `Descriptors` harness, under a real step limit. The time it took, and its result.
#[cfg(unix)]
fn route_wait(harness: Descriptors, step_timeout: Duration) -> (CoreDriver, Duration, String) {
    let mut run = Run {
        driver: CoreDriver::new(Box::new(harness)),
        bindings: Bindings::new(),
    };
    run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
    running(&mut run, "s1");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    let started = Instant::now();
    let result = botster_conformance::run_step(
        &json!({"expect_closed": {"$any": true}, "on": "r1"}),
        &mut run.driver,
        &mut run.bindings,
        &botster_conformance::NoSchemas,
        &botster_conformance::Limits {
            max_polls: usize::MAX,
            step_timeout: Some(step_timeout),
        },
    );
    let elapsed = started.elapsed();
    let text = format!("{:?}", result.unwrap_err());
    (run.driver, elapsed, text)
}

/// Steward ruling R-68, follow-up 3: a descriptor that stays readable while nothing makes progress ends the route wait
/// inconclusive at the no-spin bound, long before the step limit, with one line that names the spin, the readable
/// descriptors, the handle and the route.
#[test]
#[cfg(unix)]
fn a_descriptor_that_stays_readable_ends_the_route_wait_inconclusive_with_one_line_before_the_step_limit(
) {
    for (wake, route, readable) in [
        (true, true, "the route readiness and the wake descriptors"),
        (true, false, "the wake descriptor"),
        (false, true, "the route readiness descriptor"),
    ] {
        // timer: deadline — the step limit that the no-spin bound must come long before
        let limit = Duration::from_secs(10);
        let (driver, elapsed, text) = route_wait(Descriptors::new(wake, route), limit);
        assert!(
            text.contains("Inconclusive") && text.contains("step_timeout"),
            "{readable}: {text}"
        );
        assert!(elapsed < limit / 2, "{readable}: {elapsed:?}");
        let lines = driver.diagnostics();
        assert_eq!(lines.len(), 1, "{readable}: {lines:?}");
        let line = &lines[0];
        for part in [
            "route wait spin",
            "route r1",
            "handle h",
            &format!("{readable} stayed readable for 1000 passes"),
        ] {
            assert!(line.contains(part), "{readable}: {part} not in {line}");
        }
    }
}

/// With both descriptors present and neither readable, the route wait blocks on them until the step limit: it does not
/// spin, and it leaves no diagnostic line.
#[test]
#[cfg(unix)]
fn a_route_wait_with_no_readable_descriptor_blocks_until_the_step_limit_without_a_line() {
    // timer: deadline — the step limit that ends the blocked wait
    let limit = Duration::from_millis(300);
    let (driver, elapsed, text) = route_wait(Descriptors::new(false, false), limit);
    assert!(
        text.contains("Inconclusive") && text.contains("step_timeout"),
        "{text}"
    );
    assert!(elapsed >= limit, "{elapsed:?}");
    assert!(
        driver.diagnostics().is_empty(),
        "{:?}",
        driver.diagnostics()
    );
}

/// Progress resets the no-spin count: a pump that reports runnable work, or a posted event, at every 500th pass keeps the
/// wait below the bound, so the step limit ends it, with no diagnostic line.
#[test]
#[cfg(unix)]
fn progress_between_readable_passes_resets_the_no_spin_count() {
    for posted in [false, true] {
        let mut inner = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
        if posted {
            inner.posted_every = 500;
        } else {
            inner.more_every = 500;
        }
        // timer: deadline — the step limit that ends the wait that the progress keeps below the bound
        let limit = Duration::from_millis(300);
        let (driver, elapsed, text) = route_wait(Descriptors::with(inner, true, true), limit);
        assert!(
            text.contains("Inconclusive") && text.contains("step_timeout"),
            "{posted}: {text}"
        );
        assert!(elapsed >= limit, "{posted}: {elapsed:?}");
        assert!(
            driver.diagnostics().is_empty(),
            "{posted}: {:?}",
            driver.diagnostics()
        );
    }
}

/// The reads of a route wait take a deadline that has passed: one attempt that does not wait, because the driver waits on
/// both descriptors itself (steward ruling R-68). An end with no readiness descriptor gets the step deadline, as before.
#[test]
#[cfg(unix)]
fn the_reads_of_a_route_wait_do_not_wait_and_an_end_with_no_descriptor_waits_to_the_step_deadline()
{
    for has_readiness in [true, false] {
        let mut harness = Descriptors::new(false, false);
        harness.has_readiness = has_readiness;
        let reads = harness.reads.clone();
        // timer: deadline — the step limit that ends the wait
        let limit = Duration::from_millis(300);
        let (_driver, _elapsed, text) = route_wait(harness, limit);
        // With both descriptors the wait ends at the step limit. With none, an idle read after a pump that made no
        // progress is the end of what the route gives, as before R-68.
        let ended = if has_readiness {
            "Inconclusive"
        } else {
            "no close"
        };
        assert!(text.contains(ended), "{has_readiness}: {text}");
        let reads = reads.lock().unwrap().clone();
        assert!(!reads.is_empty(), "{has_readiness}");
        let passed = reads
            .iter()
            .filter(|left| **left == Some(Duration::ZERO))
            .count();
        if has_readiness {
            assert_eq!(passed, reads.len(), "every read is one attempt: {reads:?}");
        } else {
            assert!(
                reads
                    .iter()
                    .any(|left| left.is_some_and(|l| l > Duration::ZERO)),
                "a read gets the time left of the step: {reads:?}"
            );
        }
    }
}

/// `route_expect_refusals` keeps its own read path (steward ruling R-68, condition 3): the first idle read of a route pumps
/// its handle once (the handoff, Core OU-9), and the step then pumps once more before a second idle read in a row ends it.
/// A later step on the same route has had its handoff pump: it pumps once.
#[test]
fn route_expect_refusals_pumps_once_for_the_handoff_of_a_route_and_once_before_it_ends() {
    let pumps = Arc::new(AtomicUsize::new(0));
    let mut run = Run::new(Harness::new(Some(pumps.clone())));
    running(&mut run, "s1");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    // The fake's worker writes its first frames in the pump that hands the route off: they are drained first, so the
    // step's reads find nothing.
    run.ok(json!({"pump": {}}));
    run.ok(json!({"route_drain": true, "route": "r1"}));
    for (step, expected) in [("first", 2), ("later", 1)] {
        let before = pumps.load(Ordering::SeqCst);
        let none = run.exec(json!({"route_expect_refusals": {"route": "r1", "count": 1}}));
        assert!(
            none.as_ref()
                .is_err_and(|e| e.contains("Inconclusive") && e.contains("route_idle")),
            "{step}: {none:?}"
        );
        assert_eq!(pumps.load(Ordering::SeqCst) - before, expected, "{step}");
    }
}

/// Bytes of a frame that is not complete yet are progress too (016c on #47): the read gives `Pending`, and the no-spin
/// count starts again. A route that gives one byte at every 4th read, while both descriptors stay readable and the pump
/// does nothing, never reaches the bound: the step limit ends the wait, with no line.
#[test]
#[cfg(unix)]
fn bytes_of_a_frame_that_is_not_complete_reset_the_no_spin_count() {
    let mut harness = Descriptors::new(true, true);
    harness.dribble = 4;
    // timer: deadline — the step limit that ends the wait that the bytes keep below the bound
    let limit = Duration::from_millis(300);
    let (driver, elapsed, text) = route_wait(harness, limit);
    assert!(
        text.contains("Inconclusive") && text.contains("step_timeout"),
        "{text}"
    );
    assert!(elapsed >= limit, "{elapsed:?}");
    assert!(
        driver.diagnostics().is_empty(),
        "{:?}",
        driver.diagnostics()
    );
}

/// A readable wake counts toward the no-spin bound only after the pump and the reads that follow it (016c on #47). Both
/// descriptors stay readable and nothing makes progress, and the end of the stream comes at the first read after the
/// readable wake that is the 1000th in a row. A pass makes two reads, so that is read 2001. The wait takes the end, with no
/// diagnostic line.
#[test]
#[cfg(unix)]
fn the_end_of_the_stream_after_the_last_readable_wake_before_the_bound_closes_the_route_with_no_line(
) {
    let mut harness = Descriptors::new(true, true);
    harness.eof_after = 2 * 1000;
    let mut run = Run {
        driver: CoreDriver::new(Box::new(harness)),
        bindings: Bindings::new(),
    };
    run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
    running(&mut run, "s1");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    let result = botster_conformance::run_step(
        &json!({"expect_closed": null, "on": "r1"}),
        &mut run.driver,
        &mut run.bindings,
        &botster_conformance::NoSchemas,
        &botster_conformance::Limits {
            max_polls: usize::MAX,
            // timer: deadline — the step limit, far above the passes that the test makes
            step_timeout: Some(Duration::from_secs(10)),
        },
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(
        run.driver.diagnostics().is_empty(),
        "{:?}",
        run.driver.diagnostics()
    );
}

/// Every route read that can wait goes through the route wait (steward ruling R-68, item 4), and `route_expect` too
/// (016c and 0167 on #47). On an idle route with both descriptors present and neither readable, `route_expect` pumps the
/// route's handle, makes only reads that do not wait, and waits on the descriptors until the step deadline: the step is
/// inconclusive, never "no frame".
#[test]
#[cfg(unix)]
fn route_expect_on_an_idle_route_pumps_its_handle_and_waits_on_both_descriptors_until_the_step_deadline(
) {
    let pumps = Arc::new(AtomicUsize::new(0));
    let harness = Descriptors::with(Harness::new(Some(pumps.clone())), false, false);
    let reads = harness.reads.clone();
    let mut run = Run {
        driver: CoreDriver::new(Box::new(harness)),
        bindings: Bindings::new(),
    };
    run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
    running(&mut run, "s1");
    run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
    let before = pumps.load(Ordering::SeqCst);
    reads.lock().unwrap().clear();
    // timer: deadline — the step limit that ends the wait
    let limit = Duration::from_millis(300);
    let started = Instant::now();
    let result = run.driver.exec(
        &json!({"route_expect": {"frame": "output"}, "route": "r1"}),
        &mut run.bindings,
        &Deadline::after(Some(limit)),
    );
    let elapsed = started.elapsed();
    let text = format!("{:?}", result.unwrap_err());
    assert!(
        text.contains("Inconclusive") && text.contains("step_timeout"),
        "{text}"
    );
    assert!(elapsed >= limit, "{elapsed:?}");
    assert!(pumps.load(Ordering::SeqCst) > before, "no pump");
    let reads = reads.lock().unwrap().clone();
    assert!(!reads.is_empty());
    assert!(
        reads.iter().all(|left| *left == Some(Duration::ZERO)),
        "every read is one attempt: {reads:?}"
    );
}

/// A `Blocked` read (a stalled reader) is an idle read, as the route driver takes it (0167 R68-F4): the pass pumps the
/// route's handle and reads again. With both descriptors present and neither readable, the wait then lasts until the step
/// deadline. With no readiness descriptor, the pass still pumps before the route is idle.
#[test]
#[cfg(unix)]
fn a_blocked_read_is_idle_the_pass_pumps_the_handle_and_waits_on_the_descriptors() {
    for has_readiness in [true, false] {
        let pumps = Arc::new(AtomicUsize::new(0));
        let mut harness = Descriptors::with(Harness::new(Some(pumps.clone())), false, false);
        harness.blocked = true;
        harness.has_readiness = has_readiness;
        let mut run = Run {
            driver: CoreDriver::new(Box::new(harness)),
            bindings: Bindings::new(),
        };
        run.ok(json!({"open": {"as": "h", "data_dir": "d"}}));
        running(&mut run, "s1");
        run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
        let before = pumps.load(Ordering::SeqCst);
        // timer: deadline — the step limit that ends the wait
        let limit = Duration::from_millis(300);
        let started = Instant::now();
        let result = botster_conformance::run_step(
            &json!({"expect_closed": {"$any": true}, "on": "r1"}),
            &mut run.driver,
            &mut run.bindings,
            &botster_conformance::NoSchemas,
            &botster_conformance::Limits {
                max_polls: usize::MAX,
                step_timeout: Some(limit),
            },
        );
        let elapsed = started.elapsed();
        let text = format!("{:?}", result.unwrap_err());
        assert!(
            pumps.load(Ordering::SeqCst) > before,
            "{has_readiness}: no pump"
        );
        if has_readiness {
            assert!(
                text.contains("Inconclusive") && text.contains("step_timeout"),
                "{text}"
            );
            assert!(elapsed >= limit, "{elapsed:?}");
        } else {
            assert!(text.contains("no close"), "{text}");
        }
    }
}

/// Steward ruling R-89: the step deadline bounds every wait, on every path that goes on waiting. Five sources of "go on":
/// a Core whose pump always reports `more`; a Core whose pump always reports a posted event that no poll returns; a route
/// client that always reports progress; a Core that always names a next
/// deadline ahead of the injected clock, so an idle wait jumps the clock on every pass; and a wake handle that always
/// returns `Woken` with no work. Through `pump_until` and `pump_collect`, each ends INCONCLUSIVE `step_timeout`. Each
/// case runs on its own thread with a watchdog, so a wait that the deadline does not bound fails the test and never
/// hangs it. All cases start before the test waits for the first result, so the test takes about one step deadline,
/// not one per case (BUILD testing rule 4).
#[test]
fn a_step_deadline_ends_a_wait_whose_core_or_client_always_reports_progress() {
    let wanted = json!({"RouteStalled": {"route": 77}});
    let steps = [
        json!({"pump_until": {"event": wanted}}),
        json!({"pump_collect": {"events": [wanted]}}),
    ];
    let cases = [
        "core_more",
        "core_posted",
        "busy_client",
        "deadline_ahead",
        "always_woken",
    ]
    .into_iter()
    .flat_map(|source| steps.iter().map(move |step| (source, step.clone())))
    .map(|(source, step)| {
        let label = format!("{source} {step}");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut harness = Harness::new(Some(Arc::new(AtomicUsize::new(0))));
            match source {
                "core_more" => harness.more_every = 1,
                "core_posted" => harness.posted_every = 1,
                "busy_client" => harness.client_busy = true,
                "deadline_ahead" => harness.deadline_ahead = true,
                _ => harness.always_woken = true,
            }
            let mut run = Run::new(harness);
            running(&mut run, "s1");
            if source == "busy_client" {
                run.ok(json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}));
            }
            // timer: deadline — the step limit that must end the wait although every pass goes on
            let limit = Deadline::after(Some(Duration::from_millis(200)));
            let _ = tx.send(run.exec_within(step, &limit));
        });
        (label, rx)
    })
    .collect::<Vec<_>>();
    for (label, rx) in cases {
        // timer: deadline — the watchdog of a wait that the step deadline does not bound
        let watchdog = Duration::from_secs(10);
        let result = rx
            .recv_timeout(watchdog)
            .unwrap_or_else(|_| panic!("{label}: the wait outlived its step deadline"));
        let r = result.unwrap_err();
        assert!(
            r.contains("Inconclusive") && r.contains("step_timeout"),
            "{label}: {r}"
        );
    }
}

/// R-85: the R-85 transcript defect, as `Run::exec` prints a `StepError`.
fn is_0d_defect(result: &Result<(), String>) -> bool {
    result
        .as_ref()
        .is_err_and(|e| e.contains("TranscriptDefect") && e.contains("R-85"))
}

// R-85: a `pty_input` expectation compares no `0d`, whatever the source of its bytes: a literal, a `let` value, any computed
// value. The check runs before the control, on every tier, so it does not depend on the log. A `0d` that is not on a byte
// boundary is no `0d` byte, and bytes without one reach the matcher as before.
#[test]
fn a_pty_input_expectation_with_a_0d_byte_from_any_source_is_a_transcript_defect() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    let read = |bytes: Value| json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": bytes}});
    assert!(is_0d_defect(&run.exec(read(json!({"$bytes_hex": "610d"})))));
    assert!(is_0d_defect(&run.exec(read(json!({"$bytes_hex": "0D"})))));
    run.ok(json!({"let": {"$computed": {"hex_concat": ["61", "0d"]}}}));
    assert!(is_0d_defect(&run.exec(read(json!("$computed")))));
    // Not a 0d byte: the matcher decides, and the log of this session is empty.
    for bytes in [
        json!({"$bytes_hex": "a0d1"}),
        json!({"$bytes_hex": "6162"}),
        json!({"$bytes_hex": "0d0"}),
    ] {
        let got = run.exec(read(bytes.clone()));
        assert!(got.is_err() && !is_0d_defect(&got), "{bytes}: {got:?}");
    }
    run.ok(read(json!({"$bytes_hex": ""})));
}

// R-85: a capture of the input log (an unbound `expect` variable, `bind`, or `bind_fields` `bytes`) makes the log's variable an
// input log, so a later relation over it compares the other side with the log, in either order, and a `0d` in it is a transcript
// defect. The same relation over values that are not an input log is only a relation.
#[test]
fn a_check_over_a_captured_input_log_with_a_0d_byte_is_a_transcript_defect() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"let": {"$cr": {"hex_concat": ["0d", ""]}}}));
    let captures = [
        json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": "$by_expect"}}),
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$by_field"}}),
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind": "$by_bind"}),
    ];
    for capture in captures {
        run.ok(capture);
    }
    for log in ["$by_expect", "$by_field", "$by_bind"] {
        for rel in [
            json!({"hex_ends_with": [log, "$cr"]}),
            json!({"hex_prefix": ["$cr", log]}),
            json!({"hex_prefix": [log, "$cr"]}),
            json!({"hex_ends_with": ["$cr", log]}),
            json!({"eq": [log, "$cr"]}),
            json!({"ne": ["$cr", log]}),
            json!({"prefix_of": [log, "$cr"]}),
            json!({"suffix_of": ["$cr", log]}),
            json!({"eq": [{"bytes": log}, {"bytes": "$cr"}]}),
        ] {
            assert!(is_0d_defect(&run.exec(json!({ "check": rel }))), "{rel}");
            assert!(is_0d_defect(&run.exec(json!({ "assume": rel }))), "{rel}");
        }
        // A relation that compares no bytes with the log: a number is no 0d byte.
        let got = run.exec(json!({"check": {"hex_len_eq": [log, 0]}}));
        assert!(!is_0d_defect(&got), "{log}: {got:?}");
        // A string that is not hex is no bytes, though it holds `0d` at a byte boundary.
        let got = run.exec(json!({"check": {"eq": [log, "zz0d"]}}));
        assert!(!is_0d_defect(&got), "{log}: {got:?}");
    }
    run.ok(json!({"let": {"$other": {"hex_concat": ["61", "0d"]}}}));
    let got = run.exec(json!({"check": {"hex_ends_with": ["$other", "$cr"]}}));
    assert_eq!(got, Ok(()));
}

// R-85 (016c item 1 on #61): an `await_control` of `pty_input` is an input-log read too. Its expected bytes are checked before
// the first control read, and its capture holds the input log.
#[test]
fn an_await_control_of_pty_input_is_an_input_log_read() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    let read = |bytes: Value| json!({"await_control": {"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": bytes}}});
    assert!(is_0d_defect(&run.exec(read(json!({"$bytes_hex": "0d"})))));
    run.ok(read(json!("$waited")));
    assert!(is_0d_defect(
        &run.exec(json!({"check": {"hex_ends_with": ["$waited", "0d"]}}))
    ));
}

// R-85 (016c item 2 on #61): every hex string of a `pty_input` matcher is checked, inside a matcher form too: an alternative of
// `$one_of` with a 0d byte is a defect, even when another alternative (here the empty log) would match.
#[test]
fn a_0d_byte_in_any_alternative_of_a_pty_input_matcher_is_a_transcript_defect() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"let": {"$cr": {"hex_concat": ["0d", ""]}}}));
    let read = |bytes: Value| json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": bytes}});
    run.ok(read(
        json!({"$one_of": [{"$bytes_hex": ""}, {"$bytes_hex": "61"}]}),
    ));
    for bytes in [
        json!({"$one_of": [{"$bytes_hex": ""}, {"$bytes_hex": "0d"}]}),
        json!({"$one_of": [{"$bytes_hex": ""}, "$cr"]}),
        json!({"$one_of": [{"$bytes_hex": ""}, {"$bytes_hex": "$cr"}]}),
    ] {
        assert!(is_0d_defect(&run.exec(read(bytes.clone()))), "{bytes}");
    }
}

// R-85 (016c item 3 on #61): a variable that a later step binds again, to a value that is not the log, no longer holds the input
// log: a relation over it is only a relation. Here a `let` and a `call`'s `bind` and `bind_fields` replace it.
#[test]
fn a_rebound_input_log_variable_is_no_longer_an_input_log() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    for log in ["$by_let", "$by_bind", "$by_field"] {
        run.ok(json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": log}}));
    }
    run.ok(json!({"let": {"$by_let": {"hex_concat": ["61", "0d"]}}}));
    run.ok(json!({"call": {"method": "get", "args": {"id": "s1"}}, "bind": "$by_bind"}));
    run.ok(json!({"call": {"method": "get", "args": {"id": "s1"}}, "bind_fields": {"id": "$by_field"}}));
    assert_eq!(
        run.exec(json!({"check": {"hex_ends_with": ["$by_let", "0d"]}})),
        Ok(())
    );
    for log in ["$by_bind", "$by_field"] {
        let got = run.exec(json!({"check": {"eq": [log, "0d"]}}));
        assert!(got.is_err() && !is_0d_defect(&got), "{log}: {got:?}");
    }
}

// R-85 (016c item 4 on #61): a `let` that keeps the log's bytes (an alias, a field, a concatenation) holds the input log too; a
// `let` whose value is a number (a length) does not compare bytes.
#[test]
fn a_let_that_keeps_the_input_log_bytes_is_an_input_log() {
    let mut run = Run::new(Harness::new(None));
    running(&mut run, "s1");
    run.ok(json!({"control": {"op": "pty_input", "session": "s1"}, "bind": "$whole"}));
    run.ok(json!({"let": {"$field": {"field": ["$whole", "bytes"]}}}));
    run.ok(json!({"let": {"$alias": {"hex_concat": ["$field", ""]}}}));
    run.ok(json!({"let": {"$joined": {"hex_concat": ["$alias", "61"]}}}));
    run.ok(json!({"let": {"$n": {"hex_len": ["$field", 0]}}}));
    for alias in ["$field", "$alias", "$joined"] {
        assert!(
            is_0d_defect(&run.exec(json!({"check": {"hex_ends_with": [alias, "0d"]}}))),
            "{alias}"
        );
    }
    let got = run.exec(json!({"check": {"eq": ["$n", "0d"]}}));
    assert!(got.is_err() && !is_0d_defect(&got), "{got:?}");
    // 016c round 2: a self-assignment keeps the log (case A), and a later assignment of the same `let` that reads an earlier
    // one keeps it too (case B). A self-assignment that reads no log ends it.
    run.ok(json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": "$self"}}));
    run.ok(json!({"let": {"$self": {"hex_concat": ["61", "$self"]}}}));
    run.ok(
        json!({"let": {"$a": {"hex_concat": ["$alias", ""]}, "$z": {"hex_concat": ["$a", ""]}}}),
    );
    for alias in ["$self", "$a", "$z"] {
        assert!(
            is_0d_defect(&run.exec(json!({"check": {"hex_ends_with": [alias, "0d"]}}))),
            "{alias}"
        );
    }
    run.ok(json!({"let": {"$self": {"hex_concat": ["61", ""]}}}));
    let got = run.exec(json!({"check": {"hex_ends_with": ["$self", "0d"]}}));
    assert!(got.is_err() && !is_0d_defect(&got), "{got:?}");
}
