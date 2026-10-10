//! The Core step driver (design 5.5, 6.2): a [`StepDriver`] over a [`CoreHarness`].
//!
//! Operations go through `CoreApi` only, with event-driven waits (Core section 11). Every step names its handle with `"handle"`
//! (default `h`). Steps:
//!
//! | Step | Form |
//! |---|---|
//! | `data_dir` | `{"data_dir": "<name>"}` |
//! | `open` | `{"open": {as, data_dir?, worker?: "current"\|"previous"\|null, limits?}, "expect_error"?: <matcher>}` |
//! | `drop` | `{"drop": {handle}}` |
//! | `begin` | `{"begin": {handle?, op}, "bind"?: "$op", "expect_error"?: <matcher>}`; `op` is the JSON form of `Op`, and a `Create` request may name `program` (the probe script) |
//! | `pump` | `{"pump": {handle?}, "bind"?: "$r", "expect"?: <matcher>}`: one `pump`; the matcher checks the `PumpReport` (`more`, `events_posted`) |
//! | `pump_until` | `{"pump_until": {handle?, event: <matcher>}}`: pumps and polls until an event matches; it sets no pump count, and its limit is the runner's (inconclusive) |
//! | `poll_events` | `{"poll_events": {handle?, max}, "expect": [<matcher>, ..]}`: one poll, whose events match the list |
//! | `call` | `{"call": {handle?, method, args?}, "expect"?: <matcher>, "expect_error"?: <matcher>}`: a sync call (`get`, `list`, `status`, `terminal_state`, `cancel`, `read_page`, `release`, `features`, `limits`, `worker_protocol`, `set_silence_threshold{id, ms?}`, `terminal_identity`, `snapshot_formats{id}`, `release_owner{client}`, `next_deadline` (`{in_ms}` from the injected now, or null), `diagnostics`, ...) |
//! | `advance_clock` | `{"advance_clock": {ms?, unix?}}`: moves the monotonic clock by `ms`, and `unix` sets the unix clock (it may jump back; Core TM-1). The injected clock starts at unix 1000000 and follows the monotonic clock by whole seconds |
//! | `pump_idle` | `{"pump_idle": {handle?}}`: pumps until `PumpReport.more` is false; it asserts no pump count |
//! | `drain` | `{"drain": {handle?}}`: alternates pumps and polls until nothing is runnable and no event is queued, and drops the events with no check (Core EV-5(d), TM-6) |
//! | `poll_all` | `{"poll_all": {handle?}}`: polls and drops every queued event without a pump, so work that a poll unparks stays unpumped |
//! | `wait_wake` | `{"wait_wake": {handle?, timeout_ms?, other_thread?}, "expect": "Woken"\|"TimedOut"}` |
//! | `real_wait` | `{"real_wait": {handle?, ms}}`: observes the handle for `ms` of real time: it pumps and polls into the handle's buffer (no event is dropped or judged) and then ends. It asserts no wake, and no event. It waits on the wake handle, bounded by the interval and by the step deadline (an expired step deadline is inconclusive); it never polls on a period.  It never moves the host's clock, injected or real: the interval is real time, for a worker or a guardian whose deadline runs on its own clock. It is the real monotonic deadline that delimits an observation interval of a slow-tier test (Core SV-8, AD-7: a real guardian or worker runs on its own clock, which `advance_clock` does not move) |
//! | `poll_find` | `{"poll_find": {handle?, event: <matcher>}}`: polls the queued events of the handle (in batches, until the queue is empty) and never pumps and never moves the clock; the first buffered event that matches is taken and binds its variables, and every other event stays for the next step. It fails when no queued event matches. It reads what an earlier pump posted (Core E3-1: the events of one pump) |
//! | `control` | `{"control": {op, handle?, ..args}, "expect"?: <matcher>, "bind"?: "$v", "bind_fields"?: {field: "$v"}}`: a test control of the harness, or of a route (`docs/core-testkit-controls.md`) |
//! | `pump_collect` | `{"pump_collect": {handle?, events: [<matcher>, ..], exhaustive?, ignore?: [<matcher>, ..], forbid?: [<matcher>, ..]}}`: pumps and polls until every matcher has matched a distinct event, in any order; other events pass, and a second event for a matcher that matched is a failure. An event that matches a `forbid` matcher fails the step, whenever it comes before the end. With `exhaustive`, an event that no matcher wants (and no `ignore` matcher lets pass) fails the step, and so does an event that is queued once every matcher has matched and Core is at rest |
//! | `await_quiet` | `{"await_quiet": {handle?}}`: pumps (never polls, never moves the clock) until no runnable work remains and the control `edges_quiet` says that no edge (program, process, route transport, service lane) holds an undelivered report. Core A5-2 lets the edges and the scheduler split output and defer progress, so this is the fence before a clock step or a read of cached state, and it keeps every queued event |
//! | `await_control` | `{"await_control": {handle?, control: {op, ..args}, expect: <matcher>}}`: pumps (no poll, no clock step) until the control answers as the matcher says. The fence for one effect at an edge while a state report is intentionally parked, when `await_quiet` could not end |
//! | `await_get` | `{"await_get": {handle?, id, expect?\|expect_error?}}`: pumps (no poll, no clock step) until the cached `get` of the session matches: the fence for a lifecycle state |
//! | `await_pty` | `{"await_pty": {session, min_bytes}}`: pumps until at least that many bytes reached the PTY (control `pty_input`) |
//! | `let` | `{"let": {"$x": {add\|sub\|gt\|lt\|eq\|contains\|field\|without\|hex_concat\|concat\|repeat: [a, b], str_len\|hex_len: [a, b] (the second argument is not used), rtrim_spaces\|record_payloads\|sum_hex_bytes\|utf8_len: [a]}}}`: binds a value computed from bound values; `call` and `pump` also take `bind` and `bind_fields` |
//! | `check` | `{"check": {eq\|ne\|lt\|le\|gt\|contains\|subset_of\|prefix_of\|suffix_of: [a, b], mul_eq: [a, k, b], hex_len_eq: [hex, n], hex_prefix: [prefix, whole], hex_ends_with: [whole, suffix]}}`: a relation between bound values |
//! | `collect_events` | `{"collect_events": {handle?}, "bind": "$e"}`: like `drain`, but binds the events that it polled, in order (Core A5-1) |
//! | `expect_refused` | a `control` step with `"expect_refused": <matcher>` passes only when the harness refuses the control with a typed value (`ControlError::Refused`) that the matcher accepts (Core A5-3: a code that is not in the call's sync column cannot be scripted; the value names the call and the code) |
//! | `forbid_events` | `{"forbid_events": {handle?, events: [<matcher>, ..]}}` from here on, an event that matches fails the step that polls it, whichever wait polled it; `{"forbid_events": {"clear": true}}` ends it (Core A5-2, E2-3: a quiet check stays active through every wait) |
//! | `tap_drain` | `{"tap_drain": {session, max?, accounted?}, "bind": "$t"}`: reads the tap, and binds `{bytes, dropped, instances}`: the bytes in order, the sum of `dropped_before`, and the instance of each chunk with bytes. With `accounted: N` it goes on across pumps and wakes until bytes read plus bytes dropped are N (an empty read is no barrier); without it, it ends at the first empty read. The contract fixes neither the number of reads nor their sizes (Core TP-1, EV-9) |
//! | `run_suite`, `check_crates`, `error_codes_reachable`, `run_deterministic` | statement steps (Core A5-1, A5-3, A5-4): the harness runs them (`CoreHarness::statement`) and reports, and the driver checks the report against the statement. A harness without the capability gives `unsupported_control`, never a pass. `run_script_events` runs a script on a fresh harness for `run_deterministic` |
//! | `assume` | `{"assume": {<relation of check>}}`: a precondition that the contract leaves open. If the relation does not hold the step is `inconclusive` (`assumption`), never a failure; a malformed relation is still an error |
//! | `route_flood` | `{"route_flood": {route?, max?}, "bind"?: "$n"}`: sends refused `text` frames (ops 1, 2, ...) until the route's `input_blocked` says the stream takes no more; binds the number sent |
//! | `route_expect_refusals` | `{"route_expect_refusals": {route?, count, reason?}}`: the next `count` refusals of the route are the ops 1 to `count`, in order, each with the reason |
//! | `attach_route` | `{"attach_route": {route, session, client?, options?}, "expect_error"?: <matcher>}`, then the route steps of the codec suite |
//!
//! The order constructs (`expect`, `expect_set`, ...) take `"on"`: a route name for route frames, anything else for the events of a handle.
//! An expectation carries `"because"`: the clause that fixes its value (design 5.6, rule 6).

use crate::harness::*;
use botster_conformance::matcher::matches;
use botster_conformance::{
    substitute, Bindings, Deadline, Direction, FrameSchema, NoSchemas, Poll, SchemaResolver,
    StepDriver, StepError,
};
use botster_core_contract::prelude::*;
use botster_hub_conformance::route::{Framing, RouteDriver, RouteRead, RouteSchemas, RouteSubject};
use botster_route_codec::prelude::HexBytes;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

const DEFAULT_HANDLE: &str = "h";

fn failed(expected: Value, received: Value) -> StepError {
    StepError::Failed { expected, received }
}

fn bad(reason: impl Into<String>) -> StepError {
    failed(json!("a well-formed Core step"), json!(reason.into()))
}

/// The schema resolver of the Core suite: a route frame is checked against the codec schemas, and a Core event is not a frame.
pub struct CoreSchemas;

impl SchemaResolver for CoreSchemas {
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String> {
        RouteSchemas.validate(schema_ref, value)
    }
    fn violations(&self, schema_ref: &str, value: &Value) -> Result<Vec<String>, String> {
        RouteSchemas.violations(schema_ref, value)
    }
    fn frame_schema(&self, direction: Direction, frame: &Value) -> FrameSchema {
        if frame.get("frame").is_some() || frame.get("eof").is_some() {
            RouteSchemas.frame_schema(direction, frame)
        } else {
            FrameSchema::NotFramed
        }
    }
}

/// The clock that the driver passes to `pump`. Against a fake it is injected and moves only by `advance_clock` or by a jump to the next
/// deadline of an idle wait (the host would arm its timer for that instant, Core TM-3). Against a real Core it follows real time.
/// An injected clock jumps only when the harness's progress is injected too (steward ruling R-46): with real worker processes a jump
/// would expire Core's deadlines before the workers act.
struct Clock {
    base: Instant,
    unix: u64,
    elapsed: Duration,
    real: bool,
    /// An idle wait may jump the injected clock to Core's next deadline (`CoreHarness::progress_is_injected`).
    jumps: bool,
    /// A jump of the unix clock alone (Core TM-1): the unix time then, and the monotonic time of the jump.
    unix_jump: Option<(u64, Duration)>,
}

impl Clock {
    fn now(&self) -> Now {
        let since = if self.real {
            self.base.elapsed()
        } else {
            self.elapsed
        };
        let unix = match self.unix_jump {
            Some((at, from)) => at + since.saturating_sub(from).as_secs(),
            None => self.unix + since.as_secs(),
        };
        Now {
            monotonic: self.base + since,
            unix,
        }
    }

    /// Moves an injected clock to `at`. A real clock moves by itself.
    fn jump_to(&mut self, at: Instant) {
        if !self.real {
            self.elapsed = self.elapsed.max(at.saturating_duration_since(self.base));
        }
    }
}

struct Handle {
    core: Box<dyn CoreApi>,
    events: VecDeque<Value>,
}

/// What the route subject and the driver share.
struct Shared {
    ends: BTreeMap<u32, Box<dyn RouteClient>>,
    limits: BTreeMap<u32, AppliedRouteLimits>,
    fake: bool,
}

type Share = Rc<RefCell<Shared>>;

/// The route endpoint of Core: the client end that the harness kept (design 6.2: `attach_route` followed by route steps).
struct CoreRoutes(Share);

impl RouteSubject for CoreRoutes {
    fn is_fake(&self) -> bool {
        self.0.borrow().fake
    }
    fn has_feature(&self, _name: &str) -> bool {
        false
    }
    fn has_control(&self, op: &str) -> bool {
        self.0.borrow().ends.values().any(|e| e.has_control(op))
    }
    fn configure(&mut self, _worker: &Value) -> Result<(), String> {
        Ok(())
    }
    fn attach(&mut self, spec: &Value) -> Result<u32, String> {
        spec.get("end")
            .and_then(Value::as_u64)
            .map(|n| n as u32)
            .ok_or_else(|| "a route needs an attach_route first".to_string())
    }
    fn framing(&self) -> Framing {
        Framing::Stream
    }
    fn bounds(&self) -> botster_route_codec::prelude::FrameBounds {
        let s = self.0.borrow();
        let l = s.limits.values().next();
        botster_route_codec::prelude::FrameBounds {
            max_frame: l.map_or(4096, |l| l.max_frame_bytes),
            max_screen: l.map_or(65536, |l| l.max_screen_frame_bytes),
            max_history: l.map_or(4096, |l| l.max_history_page_bytes),
        }
    }
    fn max_chunk_bytes(&self) -> usize {
        64
    }
    fn write(&mut self, route: u32, bytes: &[u8]) {
        if let Some(end) = self.0.borrow_mut().ends.get_mut(&route) {
            end.write(bytes);
        }
    }
    fn read(&mut self, route: u32, max: usize, deadline: &Deadline) -> RouteRead {
        match self.0.borrow_mut().ends.get_mut(&route) {
            Some(end) => end.read(max, deadline),
            None => RouteRead::Eof { ended: None },
        }
    }
    fn control(&mut self, op: &str, args: &Value, route: Option<u32>) -> Result<Value, String> {
        let mut s = self.0.borrow_mut();
        // A control without a route (for example `pty_input`, which is the whole session's) goes through the first attached route.
        let route = route.or_else(|| s.ends.keys().next().copied());
        let end = route
            .and_then(|r| s.ends.get_mut(&r))
            .ok_or("a route control needs a route")?;
        end.control(op, args)
    }
    fn client_model(
        &self,
        _terminal_format: &str,
    ) -> Option<Box<dyn botster_hub_conformance::route::ClientModel>> {
        None
    }
}

/// Drives a Core through its harness.
pub struct CoreDriver {
    harness: Box<dyn CoreHarness>,
    handles: BTreeMap<String, Handle>,
    clock: Clock,
    share: Share,
    routes: RouteDriver<CoreRoutes>,
    next_end: u32,
    /// The handle that route steps pump when a route has nothing to read.
    route_handle: String,
    /// Routes that were read once after a pump: the transport handoff happens in a pump (Core OU-9), and nothing after it does.
    handed_off: std::collections::BTreeSet<String>,
    /// The matchers of the events that must never be polled (`forbid_events`), by handle.
    forbidden: BTreeMap<String, Vec<Value>>,
}

impl CoreDriver {
    pub fn new(harness: Box<dyn CoreHarness>) -> Self {
        let harness_injects = harness.injects_clock();
        let progress_injected = harness.progress_is_injected();
        let share = Rc::new(RefCell::new(Shared {
            ends: BTreeMap::new(),
            limits: BTreeMap::new(),
            fake: harness.is_fake(),
        }));
        CoreDriver {
            harness,
            handles: BTreeMap::new(),
            clock: Clock {
                base: Instant::now(),
                unix: 1_000_000,
                elapsed: Duration::ZERO,
                real: !harness_injects,
                jumps: harness_injects && progress_injected,
                unix_jump: None,
            },
            routes: RouteDriver::new(CoreRoutes(share.clone())),
            share,
            next_end: 1,
            route_handle: DEFAULT_HANDLE.to_string(),
            handed_off: Default::default(),
            forbidden: Default::default(),
        }
    }

    /// Core A5-2, E2-3: an event that a `forbid_events` matcher names fails the step that polls it, whichever wait polled it.
    fn check_forbidden(&self, name: &str, event: &Value) -> Result<(), StepError> {
        for m in self.forbidden.get(name).into_iter().flatten() {
            if matches(m, Some(event), &mut Bindings::new(), &NoSchemas).is_ok() {
                return Err(failed(
                    json!({ "forbidden": m }),
                    json!({ "polled": event }),
                ));
            }
        }
        Ok(())
    }

    fn handle(&mut self, name: &str) -> Result<&mut Handle, StepError> {
        self.handles
            .get_mut(name)
            .ok_or_else(|| bad(format!("handle '{name}' is not open")))
    }

    fn pump(&mut self, name: &str) -> Result<PumpReport, StepError> {
        let now = self.clock.now();
        Ok(self.handle(name)?.core.pump(now))
    }

    /// Polls the events that are queued, into the handle's buffer. Returns whether any came.
    fn fill(&mut self, name: &str) -> Result<bool, StepError> {
        let events = self.handle(name)?.core.poll_events(64);
        let any = !events.is_empty();
        for e in events {
            let v = serde_json::to_value(&e).map_err(|e| bad(e.to_string()))?;
            self.check_forbidden(name, &v)?;
            self.handle(name)?.events.push_back(v);
        }
        Ok(any)
    }

    /// The next event of a handle: a queued one, or one that a pump produces. A wait ends on an event, or on the wake handle (Core TM-3).
    fn next_event(&mut self, name: &str, deadline: &Deadline) -> Result<Poll, StepError> {
        if let Some(e) = self.handle(name)?.events.pop_front() {
            return Ok(Poll::Item(e));
        }
        if self.fill(name)? {
            return self.next_event(name, deadline);
        }
        let report = self.pump(name)?;
        if self.fill(name)? || report.events_posted > 0 || report.more {
            return Ok(Poll::Pending);
        }
        self.idle_wait(name, deadline)
    }

    /// Nothing is runnable. A wait ends on an event, on the wake handle or on a deadline, never on a count of pumps (design 5.5, 6.1).
    fn idle_wait(&mut self, name: &str, deadline: &Deadline) -> Result<Poll, StepError> {
        // Nothing to do now. A deadline that Core owns is armed by the host (Core TM-3): an injected clock that controls all progress
        // jumps to it, and a real wait is bounded by it. The wake handle never fires for a deadline. An injected clock that does not
        // jump (R-46) moves only by `advance_clock`: its deadline is no bound of a real wait.
        let next = self.handle(name)?.core.next_deadline();
        if let (Some(at), true) = (next, self.clock.jumps) {
            if at > self.clock.now().monotonic {
                self.clock.jump_to(at);
                return Ok(Poll::Pending);
            }
        }
        let until_deadline = next
            .filter(|_| self.clock.real || self.clock.jumps)
            .map(|at| at.saturating_duration_since(self.clock.now().monotonic));
        let step_left = deadline.remaining();
        let timeout = match (until_deadline, step_left) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a.or(b).unwrap_or(Duration::ZERO),
        };
        let wake = self.handle(name)?.core.wake_handle();
        Ok(match wake.wait(timeout) {
            Wake::Woken => Poll::Pending,
            _ if deadline.expired() => Poll::TimedOut,
            // A real Core may still have a deadline or worker input to come: the step limit, not idleness, ends the wait (design 6.1).
            // So may real progress under an injected clock that does not jump (R-46).
            _ if !self.clock.jumps && step_left.is_some() => Poll::Pending,
            _ => Poll::Idle,
        })
    }

    fn open(&mut self, spec: &Value) -> Result<(), StepError> {
        let name = spec
            .get("as")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        let dir = spec.get("data_dir").and_then(Value::as_str).unwrap_or("d");
        let build = match spec.get("worker") {
            Some(Value::Null) => None,
            Some(Value::Object(w)) => {
                let name = w
                    .get("file_name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad("a worker object names its 'file_name'"))?;
                Some(
                    self.harness
                        .worker_named(name)
                        .ok_or(StepError::Unsupported {
                            op: "worker:file_name".into(),
                        })?,
                )
            }
            Some(Value::String(s)) if s == "previous" => self.harness.worker(WorkerBuild::Previous),
            _ => self.harness.worker(WorkerBuild::Current),
        };
        if spec.get("worker") == Some(&Value::String("previous".into())) && build.is_none() {
            return Err(StepError::Unsupported {
                op: "worker:previous".into(),
            });
        }
        let open = OpenSpec {
            handle: name.clone(),
            data_dir: self.harness.data_dir(dir),
            worker: build,
            limits: spec.get("limits").cloned().unwrap_or_else(|| json!({})),
        };
        match self.harness.open(&open) {
            Ok(_) if spec.get("expect_error").is_some() => Err(failed(
                spec["expect_error"].clone(),
                json!("the open succeeded"),
            )),
            Ok(core) => {
                self.handles.insert(
                    name,
                    Handle {
                        core,
                        events: VecDeque::new(),
                    },
                );
                Ok(())
            }
            Err(e) => self.error_result(e, spec_expect(spec), None),
        }
    }

    /// An error of Core is a result: it must match `expect_error`, or the step fails.
    fn error_result(
        &self,
        e: CoreError,
        expect: Option<&Value>,
        bindings: Option<&mut Bindings>,
    ) -> Result<(), StepError> {
        if e.detail.starts_with("unsupported_control:") {
            return Err(StepError::Unsupported {
                op: e
                    .detail
                    .trim_start_matches("unsupported_control:")
                    .trim()
                    .to_string(),
            });
        }
        let got = serde_json::to_value(&e).map_err(|e| bad(e.to_string()))?;
        match expect {
            Some(m) => matches(
                m,
                Some(&got),
                &mut bindings.map_or_else(Bindings::new, |b| b.clone()),
                &NoSchemas,
            )
            .map_err(|f| failed(m.clone(), json!({ "error": got, "reason": f.describe() }))),
            None => Err(failed(json!("success"), got)),
        }
    }

    fn core_control(
        &mut self,
        step: &Value,
        bindings: &Bindings,
    ) -> Option<Result<Value, StepError>> {
        // A control that names a route is a control of that route's endpoint (the route steps): the harness never sees it.
        if step.get("route").is_some() {
            return None;
        }
        let c = step.get("control")?;
        let op = c.get("op").and_then(Value::as_str)?.to_string();
        if !self.harness.has_control(&op) {
            return None;
        }
        // The arguments may name values that earlier steps bound (a capture, a screen text, an offer): a harness has no bindings.
        let c = &match substitute(c, bindings) {
            Ok(c) => c,
            Err(v) => return Some(Err(bad(format!("unbound variable {v}")))),
        };
        let handle = c
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        Some(self.harness.control(&handle, &op, c).map_err(|e| match e {
            ControlError::Unsupported => StepError::Unsupported { op: op.clone() },
            ControlError::Bad(why) => bad(format!("control '{op}': {why}")),
            ControlError::Refused(v) => failed(json!("the control runs"), json!({ "refused": v })),
        }))
    }

    /// The `Create` request of a transcript names its program (design 6.2): the driver turns it into the argv of the probe binary.
    fn normalize(&self, op: &mut Value) {
        normalize_op(op, &self.harness.probe_binary());
    }
}

/// The `Create` request and the `SpawnService` of a transcript name their program (design 6.2): this turns a transcript op into the
/// op that `Op` reads, with the probe binary as `argv[0]`.
pub fn normalize_op(op: &mut Value, probe: &str) {
    {
        if let Some(spawn) = op.get_mut("SpawnService").and_then(Value::as_object_mut) {
            // A service runs a probe program too (design 6.2), and the lane limits have a small default.
            let program = spawn.remove("program").unwrap_or(json!([{ "hold": {} }]));
            spawn
                .entry("argv")
                .or_insert(json!([probe, json!({ "program": program }).to_string()]));
            spawn.entry("env").or_insert(json!({}));
            spawn.entry("cwd").or_insert(json!("/"));
            spawn.entry("limits").or_insert(json!({
                "lanes": [{ "max_frame_bytes": 1024, "outbound_queue_bytes": 4096, "inbound_queue_bytes": 4096 }],
                "rlimits": {}, "startup": 10000, "stop_grace": 5000, "orphan_grace": 5000
            }));
            return;
        }
        let Some(request) = op.get_mut("Create").and_then(|c| c.get_mut("request")) else {
            return;
        };
        let Some(map) = request.as_object_mut() else {
            return;
        };
        let program = map.remove("program");
        let fake = map.remove("fake");
        if program.is_some() || fake.is_some() || !map.contains_key("argv") {
            let script = json!({ "program": program.unwrap_or(json!([{ "hold": {} }])), "fake": fake.unwrap_or(json!({})) });
            map.insert("argv".into(), json!([probe, script.to_string()]));
        }
        map.entry("env").or_insert(json!({}));
        map.entry("cwd").or_insert(json!("/"));
        map.entry("size")
            .or_insert(json!({ "rows": 24, "cols": 80 }));
    }
}

impl CoreDriver {
    fn call(
        &mut self,
        name: &str,
        method: &str,
        args: &Value,
    ) -> Result<Result<Value, CoreError>, StepError> {
        let now = self.clock.now().monotonic;
        let h = self.handle(name)?;
        let sid = || {
            SessionId(
                args.get("id")
                    .or(args.get("session"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            )
        };
        let number = |key: &str| {
            args.get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| bad(format!("'{method}' needs '{key}'")))
        };
        Ok(match method {
            "get" => h.core.get(&sid()).map(|r| js(&r)),
            "list" => Ok(js(&h.core.list())),
            "status" => Ok(js(&h.core.status())),
            "terminal_state" => h.core.terminal_state(&sid()).map(|r| js(&r)),
            "features" => Ok(js(&h.core.features())),
            "limits" => Ok(js(&h.core.limits())),
            "worker_protocol" => Ok(json!(h.core.worker_protocol())),
            "adoptable_worker_protocols" => Ok(js(&h.core.adoptable_worker_protocols())),
            "worker_protocol_compatibility" => {
                let p = args
                    .get("protocol")
                    .and_then(Value::as_u64)
                    .map(|n| n as u8);
                Ok(js(&h.core.worker_protocol_compatibility(p)))
            }
            "cancel" => Ok(js(&h.core.cancel(OpId(number("op")?)))),
            "read_page" => {
                let page = args.get("page").and_then(Value::as_u64).unwrap_or(0) as u32;
                h.core
                    .read_page(CaptureId(number("capture")?), page)
                    .map(|r| js(&r))
            }
            "release" => {
                h.core.release(CaptureId(number("capture")?));
                Ok(Value::Null)
            }
            "diagnostics" => Ok(h.core.diagnostics()),
            "terminal_identity" => Ok(js(&h.core.terminal_identity())),
            "snapshot_formats" => h.core.snapshot_formats(&sid()).map(|r| js(&r)),
            "release_owner" => {
                let client = ClientId(
                    args.get("client")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
                h.core.release_owner(&client);
                Ok(Value::Null)
            }
            "set_silence_threshold" => {
                // `ms: null` or no `ms` turns the threshold off (Core A2-7).
                let threshold = args.get("ms").and_then(duration_of);
                h.core
                    .set_silence_threshold(&sid(), threshold)
                    .map(|()| Value::Null)
            }
            "next_deadline" => {
                // Core TM-3: the earliest deadline, as milliseconds from the injected `now`; null when Core owns none.
                return Ok(Ok(match h.core.next_deadline() {
                    Some(at) => {
                        json!({ "in_ms": at.saturating_duration_since(now).as_millis() as u64 })
                    }
                    None => Value::Null,
                }));
            }
            "tap_read" => {
                // Core TP-1: the host drains the raw-output tap with a sync read.
                let max = number("max")? as usize;
                h.core.tap_read(&sid(), max).map(|r| js(&r))
            }
            "service_report" => {
                let id: ServiceId =
                    serde_json::from_value(args.get("id").cloned().unwrap_or_default())
                        .map_err(|e| bad(e.to_string()))?;
                h.core.service_report(&id).map(|r| js(&r))
            }
            // Core SV-3, A2-5: the lane calls. `args`: `id` (the service id), `lane`, and for a send `type` and `payload_hex`. A lane
            // call has no `CoreError` (A2-5: `Backpressured` is a `SendError` value, not a code), so its result is `{"ok": ..}` or
            // `{"error": "<value>"}`, and a transcript matches it with `expect`.
            "service_send" => {
                let (id, lane) = service_lane(args)?;
                let frame = OutboundFrame {
                    frame_type: u8::try_from(number("type")?)
                        .map_err(|_| bad("'type' is a byte (0 to 255)"))?,
                    payload: hex_arg(args, "payload_hex")?,
                };
                Ok(match h.core.service_send(&id, lane, &frame) {
                    Ok(()) => json!({ "ok": null }),
                    Err(e) => json!({ "error": js(&e) }),
                })
            }
            "service_recv" => {
                let (id, lane) = service_lane(args)?;
                Ok(match h.core.service_recv(&id, lane) {
                    Ok(frame) => json!({ "ok": frame.map(|f| js(&f)) }),
                    Err(e) => json!({ "error": js(&e) }),
                })
            }
            // Core SV-9: a sync read of the bounded log ring. `max` bounds the bytes.
            "service_log_tail" => {
                let id = service_id(args, "id")?;
                let max =
                    usize::try_from(number("max")?).map_err(|_| bad("'max' is a byte count"))?;
                h.core
                    .service_log_tail(&id, max)
                    .map(|b| json!({ "len": b.len(), "bytes": js(&HexBytes(b)) }))
            }
            other => {
                return Err(StepError::Unsupported {
                    op: format!("call:{other}"),
                })
            }
        })
    }

    fn attach(
        &mut self,
        spec: &Value,
        step: &Value,
        bindings: &mut Bindings,
    ) -> Result<(), StepError> {
        let route = spec
            .get("route")
            .and_then(Value::as_str)
            .unwrap_or("r1")
            .to_string();
        let name = spec
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        let session = SessionId(
            spec.get("session")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("needs 'session'"))?
                .to_string(),
        );
        let client = ClientId(
            spec.get("client")
                .and_then(Value::as_str)
                .unwrap_or("c1")
                .to_string(),
        );
        let mut options =
            json!({ "file_directory": "/tmp", "answers_queries": false, "input": true });
        if let (Some(o), Some(given)) = (
            options.as_object_mut(),
            spec.get("options").and_then(Value::as_object),
        ) {
            o.extend(given.clone());
        }
        let exact = take_nanos(&mut options);
        let mut options: AttachOptions =
            serde_json::from_value(options).map_err(|e| bad(format!("attach options: {e}")))?;
        apply_nanos(&mut options, &exact);
        let h = self
            .handles
            .get_mut(&name)
            .ok_or_else(|| bad(format!("handle '{name}' is not open")))?;
        match self
            .harness
            .attach_stream(&name, h.core.as_mut(), client, &session, options)
        {
            Ok(_) if step.get("expect_error").is_some() => Err(failed(
                step["expect_error"].clone(),
                json!("the attach succeeded"),
            )),
            Ok((result, end)) => {
                let n = self.next_end;
                self.next_end += 1;
                bindings.insert(route.clone(), json!(result.route.0));
                {
                    let mut s = self.share.borrow_mut();
                    s.ends.insert(n, end);
                    s.limits.insert(n, result.limits);
                }
                self.route_handle = name;
                let mut inner = json!({ "route_attach": { "end": n }, "route": route });
                if let Some(h) = spec.get("history") {
                    inner["route_attach"]["history"] = h.clone();
                }
                self.routes.exec(&inner, bindings, &Deadline::none())
            }
            Err(e) => self.error_result(e, step.get("expect_error"), Some(bindings)),
        }
    }
}

fn service_id(args: &Value, key: &str) -> Result<ServiceId, StepError> {
    serde_json::from_value(args.get(key).cloned().unwrap_or_default())
        .map_err(|e| bad(e.to_string()))
}

fn service_lane(args: &Value) -> Result<(ServiceId, u8), StepError> {
    let lane = args
        .get("lane")
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or_else(|| bad("a lane call needs 'lane' (0 to 255)"))?;
    Ok((service_id(args, "id")?, lane))
}

fn hex_arg(args: &Value, key: &str) -> Result<HexBytes, StepError> {
    let text = args
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("'{key}' is a hex string")))?;
    botster_route_codec::prelude::hex_decode(text)
        .map(HexBytes)
        .map_err(|e| bad(format!("'{key}': {e}")))
}

/// The attach options that are a Duration (Core OU-1): the whole-millisecond form `5` keeps its meaning, and `{"ns": n}` is the exact
/// `Duration::from_nanos(n)`, which a transcript needs for a value below one millisecond (Core A7-1). The exact values are taken out of
/// the options here and put back after the options are read, because the contract type carries whole milliseconds on the wire.
/// A Duration-valued step argument or attach option, in the one form that the runner reads everywhere (Core OU-1, A7-1, TM-1): a whole
/// number of milliseconds, as before, or `{"ns": n}`, which is exactly `Duration::from_nanos(n)`. Anything else is not a duration.
fn duration_of(v: &Value) -> Option<Duration> {
    if let Some(ms) = v.as_u64() {
        return Some(Duration::from_millis(ms));
    }
    v.get("ns")
        .and_then(Value::as_u64)
        .map(Duration::from_nanos)
}

const DURATION_OPTIONS: &[&str] = &["query_deadline", "stall_deadline"];

fn take_nanos(options: &mut Value) -> Vec<(&'static str, Duration)> {
    let mut exact = vec![];
    let Some(map) = options.as_object_mut() else {
        return exact;
    };
    for name in DURATION_OPTIONS {
        let given = map
            .get(*name)
            .filter(|v| v.get("ns").is_some())
            .and_then(duration_of);
        if let Some(d) = given {
            // A placeholder of one millisecond reads as a valid option; the exact value replaces it.
            map.insert((*name).to_string(), json!(1));
            exact.push((*name, d));
        }
    }
    exact
}

fn apply_nanos(options: &mut AttachOptions, exact: &[(&'static str, Duration)]) {
    for (name, d) in exact {
        let d = Some(*d);
        match *name {
            "query_deadline" => options.query_deadline = d,
            _ => options.stall_deadline = d,
        }
    }
}

fn spec_expect(spec: &Value) -> Option<&Value> {
    spec.get("expect_error")
}

fn js<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("a result serializes")
}

impl StepDriver for CoreDriver {
    fn is_fake(&self) -> bool {
        self.harness.is_fake()
    }

    fn has_feature(&self, name: &str) -> bool {
        self.handles.get(DEFAULT_HANDLE).is_some_and(|h| {
            h.core.features().names.iter().any(|f| {
                serde_json::to_value(f)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s == name))
                    .unwrap_or(false)
            })
        })
    }

    fn has_control(&self, op: &str) -> bool {
        self.harness.has_control(op)
    }

    /// `setup.limits` configures the default handle, which opens before the first step unless `setup.open` is false.
    fn setup(&mut self, setup: &Value) -> Result<(), StepError> {
        if setup.get("open") == Some(&Value::Bool(false)) {
            return Ok(());
        }
        let mut spec = json!({ "as": DEFAULT_HANDLE, "data_dir": "d" });
        if let Some(limits) = setup.get("limits") {
            spec["limits"] = limits.clone();
        }
        self.open(&spec)
    }

    fn exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let obj = step.as_object().ok_or_else(|| bad("a step is an object"))?;
        if let Some(name) = obj.get("data_dir").and_then(Value::as_str) {
            self.harness.data_dir(name);
            return Ok(());
        }
        if let Some(spec) = obj.get("open") {
            // The limits may name values that an earlier step bound (a size that an earlier capture reported).
            let spec =
                substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            return self.open(&merge_expect(&spec, step));
        }
        if let Some(spec) = obj.get("drop") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE);
            self.handles.remove(name);
            self.harness.drop_handle(name);
            return Ok(());
        }
        if let Some(spec) = obj.get("begin") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let mut op = substitute(
                spec.get("op").ok_or_else(|| bad("begin needs 'op'"))?,
                bindings,
            )
            .map_err(|v| bad(format!("unbound variable {v}")))?;
            self.normalize(&mut op);
            let op: Op = serde_json::from_value(op).map_err(|e| bad(format!("begin op: {e}")))?;
            return match self.handle(&name)?.core.begin(op) {
                Ok(id) => {
                    if step.get("expect_error").is_some() {
                        return Err(failed(step["expect_error"].clone(), json!({ "ok": id.0 })));
                    }
                    if let Some(var) = step.get("bind").and_then(Value::as_str) {
                        bindings.insert(var.trim_start_matches('$').to_string(), json!(id.0));
                    }
                    Ok(())
                }
                Err(e) => self.error_result(e, step.get("expect_error"), Some(bindings)),
            };
        }
        if let Some(spec) = obj.get("pump") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let report = self.pump(&name)?;
            let value = js(&report);
            bind_result(step, &value, bindings)?;
            return match step.get("expect") {
                Some(m) => matches(m, Some(&value), bindings, &NoSchemas).map_err(|f| {
                    failed(m.clone(), json!({ "pump": value, "reason": f.describe() }))
                }),
                None => Ok(()),
            };
        }
        if let Some(spec) = obj.get("pump_idle") {
            return self.pump_idle(spec, deadline);
        }
        if let Some(spec) = obj.get("poll_all") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            self.poll_all(&name)?;
            return Ok(());
        }
        if let Some(spec) = obj.get("drain") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            return self.drain(&name, deadline);
        }
        if let Some(spec) = obj.get("pump_until") {
            return self.pump_until(spec, step, bindings, deadline);
        }
        if let Some(spec) = obj.get("pump_collect") {
            return self.pump_collect(spec, bindings, deadline);
        }
        if let Some(spec) = obj.get("await_quiet") {
            return self.await_quiet(spec, deadline);
        }
        if let Some(spec) = obj.get("await_control") {
            return self.await_control(spec, bindings, deadline);
        }
        if let Some(spec) = obj.get("await_get") {
            return self.await_get(spec, bindings, deadline);
        }
        if let Some(spec) = obj.get("await_pty") {
            // `min_bytes` may be a number that an earlier step computed (the length of an expected reply).
            let spec =
                substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            return self.await_pty(&spec, deadline);
        }
        if let Some(rel) = obj.get("assume") {
            // A precondition that the contract leaves open (for example that a capture is smaller than its bound): when it does not hold, the
            // run proved nothing about the clause, so it is `inconclusive`, never a failure.
            let rel =
                substitute(rel, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            return match check(&rel) {
                Err(StepError::Failed { received, .. })
                    if received == json!("the relation does not hold") =>
                {
                    Err(StepError::Inconclusive {
                        limit: "assumption".into(),
                    })
                }
                other => other,
            };
        }
        if let Some(rel) = obj.get("check") {
            let rel =
                substitute(rel, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            return check(&rel);
        }
        if let Some(spec) = obj.get("poll_events") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let max = spec.get("max").and_then(Value::as_u64).unwrap_or(64) as usize;
            let h = self.handle(&name)?;
            let got: Vec<Value> = h
                .core
                .poll_events(max)
                .iter()
                .map(|e| serde_json::to_value(e).expect("json"))
                .collect();
            for e in &got {
                self.check_forbidden(&name, e)?;
            }
            let expected = step
                .get("expect")
                .ok_or_else(|| bad("poll_events needs 'expect'"))?;
            return matches(
                expected,
                Some(&Value::Array(got.clone())),
                bindings,
                &NoSchemas,
            )
            .map_err(|f| {
                failed(
                    expected.clone(),
                    json!({ "events": got, "reason": f.describe() }),
                )
            });
        }
        if let Some(spec) = obj.get("call") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let method = spec
                .get("method")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("call needs 'method'"))?;
            let args = substitute(&spec.get("args").cloned().unwrap_or(json!({})), bindings)
                .map_err(|v| bad(format!("unbound variable {v}")))?;
            let result = self.call(&name, method, &args)?;
            return match result {
                Ok(v) => {
                    bind_result(step, &v, bindings)?;
                    match step.get("expect") {
                        Some(m) => matches(m, Some(&v), bindings, &NoSchemas).map_err(|f| {
                            failed(
                                m.clone(),
                                json!({ "call": method, "result": v, "reason": f.describe() }),
                            )
                        }),
                        None if step.get("expect_error").is_some() => {
                            Err(failed(step["expect_error"].clone(), v))
                        }
                        None => Ok(()),
                    }
                }

                Err(e) => self.error_result(e, step.get("expect_error"), Some(bindings)),
            };
        }
        if let Some(spec) = obj.get("advance_clock") {
            self.clock.elapsed += spec.get("ms").and_then(duration_of).unwrap_or_default();
            // Core TM-1: the unix clock stamps `at` fields only. It may jump by itself, forward or back.
            if let Some(unix) = spec.get("unix").and_then(Value::as_u64) {
                self.clock.unix_jump = Some((unix, self.clock.elapsed));
            }
            return Ok(());
        }
        if let Some(spec) = obj.get("poll_find") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let matcher = spec
                .get("event")
                .ok_or_else(|| bad("poll_find needs 'event'"))?;
            // Nothing pumps here, so the queue only empties; a poll batch is not the whole queue (Core A5-2).
            while self.fill(&name)? {}
            let queued: Vec<Value> = self.handle(&name)?.events.iter().cloned().collect();
            for (i, e) in queued.iter().enumerate() {
                let mut attempt = bindings.clone();
                if matches(matcher, Some(e), &mut attempt, &NoSchemas).is_ok() {
                    *bindings = attempt;
                    self.handle(&name)?.events.remove(i);
                    return Ok(());
                }
            }
            return Err(failed(matcher.clone(), json!({ "queued": queued })));
        }
        if let Some(spec) = obj.get("real_wait") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let interval = spec
                .get("ms")
                .and_then(duration_of)
                .ok_or_else(|| bad("real_wait needs 'ms'"))?;
            // timer: deadline — the real interval of the observation (Core SV-8, AD-7); the wait is on the wake handle, never a polling period
            let end = Instant::now() + interval;
            loop {
                self.pump(&name)?;
                self.fill(&name)?;
                let now = Instant::now();
                if now >= end {
                    return Ok(());
                }
                if deadline.expired() {
                    return Err(StepError::Inconclusive {
                        limit: "step_timeout".into(),
                    });
                }
                let mut left = end - now;
                if let Some(step_left) = deadline.remaining() {
                    left = left.min(step_left);
                }
                let wake = self.handle(&name)?.core.wake_handle();
                wake.wait(left);
            }
        }
        if let Some(spec) = obj.get("wait_wake") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let timeout = spec
                .get("timeout_ms")
                .and_then(duration_of)
                .unwrap_or_default();
            let wake = self.handle(&name)?.core.wake_handle();
            let got = if spec.get("other_thread") == Some(&Value::Bool(true)) {
                // Core TH-2: `wait` runs on any thread while the owner thread holds the handle.
                std::thread::scope(|s| s.spawn(|| wake.wait(timeout)).join())
                    .map_err(|_| bad("the waiting thread panicked"))?
            } else {
                wake.wait(timeout)
            };
            let got = serde_json::to_value(got).map_err(|e| bad(e.to_string()))?;
            let expected = step.get("expect").cloned().unwrap_or(json!("Woken"));
            return matches(&expected, Some(&got), bindings, &NoSchemas)
                .map_err(|f| failed(expected, json!({ "wake": got, "reason": f.describe() })));
        }
        if obj.contains_key("control") {
            // Core A5-3: a control that the testkit refuses to script (a code outside the call's sync column) answers with a typed
            // refusal, and `expect_refused` matches that value. Any other failure of the control fails the step.
            if let Some(expected) = step.get("expect_refused") {
                return self.expect_refused(step, expected, bindings);
            }
            if let Some(result) = self.core_control(step, bindings) {
                let value = result?;
                bind_result(step, &value, bindings)?;
                if let Some(m) = step.get("expect") {
                    matches(m, Some(&value), bindings, &NoSchemas).map_err(|f| {
                        failed(
                            m.clone(),
                            json!({ "control": value, "reason": f.describe() }),
                        )
                    })?;
                }
                return Ok(());
            }
        }
        if let Some(spec) = obj.get("forbid_events") {
            // Core A5-2, E2-3: from here on, an event that matches one of these fails the step that polls it, so a quiet check
            // stays active through every wait in between. `clear` ends it.
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            if spec.get("clear") == Some(&Value::Bool(true)) {
                self.forbidden.remove(&name);
                return Ok(());
            }
            let events = substitute(spec.get("events").unwrap_or(&json!([])), bindings)
                .map_err(|v| bad(format!("unbound variable {v}")))?;
            let list = events
                .as_array()
                .ok_or_else(|| bad("forbid_events takes a list of matchers"))?;
            self.forbidden
                .entry(name.clone())
                .or_default()
                .extend(list.iter().cloned());
            // An event that an earlier wait already polled and that no step has taken yet is buffered: it is checked now, so a wait
            // that returned with extra events in its batch cannot hide one from the observer that starts after it.
            let buffered: Vec<Value> = self.handle(&name)?.events.iter().cloned().collect();
            for e in &buffered {
                self.check_forbidden(&name, e)?;
            }
            return Ok(());
        }
        if let Some(spec) = obj.get("tap_drain") {
            // Core TP-1: the host drains the tap with `tap_read`. The contract fixes neither the number of reads nor their sizes,
            // and an empty read is no barrier: tap bytes travel the worker's control link with flow control, so more can come. With
            // `accounted: N` the drain goes on, across pumps and wakes, until the bytes that it read plus the bytes that were
            // dropped before them are N; without it, the drain ends at the first empty read (a tap that was never enabled).
            // The result is `{bytes, dropped, instances}`: the bytes in order, the sum of `dropped_before`, and the instance that
            // each chunk named (one entry for each chunk with bytes, in order).
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let max = spec.get("max").and_then(Value::as_u64).unwrap_or(65536);
            let accounted = spec.get("accounted").and_then(Value::as_u64);
            let args = substitute(
                &json!({ "session": spec.get("session"), "max": max }),
                bindings,
            )
            .map_err(|v| bad(format!("unbound variable {v}")))?;
            let mut bytes = String::new();
            let mut dropped = 0u64;
            let mut instances: Vec<Value> = vec![];
            // A first stall is a retry (the pump may have moved bytes); the second in a row, with Core idle, is a failure.
            let mut stalled = false;
            for _ in 0..poll_limit(deadline) {
                if deadline.expired() {
                    return Err(StepError::Inconclusive {
                        limit: "step_timeout".into(),
                    });
                }
                let chunk = self
                    .call(&name, "tap_read", &args)?
                    .map_err(|e| failed(json!("a chunk"), json!({ "error": e.detail })))?;
                let hex = chunk["bytes"]["$bytes_hex"].as_str().unwrap_or_default();
                dropped += chunk["dropped_before"].as_u64().unwrap_or(0);
                if !hex.is_empty() {
                    stalled = false;
                    instances.push(chunk["instance"].clone());
                    bytes.push_str(hex);
                    continue;
                }
                let have = (bytes.len() / 2) as u64 + dropped;
                match accounted {
                    Some(n) if have < n => {
                        // Not all accounted for: Core has more to hand over. A pump moves it, and an idle Core waits on its wake.
                        let report = self.pump(&name)?;
                        if report.more || report.events_posted > 0 {
                            continue;
                        }
                        match self.idle_wait(&name, deadline)? {
                            Poll::Pending => continue,
                            Poll::Idle if !stalled => stalled = true,
                            Poll::Idle => {
                                return Err(failed(
                                    json!({ "accounted": n }),
                                    json!({ "bytes": bytes.len() / 2, "dropped": dropped, "reason": "the tap has nothing more" }),
                                ))
                            }
                            // The step ran out of time (or a route closed under it): the run proved nothing.
                            _ => {
                                return Err(StepError::Inconclusive {
                                    limit: "step_timeout".into(),
                                })
                            }
                        }
                    }
                    _ => {
                        let value = json!({ "bytes": { "$bytes_hex": bytes }, "dropped": dropped, "instances": instances });
                        return bind_result(step, &value, bindings);
                    }
                }
            }
            return Err(StepError::Inconclusive {
                limit: "max_polls".into(),
            });
        }
        if let Some(spec) = obj.get("let") {
            return let_step(spec, bindings);
        }
        if let Some(spec) = obj.get("collect_events") {
            let name = spec
                .get("handle")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_HANDLE)
                .to_string();
            let events = self.collect_events(&name, deadline)?;
            return bind_result(step, &events, bindings);
        }
        // Suite statements (design 5.5; Core A5-1, A5-3, A5-4): the harness runs them and reports, and the driver checks the report.
        for statement in STATEMENTS {
            if let Some(spec) = obj.get(*statement) {
                let spec =
                    substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
                return match self.harness.statement(statement, &spec) {
                    Err(ControlError::Unsupported) => Err(StepError::Unsupported {
                        op: (*statement).into(),
                    }),
                    Err(ControlError::Bad(why)) => Err(bad(format!("{statement}: {why}"))),
                    Err(ControlError::Refused(v)) => Err(failed(json!("a report"), v)),
                    Ok(report) => check_statement(statement, &spec, &report),
                };
            }
        }
        if let Some(spec) = obj.get("attach_route") {
            // The options may name values that earlier steps bound (a terminal format that an earlier read reported).
            let spec =
                substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            return self.attach(&spec, step, bindings);
        }
        if let Some(what) = obj.get("type_check").and_then(Value::as_str) {
            // Core TH-1: a property of the concrete Core type.
            return match (what, self.harness.core_is_send_not_sync()) {
                ("send_not_sync", Some(true)) => Ok(()),
                ("send_not_sync", Some(false)) => Err(failed(
                    json!("Core is Send and not Sync"),
                    json!("the type is not Send, or it is Sync"),
                )),
                ("a6_2_deferred_set", _) => a6_2_deferred_set(DEFERRED_TXT)
                    .map_err(|why| failed(json!("the deferred set of Core A6-2"), json!(why))),
                _ => Err(StepError::Unsupported {
                    op: "type_check".into(),
                }),
            };
        }
        if let Some(spec) = obj.get("route_flood") {
            return self.route_flood(spec, step, bindings, deadline);
        }
        if let Some(spec) = obj.get("route_expect_refusals") {
            return self.route_expect_refusals(spec, deadline);
        }
        // Everything else is a route step of the codec suite.
        self.routes.exec(step, bindings, deadline)
    }

    fn next(&mut self, scope: &Value, deadline: &Deadline) -> Poll {
        match scope {
            // A route name: route frames. A route that has nothing to read yet may need a pump (Core OU-9, OR-1).
            Value::String(route) => {
                match self.routes.next(scope, deadline) {
                    // The transport reaches the worker in a pump (Core OU-9, OR-1): one pump per route. After that the data plane needs none (Core TH-3).
                    Poll::Idle if self.handed_off.insert(route.clone()) => {
                        let name = self.route_handle.clone();
                        if self.pump(&name).is_err() {
                            return Poll::Idle;
                        }
                        Poll::Pending
                    }
                    other => other,
                }
            }
            _ => {
                let name = scope
                    .get("handle")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_HANDLE)
                    .to_string();
                self.next_event(&name, deadline).unwrap_or(Poll::Idle)
            }
        }
    }
}

/// The statement steps that a harness runs (Core A5-1, A5-3, A5-4).
const STATEMENTS: &[&str] = &[
    "run_suite",
    "check_crates",
    "error_codes_reachable",
    "run_deterministic",
];

/// Checks the report that a harness gave for a statement against what the statement says.
///
/// - `run_suite {subject, harnesses, filter?, under?}`: the report has one run for each named harness. Each run names the ids that it
///   selected and the ids that passed: they are the same, at least one, none failed, and for the Core suite they are every transcript
///   of the selection (a statement transcript is not selected). Every harness selected the same ids (A5-4). It names every `under`
///   variation (seeds, shuttle).
/// - `check_crates {crate, not_in, no_dependency_of}`: the crate is in none of the facades and no listed crate depends on it.
/// - `error_codes_reachable {..}`: every code was reached at its timing, and the report lists exactly the codes that the statement says
///   have no scenario or wait for a steward ruling (`open_steward`).
/// - `run_deterministic {runs, steps}`: every run gave the same events once each instance is replaced by its first-appearance number,
///   and in each run two sessions never share an instance (Core ID-2, EV-9).
fn check_statement(kind: &str, spec: &Value, report: &Value) -> Result<(), StepError> {
    let fail = |why: &str| Err(failed(json!(why), report.clone()));
    let list = |v: &Value, k: &str| v.get(k).and_then(Value::as_array).cloned();
    match kind {
        "run_suite" => {
            let runs = list(report, "runs").ok_or_else(|| bad("the report has no `runs`"))?;
            // The ids that the statement selects. For the Core suite the driver knows them: every transcript of the subject with the
            // filter prefix, except the transcripts that are statements themselves (design 5.5: a suite statement runs the others).
            let filter = spec
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("conf::");
            let expected: Option<Vec<String>> =
                (spec.get("subject") == Some(&json!("core"))).then(|| {
                    // The file name of a Core transcript is its id. Only the files of the selection are parsed: parsing the whole suite
                    // for every statement made this check grow with the suite.
                    let mut ids: Vec<String> = crate::CORE_TRANSCRIPTS
                        .files()
                        .filter(|f| {
                            f.path()
                                .file_name()
                                .and_then(|n| n.to_str())
                                .is_some_and(|n| n.starts_with(filter) && n.ends_with(".json"))
                        })
                        .filter_map(|f| serde_json::from_slice::<Value>(f.contents()).ok())
                        .filter(|t| {
                            !t.get("steps")
                                .and_then(Value::as_array)
                                .is_some_and(|steps| {
                                    steps
                                        .iter()
                                        .any(|s| STATEMENTS.iter().any(|k| s.get(*k).is_some()))
                                })
                        })
                        .filter_map(|t| t.get("id").and_then(Value::as_str).map(str::to_string))
                        .collect();
                    ids.sort();
                    ids
                });
            let ids_of = |r: &Value, k: &str| -> Option<Vec<String>> {
                let mut v: Vec<String> = r
                    .get(k)?
                    .as_array()?
                    .iter()
                    .map(|x| x.as_str().map(str::to_string))
                    .collect::<Option<_>>()?;
                v.sort();
                Some(v)
            };
            let mut selected_sets: Vec<Vec<String>> = vec![];
            for h in list(spec, "harnesses").unwrap_or_default() {
                // One run for the harness, with the ids that it selected and the ids that passed: they are the same ids, there is at
                // least one, none failed, and (for the Core suite) they are every transcript that the statement selects.
                let ok = runs
                    .iter()
                    .find(|r| r.get("harness") == Some(&h))
                    .and_then(|r| {
                        let selected = ids_of(r, "selected")?;
                        let passed = ids_of(r, "passed")?;
                        let clean = r.get("failed") == Some(&json!([]));
                        let all = expected.as_ref().is_none_or(|e| *e == selected);
                        (clean && !selected.is_empty() && selected == passed && all)
                            .then_some(selected)
                    });
                match ok {
                    Some(selected) => selected_sets.push(selected),
                    None => {
                        return fail(&format!(
                            "a run on {h} that selected ids, passed all of them, failed none and covered every transcript of the selection"
                        ))
                    }
                }
            }
            // Both harnesses run the SAME suite (Core A5-4).
            if !selected_sets.windows(2).all(|w| w[0] == w[1]) {
                return fail("the same ids on every harness");
            }
            for u in list(spec, "under").unwrap_or_default() {
                if !list(report, "under").is_some_and(|a| a.contains(&u)) {
                    return fail(&format!("a run under {u}"));
                }
            }
            Ok(())
        }
        "check_crates" => {
            if report.get("found_in") == Some(&json!([]))
                && report.get("depended_on_by") == Some(&json!([]))
            {
                Ok(())
            } else {
                fail("the crate is in no facade and no listed crate depends on it")
            }
        }
        "error_codes_reachable" => {
            let unreached = report.get("not_reached") == Some(&json!([]));
            let listed = report.get("no_scenario") == spec.get("no_scenario")
                && report.get("open_steward") == spec.get("open_steward");
            if unreached && listed {
                Ok(())
            } else {
                fail("every code reached, and the codes with no scenario listed as the statement lists them")
            }
        }
        "run_deterministic" => {
            let runs = list(report, "runs").ok_or_else(|| bad("the report has no `runs`"))?;
            let want = spec.get("runs").and_then(Value::as_u64).unwrap_or(2) as usize;
            if runs.len() != want || want < 2 {
                return fail("the requested number of fresh runs, at least two");
            }
            let normal: Vec<Value> = runs
                .iter()
                .map(normalize_instances)
                .collect::<Result<_, _>>()?;
            if normal.windows(2).all(|w| w[0] == w[1]) {
                Ok(())
            } else {
                Err(failed(json!("the same events in every run"), json!(normal)))
            }
        }
        _ => Err(bad(format!("unknown statement {kind}"))),
    }
}

/// Replaces each `instance` by the number of its first appearance in the run. An instance names one session, and a session has one
/// instance (Core ID-2, EV-9): a value that names two sessions fails.
pub fn normalize_instances(run: &Value) -> Result<Value, StepError> {
    fn walk(v: &mut Value, seen: &mut Vec<(String, Option<String>)>) -> Result<(), StepError> {
        match v {
            Value::Object(m) => {
                let id = m.get("id").and_then(Value::as_str).map(str::to_string);
                if let Some(Value::String(inst)) = m.get("instance").cloned() {
                    let n = match seen.iter().position(|(i, _)| *i == inst) {
                        Some(n) => {
                            if seen[n].1 != id && id.is_some() && seen[n].1.is_some() {
                                return Err(failed(
                                    json!("one instance for one session"),
                                    json!({ "instance": inst, "sessions": [seen[n].1, id] }),
                                ));
                            }
                            n
                        }
                        None => {
                            seen.push((inst, id));
                            seen.len() - 1
                        }
                    };
                    m.insert("instance".into(), json!(format!("instance#{n}")));
                }
                for x in m.values_mut() {
                    walk(x, seen)?;
                }
            }
            Value::Array(a) => {
                for x in a {
                    walk(x, seen)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut v = run.clone();
    walk(&mut v, &mut vec![])?;
    Ok(v)
}

/// Runs a script of Core steps on a fresh harness and returns the events of its default handle, in the order that Core returned them
/// (Core A5-1: the same script, seed and inputs give the same events). A harness runs `run_deterministic` with this on `runs` fresh
/// harnesses that it builds from the same seed.
pub fn run_script_events(
    harness: Box<dyn CoreHarness>,
    steps: &[Value],
) -> Result<Value, StepError> {
    let mut driver = CoreDriver::new(harness);
    let mut bindings = Bindings::new();
    let deadline = Deadline::none();
    for step in steps {
        driver.exec(step, &mut bindings, &deadline)?;
    }
    let collect = json!({ "collect_events": {}, "bind": "$events" });
    driver.exec(&collect, &mut bindings, &deadline)?;
    Ok(bindings.remove("events").unwrap_or(Value::Null))
}

fn merge_expect(spec: &Value, step: &Value) -> Value {
    let mut spec = spec.clone();
    if let (Some(m), Some(e)) = (spec.as_object_mut(), step.get("expect_error")) {
        m.insert("expect_error".into(), e.clone());
    }
    spec
}

impl CoreDriver {
    /// `route_flood`: `{"route_flood": {"route", "max"}, "bind": "$n"}`. The client sends `text` frames with an invalid UTF-8 byte and the ops
    /// 1, 2, 3, and so on, each of which the worker refuses (Core DP-5, with `unsupported{utf8}`), and it stops at the first frame after
    /// which the control `input_blocked` of the route says that the stream takes no more bytes. It binds the number of frames sent. The
    /// load that fills the queue is thus the one that the queue's own bound and the refusal frames' own sizes make (Core DP-5: the worker
    /// stops reading the route's input when its queue cannot hold one more frame), never a number that a transcript guesses. A flood that
    /// has not blocked the stream after `max` frames is inconclusive: `max` is a limit of the runner.
    fn route_flood(
        &mut self,
        spec: &Value,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let route = spec.get("route").and_then(Value::as_str).unwrap_or("r1");
        let max = spec.get("max").and_then(Value::as_u64).unwrap_or(100_000);
        for n in 1..=max {
            let hex = format!("84{n:016x}ff");
            self.routes.exec(
                &json!({ "route_send_raw": { "bytes_hex": hex }, "route": route }),
                bindings,
                deadline,
            )?;
            let probe = json!({ "control": { "op": "input_blocked" }, "route": route, "expect": { "blocked": true } });
            match self.routes.exec(&probe, bindings, deadline) {
                Ok(()) => {
                    if let Some(var) = step.get("bind").and_then(Value::as_str) {
                        bindings.insert(var.trim_start_matches('$').to_string(), json!(n));
                    }
                    return Ok(());
                }
                Err(StepError::Failed { .. }) => {}
                Err(other) => return Err(other),
            }
        }
        // The maximum is a limit of the runner (design 6.1), not a bound of the contract: a flood that has not blocked the stream proves
        // nothing about the worker.
        Err(StepError::Inconclusive {
            limit: "route_flood_max".into(),
        })
    }

    /// `route_expect_refusals`: `{"route_expect_refusals": {"route", "count", "reason"}}`. The next `count` refusals of the route are the
    /// ops 1 to `count`, in that order, each with the reason: none was dropped or reordered (Core DP-5: the exception frames wait in the
    /// route's queue). The other frames of the route (`modes`, `output`) pass.
    fn route_expect_refusals(
        &mut self,
        spec: &Value,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let route = spec.get("route").and_then(Value::as_str).unwrap_or("r1");
        let count = spec
            .get("count")
            .and_then(Value::as_u64)
            .ok_or_else(|| bad("route_expect_refusals needs a numeric 'count'"))?;
        let reason = spec
            .get("reason")
            .cloned()
            .unwrap_or(json!({ "$any": true }));
        let scope = json!(route);
        let mut next = 1;
        // An idle route is not the end of its frames, and neither is an idle host: only the close of the route, which is an event of the
        // route, says that no refusal will arrive. Anything else that gives no frame is the runner's limit (design 6.1: inconclusive).
        if count == 0 {
            return Ok(());
        }
        // The route gave nothing after a pump of the handle: a second idle read in a row is the end of what the boundary gives.
        let mut stuck = false;
        for _ in 0..poll_limit(deadline) {
            match self.next(&scope, deadline) {
                Poll::Item(frame) => {
                    stuck = false;
                    if frame["frame"] != json!("input_refused") {
                        continue;
                    }
                    let want = json!({ "frame": "input_refused", "op": next.to_string(), "reason": reason });
                    matches(&want, Some(&frame), &mut Bindings::new(), &NoSchemas).map_err(
                        |f| {
                            failed(
                                want.clone(),
                                json!({ "frame": frame, "reason": f.describe() }),
                            )
                        },
                    )?;
                    next += 1;
                    if next > count {
                        return Ok(());
                    }
                }
                Poll::Pending => {}
                Poll::Idle => {
                    // The route boundary has nothing readable now. The host's wake handle is not the route's readiness event (Core TM-3:
                    // it fires for control-link input; TH-3: route I/O is in the worker), and an idle host does not say that no frame is
                    // coming. So the handle is pumped, for the in-process edges that deliver in a pump, and the read is tried again while
                    // that made progress. When it made none, the boundary has given nothing within the runner's limit, and the step proves
                    // nothing: inconclusive, never a failure of the worker. A read that waits on the stream's own readiness, within the
                    // step deadline, is the adapter's (`RouteClient::read`).
                    if stuck {
                        return Err(StepError::Inconclusive {
                            limit: "route_idle".into(),
                        });
                    }
                    let name = self.route_handle.clone();
                    self.pump(&name)?;
                    stuck = true;
                }
                Poll::Closed(why) => {
                    return Err(failed(
                        json!({ "refusal_op": next }),
                        json!({ "the route closed before the refusal": why }),
                    ));
                }
                Poll::TimedOut => {
                    return Err(StepError::Inconclusive {
                        limit: "step_timeout".into(),
                    })
                }
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Pumps and polls until an event matches. The events before it are not checked: the step sets no pump count (design 5.5).
    fn pump_until(
        &mut self,
        spec: &Value,
        _step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let name = spec
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        let matcher = spec
            .get("event")
            .ok_or_else(|| bad("pump_until needs 'event'"))?;
        for _ in 0..poll_limit(deadline) {
            match self.next_event(&name, deadline)? {
                Poll::Item(e) => {
                    let mut attempt = bindings.clone();
                    if matches(matcher, Some(&e), &mut attempt, &NoSchemas).is_ok() {
                        *bindings = attempt;
                        return Ok(());
                    }
                }
                Poll::Pending => {}
                Poll::Idle => {
                    return Err(failed(
                        matcher.clone(),
                        json!("no matching event: the Core is idle"),
                    ))
                }
                Poll::TimedOut => {
                    return Err(StepError::Inconclusive {
                        limit: "step_timeout".into(),
                    })
                }
                Poll::Closed(_) => return Err(failed(matcher.clone(), json!("closed"))),
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }
}

/// The most loops of a waiting step. With a step deadline the deadline ends the wait; without one (a fake) a bound stops a fake bug (design 6.1).
fn poll_limit(deadline: &Deadline) -> usize {
    if deadline.remaining().is_some() {
        usize::MAX
    } else {
        10_000
    }
}

/// Binds the result of a call, control or pump: `bind` takes the whole value, and `bind_fields` takes one variable for each named field.
fn bind_result(step: &Value, value: &Value, bindings: &mut Bindings) -> Result<(), StepError> {
    if let Some(var) = step.get("bind").and_then(Value::as_str) {
        bindings.insert(var.trim_start_matches('$').to_string(), value.clone());
    }
    if let Some(fields) = step.get("bind_fields").and_then(Value::as_object) {
        for (field, var) in fields {
            let var = var
                .as_str()
                .ok_or_else(|| bad("bind_fields names variables"))?;
            let got = value
                .get(field)
                .ok_or_else(|| bad(format!("the result has no field '{field}'")))?;
            bindings.insert(var.trim_start_matches('$').to_string(), got.clone());
        }
    }
    Ok(())
}

/// `{"let": {"$x": {"<op>": [args]}}}`: binds a value computed from bound values. Ops: `add` and `sub` (two numbers), `eq` (two values,
/// gives a boolean), `contains` (an array and an item, gives a boolean), `field` (an object and a name) and `str` (a number and the
/// number again: the decimal string, for a route op id that equals a bound host `OpId`). It states a relation
/// between values that the contract fixes, so that a transcript never writes a count that the contract leaves open (design 5.6).
fn let_step(spec: &Value, bindings: &mut Bindings) -> Result<(), StepError> {
    let map = spec.as_object().ok_or_else(|| bad("let takes an object"))?;
    for (var, expr) in map {
        let expr = substitute(expr, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
        let (op, args) = expr
            .as_object()
            .and_then(|o| o.iter().next())
            .ok_or_else(|| bad("a let expression has one operation"))?;
        let args = args
            .as_array()
            .ok_or_else(|| bad("a let operation takes an array"))?;
        let one = [
            "rtrim_spaces",
            "record_payloads",
            "sum_hex_bytes",
            "utf8_len",
        ];
        if args.len() != if one.contains(&op.as_str()) { 1 } else { 2 } {
            return Err(bad(format!(
                "the let operation '{op}' has the wrong number of arguments"
            )));
        }
        let value = match op.as_str() {
            // A relation of two numbers as a value (Core IN-8: a paste is wrapped when the mode is on at the start of the write).
            "gt" | "lt" => {
                let (Some(a), Some(b)) = (number(&args[0]), number(&args[1])) else {
                    return Err(bad("gt and lt take numbers"));
                };
                json!(if op == "gt" { a > b } else { a < b })
            }
            // Core ST-3: `row_text` has every trailing U+0020 removed (the empty cells of a row read as spaces).
            "rtrim_spaces" => json!(args[0]
                .as_str()
                .ok_or_else(|| bad("rtrim_spaces takes a string"))?
                .trim_end_matches(' ')),
            // Core ST-7: the original payloads (hex) of the records of an `input_records` list, in the order of the list.
            "record_payloads" => Value::Array(
                args[0]
                    .as_array()
                    .ok_or_else(|| bad("record_payloads takes the input_records list"))?
                    .iter()
                    .filter_map(|e| e.get("record"))
                    .map(|r| {
                        r.get("payload")
                            .and_then(|p| p.get("$bytes_hex"))
                            .cloned()
                            .unwrap_or(json!(""))
                    })
                    .collect(),
            ),
            // The number of bytes in a list of hex strings.
            "sum_hex_bytes" => json!(args[0]
                .as_array()
                .ok_or_else(|| bad("sum_hex_bytes takes a list of hex strings"))?
                .iter()
                .map(|h| hex_of(h).map_or(0, |h| h.len() / 2))
                .sum::<usize>()),
            // The number of UTF-8 bytes of a string.
            "utf8_len" => json!(args[0]
                .as_str()
                .ok_or_else(|| bad("utf8_len takes a string"))?
                .len()),
            // The concatenation of two hex values, for a drained tap.
            "hex_concat" => {
                let (Some(a), Some(b)) = (hex_of(&args[0]), hex_of(&args[1])) else {
                    return Err(bad("hex_concat takes two hex values"));
                };
                json!({ "$bytes_hex": format!("{a}{b}") })
            }
            "add" | "sub" => {
                let (Some(a), Some(b)) = (number(&args[0]), number(&args[1])) else {
                    return Err(bad("add and sub take numbers"));
                };
                let n = if op == "add" { a + b } else { a - b };
                // The result keeps its sign and magnitude: a value that fits neither i64 nor u64 is outside JSON's integers, never wrapped.
                if let Ok(v) = i64::try_from(n) {
                    json!(v)
                } else if let Ok(v) = u64::try_from(n) {
                    json!(v)
                } else {
                    return Err(bad(format!("{n} is outside the integers of JSON")));
                }
            }
            "eq" => json!(args[0] == args[1]),
            "str" => {
                let n = number(&args[0]).ok_or_else(|| bad("str takes a number"))?;
                json!(n.to_string())
            }
            // Two strings joined: a host path and a space, for the expected paste of a path (Core DP-5b).
            "concat" => {
                let (Some(a), Some(b)) = (args[0].as_str(), args[1].as_str()) else {
                    return Err(bad("concat takes two strings"));
                };
                json!(format!("{a}{b}"))
            }
            // A string repeated n times (n from the observed file-name limit): a name that is longer than the filesystem allows (Core A2-9).
            "repeat" => {
                let (Some(a), Some(n)) = (args[0].as_str(), number(&args[1])) else {
                    return Err(bad("repeat takes a string and a count"));
                };
                let n =
                    usize::try_from(n).map_err(|_| bad("repeat takes a count from 0 to 1 MiB"))?;
                if n > (1 << 20) {
                    return Err(bad("repeat takes a count from 0 to 1 MiB"));
                }
                json!(a.repeat(n))
            }
            // The length in bytes of a string (the second argument is not used): the directory in the worst-case size of a path paste.
            "str_len" => {
                let a = args[0]
                    .as_str()
                    .ok_or_else(|| bad("str_len takes a string"))?;
                json!(a.len())
            }
            // The length in bytes of a hex value (the second argument is not used): the markers of an encoded paste.
            "hex_len" => {
                let h = hex_of(&args[0]).ok_or_else(|| bad("hex_len takes a hex value"))?;
                json!(h.len() / 2)
            }
            "contains" => json!(args[0].as_array().is_some_and(|a| a.contains(&args[1]))),
            "field" => args[0]
                .get(args[1].as_str().unwrap_or_default())
                .cloned()
                .ok_or_else(|| bad("the value has no such field"))?,
            // An object without one field (absent or present): a comparison that a clause narrows by one field
            // (Core A18-1: `conf::ad_3_adopted_equals_started` compares `terminal_state` without `last_output_at`).
            "without" => {
                let (Some(obj), Some(name)) = (args[0].as_object(), args[1].as_str()) else {
                    return Err(bad("without takes an object and a field name"));
                };
                let mut obj = obj.clone();
                obj.remove(name);
                Value::Object(obj)
            }
            other => return Err(bad(format!("unknown let operation '{other}'"))),
        };
        bindings.insert(var.trim_start_matches('$').to_string(), value);
    }
    Ok(())
}

fn number(v: &Value) -> Option<i128> {
    v.as_i64()
        .map(i128::from)
        .or_else(|| v.as_u64().map(i128::from))
}

/// `conformance/deferred.txt`, as the repository holds it.
const DEFERRED_TXT: &str = include_str!("../../../conformance/deferred.txt");

/// The deferred ids that Core A6-2 enumerates, each with the start condition that the clause gives.
const A6_2_DEFERRED: [(&str, &str); 2] = [
    (
        "conf::ad_4_previous_worker_version_adopts",
        "the first release whose worker protocol T is 2",
    ),
    (
        "conf::ad_4_missing_worker_capability_is_unsupported",
        "the first release whose protocol T adds a worker feature that the pinned T-1 worker lacks",
    ),
];

/// Core A6-2: the deferred set (the lines of `deferred.txt` that are not `not-applicable`) is exactly the two enumerated ids, and each
/// carries its start condition after `until:`.
pub fn a6_2_deferred_set(text: &str) -> Result<(), String> {
    let mut found: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("not-applicable") {
            continue;
        }
        let fields: Vec<&str> = line
            .split("  ")
            .map(str::trim)
            .filter(|f| !f.is_empty())
            .collect();
        let until = fields
            .iter()
            .find_map(|f| f.strip_prefix("until:"))
            .unwrap_or("")
            .trim()
            .to_string();
        found.push((fields[0].to_string(), until));
    }
    let mut ids: Vec<&str> = found.iter().map(|(i, _)| i.as_str()).collect();
    ids.sort_unstable();
    let mut want: Vec<&str> = A6_2_DEFERRED.iter().map(|(i, _)| *i).collect();
    want.sort_unstable();
    if ids != want {
        return Err(format!(
            "the deferred ids are {ids:?}, and A6-2 enumerates {want:?}"
        ));
    }
    for (id, condition) in A6_2_DEFERRED {
        let until = &found
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, u)| u.as_str())
            .unwrap_or("");
        if !until.starts_with(condition) {
            return Err(format!(
                "{id} has the start condition '{until}', and A6-2 gives '{condition}'"
            ));
        }
    }
    Ok(())
}

/// The bytes of a value: `{"$bytes_hex": ".."}` or a hex string.
fn hex_of(v: &Value) -> Option<String> {
    v.get("$bytes_hex")
        .and_then(Value::as_str)
        .or_else(|| v.as_str())
        .map(str::to_string)
}

/// A relation between bound values (design 5.6 rule 3: a transcript states relations, never counts the contract leaves open).
fn check(rel: &Value) -> Result<(), StepError> {
    let obj = rel
        .as_object()
        .ok_or_else(|| bad("check takes an object"))?;
    let (op, args) = obj
        .iter()
        .next()
        .ok_or_else(|| bad("check needs a relation"))?;
    let args = args
        .as_array()
        .ok_or_else(|| bad("a relation takes an array"))?;
    // The argument count belongs to the relation: a wrong count is a transcript error, never a relation that does not hold.
    let arity = match op.as_str() {
        "mul_eq" => 3,
        "eq" | "ne" | "lt" | "le" | "gt" | "contains" | "subset_of" | "prefix_of" | "suffix_of"
        | "hex_len_eq" | "hex_prefix" | "hex_ends_with" => 2,
        other => return Err(bad(format!("unknown relation '{other}'"))),
    };
    if args.len() != arity {
        return Err(bad(format!("the relation '{op}' takes {arity} arguments")));
    }
    let nums = || -> Result<Vec<i128>, StepError> {
        args.iter()
            .map(|a| number(a).ok_or_else(|| failed(json!("numbers"), json!({ "check": rel }))))
            .collect()
    };
    let holds = match op.as_str() {
        "eq" => args.len() == 2 && args[0] == args[1],
        "ne" => args.len() == 2 && args[0] != args[1],
        "contains" => args.len() == 2 && args[0].as_array().is_some_and(|a| a.contains(&args[1])),
        "prefix_of" => {
            args.len() == 2
                && match (&args[0], &args[1]) {
                    (Value::String(a), Value::String(b)) => b.starts_with(a.as_str()),
                    (Value::Array(a), Value::Array(b)) => b.starts_with(a),
                    _ => false,
                }
        }
        "suffix_of" => {
            args.len() == 2
                && match (&args[0], &args[1]) {
                    (Value::String(a), Value::String(b)) => b.ends_with(a.as_str()),
                    (Value::Array(a), Value::Array(b)) => b.ends_with(a),
                    _ => false,
                }
        }
        "le" => matches!(nums()?[..], [a, b] if a <= b),
        "subset_of" => {
            args.len() == 2
                && matches!((args[0].as_array(), args[1].as_array()), (Some(a), Some(b)) if a.iter().all(|x| b.contains(x)))
        }
        "lt" => matches!(nums()?[..], [a, b] if a < b),
        "gt" => matches!(nums()?[..], [a, b] if a > b),
        "mul_eq" => matches!(nums()?[..], [a, k, b] if b == a * k),
        "hex_len_eq" => match (hex_of(&args[0]), number(&args[1])) {
            (Some(h), Some(n)) => (h.len() / 2) as i128 == n,
            _ => false,
        },
        "hex_prefix" => match (hex_of(&args[0]), hex_of(&args[1])) {
            (Some(p), Some(w)) => w.starts_with(&p),
            _ => false,
        },
        "hex_ends_with" => match (hex_of(&args[0]), hex_of(&args[1])) {
            (Some(w), Some(suffix)) => w.ends_with(&suffix),
            _ => false,
        },
        other => return Err(bad(format!("unknown relation '{other}'"))),
    };
    if holds {
        Ok(())
    } else {
        Err(failed(rel.clone(), json!("the relation does not hold")))
    }
}

impl CoreDriver {
    /// Pumps and polls until every matcher has matched a distinct event, in any order (design 5.3: no order is asserted).
    fn pump_collect(
        &mut self,
        spec: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let name = spec
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        let mut wanted: Vec<Value> = spec
            .get("events")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| bad("pump_collect needs 'events'"))?;
        let mut done: Vec<Value> = vec![];
        let exhaustive = spec.get("exhaustive") == Some(&Value::Bool(true));
        // `ignore`: events that an exhaustive collection lets pass (for example the `Activity` of the output that caused the events).
        let ignored: Vec<Value> = spec
            .get("ignore")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let is_ignored = |e: &Value| {
            ignored
                .iter()
                .any(|m| matches(m, Some(e), &mut Bindings::new(), &NoSchemas).is_ok())
        };
        let forbidden: Vec<Value> = spec
            .get("forbid")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for _ in 0..poll_limit(deadline) {
            if wanted.is_empty() {
                // Exhaustive: nothing else is queued, and nothing else comes once Core is at rest.
                if exhaustive {
                    self.pump_idle(&json!({ "handle": name }), deadline)?;
                    self.fill(&name)?;
                    while let Some(extra) = self.handle(&name)?.events.pop_front() {
                        // `forbid` is applied before `ignore`: an event that both lets pass and forbids is forbidden.
                        if let Some(m) = forbidden.iter().find(|m| {
                            matches(m, Some(&extra), &mut bindings.clone(), &NoSchemas).is_ok()
                        }) {
                            return Err(failed(
                                json!({ "forbidden": m }),
                                json!({ "received": extra }),
                            ));
                        }
                        if !is_ignored(&extra) {
                            return Err(failed(
                                json!("no further event"),
                                json!({ "unexpected": extra }),
                            ));
                        }
                    }
                }
                return Ok(());
            }
            match self.next_event(&name, deadline)? {
                Poll::Item(e) => {
                    if let Some(m) = forbidden
                        .iter()
                        .find(|m| matches(m, Some(&e), &mut bindings.clone(), &NoSchemas).is_ok())
                    {
                        return Err(failed(json!({ "forbidden": m }), json!({ "received": e })));
                    }
                    // An event that matches a matcher that already matched is a duplicate: it is never swallowed (Core AM-3).
                    if let Some(m) = done
                        .iter()
                        .find(|m| matches(m, Some(&e), &mut bindings.clone(), &NoSchemas).is_ok())
                    {
                        return Err(failed(m.clone(), json!({ "duplicate": e })));
                    }
                    let before = wanted.len();
                    for i in 0..wanted.len() {
                        let mut attempt = bindings.clone();
                        if matches(&wanted[i], Some(&e), &mut attempt, &NoSchemas).is_ok() {
                            *bindings = attempt;
                            done.push(wanted.remove(i));
                            break;
                        }
                    }
                    // `exhaustive`: an event that no matcher wants is a failure (it is how a transcript proves that nothing else was posted).
                    if exhaustive && wanted.len() == before && !is_ignored(&e) {
                        return Err(failed(json!(wanted), json!({ "unexpected": e })));
                    }
                }
                Poll::Pending => {}
                Poll::Idle => {
                    return Err(failed(
                        json!(wanted),
                        json!("the Core is idle with events still missing"),
                    ))
                }
                Poll::TimedOut => {
                    return Err(StepError::Inconclusive {
                        limit: "step_timeout".into(),
                    })
                }
                Poll::Closed(_) => return Err(failed(json!(wanted), json!("closed"))),
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Whether at least `min` bytes reached the PTY of a session (control `pty_input`).
    fn pty_has(&mut self, name: &str, session: &str, min: usize) -> Result<bool, StepError> {
        let log = self
            .harness
            .control(name, "pty_input", &json!({ "session": session }))
            .map_err(|_| StepError::Unsupported {
                op: "pty_input".into(),
            })?;
        Ok(hex_of(&log["bytes"]).is_some_and(|h| h.len() / 2 >= min))
    }

    /// Pumps until at least `min_bytes` reached the PTY of a session (control `pty_input`).
    fn await_pty(&mut self, spec: &Value, deadline: &Deadline) -> Result<(), StepError> {
        let name = spec
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        let session = spec
            .get("session")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("await_pty needs 'session'"))?;
        let min = spec.get("min_bytes").and_then(Value::as_u64).unwrap_or(1) as usize;
        // The condition is read before the first pump and again after each pump, before the host is asked whether it is idle: a worker PTY
        // write posts no host event and may leave nothing more to run, so a pump can satisfy the step and report no progress.
        if self.pty_has(&name, session, min)? {
            return Ok(());
        }
        for _ in 0..poll_limit(deadline) {
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
            // The bytes arrive in a pump or by a deadline: wait on those, never in a spin.
            let report = self.pump(&name)?;
            if self.pty_has(&name, session, min)? {
                return Ok(());
            }
            if report.events_posted == 0 && !report.more {
                // A wait that ended on the step deadline is caught by the check at the top of the loop.
                if matches!(self.idle_wait(&name, deadline)?, Poll::Idle) {
                    return Err(failed(
                        json!({ "pty_bytes_at_least": min }),
                        json!("the Core is idle"),
                    ));
                }
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// A control step with `expect_refused`: the harness must refuse the control with a typed value, and the value must match.
    fn expect_refused(
        &mut self,
        step: &Value,
        expected: &Value,
        bindings: &mut Bindings,
    ) -> Result<(), StepError> {
        let c = step.get("control").cloned().unwrap_or_default();
        let c = substitute(&c, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
        let op = c
            .get("op")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !self.harness.has_control(&op) {
            return Err(StepError::Unsupported { op });
        }
        let handle = c
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        match self.harness.control(&handle, &op, &c) {
            Err(ControlError::Refused(got)) => matches(expected, Some(&got), bindings, &NoSchemas)
                .map_err(|f| {
                    failed(
                        expected.clone(),
                        json!({ "refused": got, "reason": f.describe() }),
                    )
                }),
            Err(ControlError::Unsupported) => Err(StepError::Unsupported { op }),
            Err(ControlError::Bad(why)) => Err(failed(
                json!("a typed refusal"),
                json!({ "bad_arguments": why }),
            )),
            Ok(value) => Err(failed(json!("the control is refused"), value)),
        }
    }

    /// Pumps until no runnable work remains (`PumpReport.more` is false). It asserts no pump count (Core TM-6, design 5.5).
    fn pump_idle(&mut self, spec: &Value, deadline: &Deadline) -> Result<(), StepError> {
        let name = spec
            .get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string();
        for _ in 0..poll_limit(deadline) {
            if !self.pump(&name)?.more {
                return Ok(());
            }
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Brings the handle to rest and drops its events, with no check: it sets up a transcript that then looks only at later events. A poll
    /// that frees room unparks work (Core EV-5(d), TM-6), so it alternates pumps and polls until a pump reports no runnable work, posts
    /// no event, and the poll after it is empty. It asserts no pump or poll count; its limit is the runner's.
    fn drain(&mut self, name: &str, deadline: &Deadline) -> Result<(), StepError> {
        for _ in 0..poll_limit(deadline) {
            let report = self.pump(name)?;
            let polled = self.poll_all(name)?;
            if !report.more && report.events_posted == 0 && !polled {
                return Ok(());
            }
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Brings the handle to rest like `drain`, and returns the events it polled on the way, in the order that Core returned them
    /// (Core A5-1: the same script, seed and inputs give the same events). It asserts no pump or poll count.
    fn collect_events(&mut self, name: &str, deadline: &Deadline) -> Result<Value, StepError> {
        // The events that earlier waits polled and left in the buffer come first: they are Core's events too, in Core's order.
        let mut all: Vec<Value> = self.handle(name)?.events.drain(..).collect();
        for _ in 0..poll_limit(deadline) {
            let report = self.pump(name)?;
            let h = self.handle(name)?;
            let polled: Vec<Value> = h
                .core
                .poll_events(1024)
                .iter()
                .map(|e| serde_json::to_value(e).expect("json"))
                .collect();
            let any = !polled.is_empty();
            for e in &polled {
                self.check_forbidden(name, e)?;
            }
            all.extend(polled);
            if !report.more && report.events_posted == 0 && !any {
                return Ok(Value::Array(all));
            }
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Polls and drops every queued event without a pump. Returns whether any came.
    fn poll_all(&mut self, name: &str) -> Result<bool, StepError> {
        self.handle(name)?.events.clear();
        let mut any = false;
        // The queue is bounded, so it empties; the limit is the runner's, never a count of the contract.
        for _ in 0..10_000 {
            let polled = self.handle(name)?.core.poll_events(1024);
            if polled.is_empty() {
                return Ok(any);
            }
            for e in &polled {
                let v = serde_json::to_value(e).map_err(|e| bad(e.to_string()))?;
                self.check_forbidden(name, &v)?;
            }
            any = true;
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Waits on the wake handle for work that an edge will announce, with the injected clock left where it is: a fence must not move time (an
    /// armed deadline, such as a capture's expiry, would pass inside it). The wait is bounded by the step deadline; with none it returns at
    /// once, and the caller's loop is bounded by the runner limit (design 6.1).
    fn wait_for_edge(&mut self, name: &str, deadline: &Deadline) -> Result<(), StepError> {
        let timeout = deadline.remaining().unwrap_or(Duration::ZERO);
        let wake = self.handle(name)?.core.wake_handle();
        let _ = wake.wait(timeout);
        Ok(())
    }

    /// Pumps until nothing is runnable and the edges hold no undelivered report (control `edges_quiet`). It never polls, so the queued events
    /// stay for the transcript, and it never moves the clock. An edge that does not report ends the wait at the runner's limit (`inconclusive`),
    /// never as a failure: a wake that has not come proves no more than that.
    fn await_quiet(&mut self, spec: &Value, deadline: &Deadline) -> Result<(), StepError> {
        let name = Self::handle_of(spec);
        for _ in 0..poll_limit(deadline) {
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
            let report = self.pump(&name)?;
            let state = self
                .harness
                .control(&name, "edges_quiet", &json!({}))
                .map_err(|_| StepError::Unsupported {
                    op: "edges_quiet".into(),
                })?;
            if !report.more && state.get("quiet") == Some(&Value::Bool(true)) {
                return Ok(());
            }
            // A report from an edge announces itself on the wake handle (Core TM-6): wait on it, with the clock left where it is. Runnable work
            // has signaled the handle, so this returns at once.
            self.wait_for_edge(&name, deadline)?;
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    fn handle_of(spec: &Value) -> String {
        spec.get("handle")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_HANDLE)
            .to_string()
    }

    /// Whether a control answers as the matcher says now. A match binds its variables.
    fn control_matches(
        &mut self,
        name: &str,
        op: &str,
        control: &Value,
        expected: &Value,
        bindings: &mut Bindings,
    ) -> Result<bool, StepError> {
        let got = self
            .harness
            .control(name, op, control)
            .map_err(|_| StepError::Unsupported { op: op.to_string() })?;
        let mut attempt = bindings.clone();
        let done = matches(expected, Some(&got), &mut attempt, &NoSchemas).is_ok();
        if done {
            *bindings = attempt;
        }
        Ok(done)
    }

    /// Pumps (never polls, never moves the clock) until a control answers as the matcher says: the fence for one effect at an edge, for
    /// the cases where a state report is intentionally parked and `await_quiet` could not end (Core EV-5(b), TM-6). The condition is
    /// checked before the first pump and again after each pump, before the wait: a pump that satisfied it needs no wake (Core TM-6 clears
    /// the level signal when no runnable work is left).
    fn await_control(
        &mut self,
        spec: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let name = Self::handle_of(spec);
        let mut control = spec
            .get("control")
            .cloned()
            .ok_or_else(|| bad("await_control needs 'control'"))?;
        control =
            substitute(&control, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
        let op = control
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("await_control names the control's 'op'"))?
            .to_string();
        let expected = spec
            .get("expect")
            .ok_or_else(|| bad("await_control needs 'expect'"))?;
        if self.control_matches(&name, &op, &control, expected, bindings)? {
            return Ok(());
        }
        for _ in 0..poll_limit(deadline) {
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
            self.pump(&name)?;
            if self.control_matches(&name, &op, &control, expected, bindings)? {
                return Ok(());
            }
            self.wait_for_edge(&name, deadline)?;
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }

    /// Whether the cached `get` of a session matches now. A match binds its variables.
    fn get_matches(
        &mut self,
        name: &str,
        id: &SessionId,
        spec: &Value,
        bindings: &mut Bindings,
    ) -> Result<bool, StepError> {
        let got = self.handle(name)?.core.get(id);
        let mut attempt = bindings.clone();
        let done = match (&got, spec.get("expect"), spec.get("expect_error")) {
            (Ok(rec), Some(m), _) => matches(m, Some(&js(rec)), &mut attempt, &NoSchemas).is_ok(),
            (Err(e), _, Some(m)) => matches(m, Some(&js(e)), &mut attempt, &NoSchemas).is_ok(),
            _ => false,
        };
        if done {
            *bindings = attempt;
        }
        Ok(done)
    }

    /// Pumps (never polls, never moves the clock) until the cached `get` of a session matches: the fence for a lifecycle state. As
    /// `await_control`, it checks before the first pump and after each pump, before the wait.
    fn await_get(
        &mut self,
        spec: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let name = Self::handle_of(spec);
        let id = SessionId(
            spec.get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("await_get needs 'id'"))?
                .to_string(),
        );
        if self.get_matches(&name, &id, spec, bindings)? {
            return Ok(());
        }
        for _ in 0..poll_limit(deadline) {
            if deadline.expired() {
                return Err(StepError::Inconclusive {
                    limit: "step_timeout".into(),
                });
            }
            self.pump(&name)?;
            if self.get_matches(&name, &id, spec, bindings)? {
                return Ok(());
            }
            self.wait_for_edge(&name, deadline)?;
        }
        Err(StepError::Inconclusive {
            limit: "max_polls".into(),
        })
    }
}
