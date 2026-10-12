//! FakePlugin: a scripted plugin host process (HP section 19, steward ruling R-1.3).
//!
//! It answers `Hello` with `Ready`, runs handlers from a fixture script, makes host calls, crashes at chosen points, runs slow handlers and sends oversize frames.
//! It holds the protocol state of a host: the handshake, the epoch, the lanes, one active handler, the deadline it measures on its own clock, and its process memory.
//! The fixture is language-neutral JSON, so a real host implements the same fixture by name.
//!
//! Every branch cites its clause. Open orders are the named choice points of [`Choice`].

use crate::limits::{lane_caps, test_limits};
use crate::link::{
    lane_by_name, lane_name, lenient_id, Backpressured, Choice, Chooser, Inbound, Link,
};
use crate::{FakeEndpoint, Unsupported};
use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::VecDeque;

/// The native controls of FakePlugin (design 6.3).
pub const CONTROLS: &[&str] = &[
    "settle",
    "status",
    "counters",
    "received",
    "hold_lane",
    "release_lane",
    "reconnect",
    "restart_process",
    "memory",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Waiting for `Hello`.
    Idle,
    /// The entrypoint runs in the bootstrap phase (HP HS-7).
    Booting,
    /// `Ready` is sent; no application frame goes out until `Activated` (HP HS-2).
    AwaitActivated,
    Active,
    /// The host stopped: a `Fault`, a crash or a `Shutdown`.
    Dead,
}

/// The state that dies with the process and survives a re-adoption (HP CR-5).
#[derive(Default)]
struct Memory {
    invocations: u64,
    entrypoint_runs: u64,
}

struct Run {
    request: Option<WireId>,
    steps: Vec<Value>,
    pc: usize,
    waiting: Option<WireId>,
    last: Value,
    deadline_at: Option<u64>,
    allocated: u64,
    held: bool,
    inv: Option<Invoke>,
}

/// FakePlugin. One process, one channel at a time.
pub struct FakePlugin {
    ch: Chooser,
    now: u64,
    fixture: Value,
    link: Link,
    state: State,
    limits: Option<PluginLimits>,
    mem: Memory,
    queue: VecDeque<Invoke>,
    run: Option<Run>,
    next_call: u64,
    version: u32,
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

fn failure(kind: ErrorKind, code: Option<&str>, message: &str) -> PluginResult {
    let mut e = ErrorBody::new(kind, message);
    if let Some(c) = code {
        e = e.with_code(c);
    }
    PluginResult::Err(e)
}

impl FakePlugin {
    /// A host for one seed. Call `configure` next.
    pub fn new(seed: u64) -> Self {
        let mut ch = Chooser::new(seed);
        let base = ch.base();
        let l = test_limits(&Value::Null).expect("the test limits parse");
        FakePlugin {
            ch,
            now: 0,
            fixture: Value::Null,
            link: Link::new(l.frame_bytes as u32, lane_caps(&l)),
            state: State::Idle,
            limits: None,
            mem: Memory::default(),
            queue: VecDeque::new(),
            run: None,
            next_call: base * 100,
            version: 1,
        }
    }

    /// The choice points taken so far (design 7.2, rule 3).
    pub fn choices(&self) -> &std::collections::BTreeMap<Choice, Vec<usize>> {
        self.ch.taken()
    }

    fn limits(&self) -> PluginLimits {
        self.limits
            .clone()
            .unwrap_or_else(|| test_limits(&Value::Null).expect("the test limits parse"))
    }

    fn epoch(&self) -> Epoch {
        self.link.epoch.unwrap_or(Epoch(0))
    }

    fn send(&mut self, lane: Lane, frame: Frame) -> bool {
        // HP MD-2: a frame over `frame_bytes` is refused by the sender, and nothing is truncated.
        self.link.send(lane, &frame).is_ok()
    }

    fn ready(&self) -> Result<Ready, String> {
        let f = &self.fixture;
        let mut r = json!({
            "epoch": self.epoch().0, "protocol_version": 1,
            "handlers": [], "tools": [], "prompts": [], "resources": [], "events": {}, "entity_providers": [], "timers": [], "watches": [], "fs_watches": [],
            "session_actions": [], "operations": {}, "grants": [],
            // HP LH-2: a host states its name and version in `Ready.host`.
            "host": f.get("host").cloned().unwrap_or(json!({"name": "fake-host", "version": "1", "globals": []})),
            "features": {"required": [], "optional": []},
        });
        if let Value::Object(reg) = f.get("registration").unwrap_or(&Value::Null) {
            for (k, v) in reg {
                r[k.as_str()] = v.clone();
            }
        }
        if let Some(x) = f.get("features") {
            r["features"] = x.clone();
        }
        serde_json::from_value(r).map_err(|e| e.to_string())
    }

    // ---- frames from the Hub -------------------------------------------------------------------------------------------------------------------------

    fn pump(&mut self) {
        while let Some(ib) = self.link.next_inbound(&mut self.ch) {
            self.on_inbound(ib);
        }
        if self.link.closed.is_some() && self.state != State::Dead {
            self.state = State::Dead;
            self.run = None;
        }
        self.drive();
    }

    fn on_inbound(&mut self, ib: Inbound) {
        match ib {
            // HP VR-2: an unknown state-changing frame is refused with `Fault{unsupported_frame, type}`.
            Inbound::Unknown { ty } => {
                let epoch = self.epoch();
                self.send(
                    Lane::Control,
                    Frame::Fault(Fault {
                        epoch,
                        code: "unsupported_frame".into(),
                        detail: Some(json!({"type": ty})),
                    }),
                );
            }
            Inbound::Malformed { ty, payload, .. } => {
                // HP VR-4, WF-2 to WF-4: an `Invoke` that carries an unknown caller, handler kind or origin shape, or a malformed field, fails closed: the host answers `invalid_request` and runs nothing.
                if FrameType::from_byte(ty) == Some(FrameType::Invoke) {
                    if let Some(request_id) = lenient_id(&payload, "request_id") {
                        let epoch = self.epoch();
                        self.send(
                            Lane::Invoke,
                            Frame::Result(ResultFrame {
                                epoch,
                                request_id,
                                result: failure(
                                    ErrorKind::InvalidRequest,
                                    None,
                                    "malformed request",
                                ),
                            }),
                        );
                    }
                }
            }
            Inbound::Frame { frame, .. } => match *frame {
                Frame::Hello(h) => self.on_hello(h),
                Frame::Activated(_) if self.state == State::AwaitActivated => {
                    self.state = State::Active;
                    // HP CA-3 (c): after `Activated` the host may make `autonomous` calls (`log` only).
                    let steps = self
                        .fixture
                        .get("autonomous")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    if !steps.is_empty() {
                        self.queue_autonomous(steps);
                    }
                }
                // HP MD-1: no order is promised between lanes, so an `Invoke` may be read before `Activated`; it waits until the host is active (HP HS-2).
                Frame::Invoke(i) if matches!(self.state, State::Active | State::AwaitActivated) => {
                    self.queue.push_back(i)
                }
                Frame::Cancel(c) => self.on_cancel(c),
                Frame::HostResult(r) => self.on_host_result(r),
                Frame::Shutdown(_) => {
                    // HP LD-4: on `Shutdown` the host stops its handlers and ends; the Hub stops the process group.
                    self.run = None;
                    self.queue.clear();
                    self.state = State::Dead;
                    self.link.close("process_exit");
                }
                _ => {}
            },
        }
    }

    fn on_hello(&mut self, h: Hello) {
        if self.state != State::Idle {
            return;
        }
        self.link.epoch = Some(h.epoch);
        self.link.frame_bytes = h.limits.frame_bytes as u32;
        self.link.lane_caps = lane_caps(&h.limits);
        self.limits = Some(h.limits.clone());
        // HP HS-3: the host picks the highest protocol version in `[min_protocol_version, protocol_version]` that it supports; with no overlap it faults.
        let supported: Vec<u32> =
            serde_json::from_value(self.fixture.get("protocol").cloned().unwrap_or(json!([1])))
                .unwrap_or_default();
        let Some(version) = supported
            .iter()
            .copied()
            .filter(|v| (h.min_protocol_version..=h.protocol_version).contains(v))
            .max()
        else {
            let epoch = h.epoch;
            self.send(
                Lane::Control,
                Frame::Fault(Fault {
                    epoch,
                    code: "unsupported_protocol".into(),
                    detail: None,
                }),
            );
            self.state = State::Dead;
            self.link.close("faulted");
            return;
        };
        self.version = version;
        if h.resumed {
            // HP HS-6, CR-5: after a re-adoption the host answers a full `Ready` again. Its process memory is kept, so the entrypoint does not run again.
            self.send_ready();
        } else {
            // HP HS-7: the entrypoint runs between `Hello` and `Activated`, and only reads (and `log.write`) are allowed there.
            self.mem.entrypoint_runs += 1;
            let steps = self
                .fixture
                .get("entrypoint")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            self.state = State::Booting;
            self.run = Some(Run {
                request: None,
                steps,
                pc: 0,
                waiting: None,
                last: Value::Null,
                deadline_at: None,
                allocated: 0,
                held: false,
                inv: None,
            });
        }
    }

    fn send_ready(&mut self) {
        let mut r = self
            .ready()
            .expect("the fixture registration is a valid Ready");
        r.protocol_version = self.version;
        self.send(Lane::Control, Frame::Ready(r));
        self.state = State::AwaitActivated;
    }

    fn queue_autonomous(&mut self, steps: Vec<Value>) {
        // The autonomous steps run as a run with no request: its calls carry the `autonomous` context.
        self.run = Some(Run {
            request: None,
            steps,
            pc: 0,
            waiting: None,
            last: Value::Null,
            deadline_at: None,
            allocated: 0,
            held: false,
            inv: None,
        });
    }

    fn on_cancel(&mut self, c: Cancel) {
        // HP IV-5: `Cancel` is best effort and idempotent. For a request that is not the active one it changes nothing.
        let active = self
            .run
            .as_ref()
            .is_some_and(|r| r.request.as_ref() == Some(&c.request_id));
        if !active {
            return;
        }
        // The host abandons the handler; its outstanding calls are answered `cancelled` by the Hub (HP IV-5, CA-5).
        self.run = None;
        // HP IV-2, IV-5, IV-7: whether a late `Result` follows is open; the Hub drops it either way.
        if self.ch.pick(Choice::LateResult, 2) == 1 {
            let epoch = self.epoch();
            let result = failure(ErrorKind::Cancelled, None, "the handler was cancelled");
            self.send(
                Lane::Invoke,
                Frame::Result(ResultFrame {
                    epoch,
                    request_id: c.request_id,
                    result,
                }),
            );
        }
    }

    fn on_host_result(&mut self, r: HostResult) {
        let Some(run) = self.run.as_mut() else { return };
        if run.waiting.as_ref() == Some(&r.call_id) {
            run.waiting = None;
            let mut v = serde_json::to_value(&r.result).unwrap_or(Value::Null);
            // HP ER-1: a host that receives a kind it does not know treats it as `failed`, honoring `retryable` and `effect`.
            if let PluginResult::Err(e) = &r.result {
                v["error"]["kind"] = json!(e.kind.effective().as_str());
            }
            run.last = v;
        }
    }

    // ---- handlers ------------------------------------------------------------------------------------------------------------------------------------

    /// Runs the active handler until it waits, holds or finishes, then starts the next queued one (HP IV-4: one at a time).
    fn drive(&mut self) {
        loop {
            if self.run.is_none() && self.state == State::Active {
                let Some(inv) = self.queue.pop_front() else {
                    return;
                };
                self.start(inv);
            }
            let Some(run) = self.run.as_mut() else { return };
            if run.waiting.is_some() || run.held {
                return;
            }
            if run.pc >= run.steps.len() {
                // The script ended without a `return`: an entrypoint is ready to register, an autonomous run is done, a handler returns `null`.
                let (booting, request) = (self.state == State::Booting, run.request.clone());
                self.run = None;
                if booting {
                    self.send_ready();
                } else if let Some(id) = request {
                    self.finish(id, PluginResult::Ok(Value::Null));
                }
                continue;
            }
            let step = run.steps[run.pc].clone();
            run.pc += 1;
            if !self.step(&step) {
                return;
            }
        }
    }

    fn start(&mut self, inv: Invoke) {
        self.mem.invocations += 1;
        let handler = self.fixture["handlers"]
            .get(&inv.handler)
            .and_then(Value::as_array)
            .cloned();
        let Some(steps) = handler else {
            // HP ER-1: a handler id that was never registered is `not_found`.
            return self.finish(
                inv.request_id,
                failure(ErrorKind::NotFound, None, "no such handler"),
            );
        };
        // HP WF-5: the deadline is a relative duration, measured from the moment the host reads the frame on its own clock.
        let deadline_at = Some(self.now + inv.deadline_ms);
        self.run = Some(Run {
            request: Some(inv.request_id.clone()),
            steps,
            pc: 0,
            waiting: None,
            last: Value::Null,
            deadline_at,
            allocated: 0,
            held: false,
            inv: Some(inv),
        });
    }

    fn finish(&mut self, request_id: WireId, result: PluginResult) {
        let lim = self.limits();
        let epoch = self.epoch();
        let too_big = matches!(&result, PluginResult::Ok(v) if serde_json::to_vec(v).map_or(0, |b| b.len() as u64) > lim.max_result_bytes);
        if !too_big
            && self
                .link
                .send(
                    Lane::Invoke,
                    &Frame::Result(ResultFrame {
                        epoch,
                        request_id: request_id.clone(),
                        result,
                    }),
                )
                .is_ok()
        {
            return;
        }
        // HP IV-2: a result over `max_result_bytes` or over one frame is never sent; the host answers `response_too_large` and truncates nothing.
        let result = failure(ErrorKind::ResponseTooLarge, None, "the result is too large");
        self.send(
            Lane::Invoke,
            Frame::Result(ResultFrame {
                epoch,
                request_id,
                result,
            }),
        );
    }

    fn call(&mut self, spec: &Value) -> bool {
        let lim = self.limits();
        let epoch = self.epoch();
        let run = self.run.as_mut().expect("a run");
        let context = match (spec.get("context"), &run.request) {
            (Some(Value::String(c)), _) if c == "bootstrap" => CallContext::Bootstrap,
            (Some(Value::String(c)), _) if c == "autonomous" => CallContext::Autonomous,
            // A context that names a request that is not live (HP CA-3).
            (Some(Value::Object(o)), _) => CallContext::Request(
                serde_json::from_value(o["request"].clone()).unwrap_or(WireId::Num(0)),
            ),
            (_, Some(id)) => CallContext::Request(id.clone()),
            (_, None) if self.state == State::Booting => CallContext::Bootstrap,
            (_, None) => CallContext::Autonomous,
        };
        self.next_call += 1;
        let call_id = WireId::Num(self.next_call);
        let name = s(spec, "name").unwrap_or("log.write").to_string();
        let hc = HostCall {
            epoch,
            call_id: call_id.clone(),
            context,
            call: name,
            args: spec.get("args").cloned().unwrap_or(json!({})),
            max_result_bytes: spec
                .get("max_result_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(lim.max_result_bytes),
        };
        let frame = Frame::HostCall(hc);
        // HP section 2, R-4: a bulk-class call travels on lane 3, every other call on lane 2.
        let lane = frame.default_lane();
        if !self.send(lane, frame) {
            self.run.as_mut().expect("a run").last = json!({"ok": false, "error": {"kind": "failed", "detail": {"code": "frame_too_large"}}});
            return true;
        }
        self.run.as_mut().expect("a run").waiting = Some(call_id);
        false
    }

    /// Runs one step. `false` means the handler waits or ended.
    fn step(&mut self, step: &Value) -> bool {
        let lim = self.limits();
        let Value::Object(m) = step else { return true };
        let Some((op, arg)) = m.iter().next() else {
            return true;
        };
        match op.as_str() {
            "call" => self.call(arg),
            "hold" => {
                // A slow handler never finishes by itself: it ends at `Cancel` or at the deadline.
                self.run.as_mut().expect("a run").held = true;
                false
            }
            "spin" => {
                // HP LM-4: a handler over the instruction budget ends with a structured failure and the host keeps running; a host that cannot stop it sends `Fault`.
                let id = self.run.as_ref().and_then(|r| r.request.clone());
                let stoppable = arg
                    .get("stoppable")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                self.run = None;
                if stoppable {
                    if let Some(id) = id {
                        let r = failure(
                            ErrorKind::Failed,
                            Some("instruction_budget"),
                            "the instruction budget was exceeded",
                        );
                        self.finish(id, r);
                    }
                } else {
                    let epoch = self.epoch();
                    self.send(
                        Lane::Control,
                        Frame::Fault(Fault {
                            epoch,
                            code: "handler_not_stoppable".into(),
                            detail: None,
                        }),
                    );
                    self.state = State::Dead;
                    self.link.close("faulted");
                }
                false
            }
            "alloc" => {
                // HP LM-6, LM-7: memory is a per-plugin allocation cap enforced by the host. An allocation over it is refused with a typed error; the host keeps running.
                let n = arg.as_u64().unwrap_or(0);
                let run = self.run.as_mut().expect("a run");
                run.allocated += n;
                let cap = lim.host_memory_bytes;
                if run.allocated > cap {
                    let id = run.request.clone();
                    self.run = None;
                    if let Some(id) = id {
                        let r = failure(
                            ErrorKind::QuotaExceeded,
                            Some("host_memory"),
                            "the host memory cap was reached",
                        );
                        self.finish(id, r);
                    }
                    return false;
                }
                true
            }
            "crash" => {
                // HP CR-1: a plugin that exits or closes its channel is crashed; a `Fault` is a crash cause (HP CR-2).
                if arg.as_str() == Some("fault") {
                    let epoch = self.epoch();
                    self.send(
                        Lane::Control,
                        Frame::Fault(Fault {
                            epoch,
                            code: "host_fault".into(),
                            detail: None,
                        }),
                    );
                }
                self.run = None;
                self.state = State::Dead;
                self.link.close("process_exit");
                false
            }
            "oversize" => {
                // A fault injection: complete wire bytes of a frame whose `len` is over `frame_bytes` (HP WF-1).
                let lane = arg
                    .get("lane")
                    .and_then(Value::as_str)
                    .and_then(lane_by_name)
                    .unwrap_or(Lane::Invoke);
                let body = vec![b' '; lim.frame_bytes as usize];
                let bytes = encode_raw(lane.byte(), 0x11, &body, u32::MAX).expect("raw frame");
                self.link.send_wire(lane, bytes);
                true
            }
            "return" => {
                let id = self.run.as_ref().and_then(|r| r.request.clone());
                let last = self.run.as_ref().map_or(Value::Null, |r| r.last.clone());
                let inv = self.run.as_ref().and_then(|r| r.inv.clone());
                let result = self.result_of(arg, &last, inv.as_ref());
                self.run = None;
                if let Some(id) = id {
                    self.finish(id, result);
                }
                false
            }
            _ => true,
        }
    }

    /// The value a `return` step names. `"$invocations"` and `"$entrypoint_runs"` read the process memory (HP CR-5); `{"$filler": n}` is a string of n bytes.
    fn result_of(&self, arg: &Value, last: &Value, inv: Option<&Invoke>) -> PluginResult {
        if arg.as_str() == Some("last") {
            return serde_json::from_value(last.clone()).unwrap_or(PluginResult::Ok(Value::Null));
        }
        if let Some(e) = arg.get("error") {
            return serde_json::from_value::<ErrorBody>(e.clone()).map_or(
                failure(ErrorKind::Failed, None, "failed"),
                PluginResult::Err,
            );
        }
        PluginResult::Ok(self.resolve(arg.get("ok").unwrap_or(&Value::Null), inv))
    }

    /// The value of a `return`. A string that starts with `$` reads the process memory (HP CR-5) or the `Invoke` (HP IV-1); `{"$filler": n}` is a string of n bytes.
    fn resolve(&self, v: &Value, inv: Option<&Invoke>) -> Value {
        match v {
            Value::String(t) if t == "$invocations" => json!(self.mem.invocations),
            Value::String(t) if t == "$entrypoint_runs" => json!(self.mem.entrypoint_runs),
            // HP IV-1: the wire carries `payload`, `caller` and `origin` as explicit fields.
            Value::String(t) if t == "$caller" => {
                serde_json::to_value(inv.map(|i| i.caller.clone())).unwrap_or(Value::Null)
            }
            Value::String(t) if t == "$origin" => json!(inv.map(|i| i.origin.0.clone())),
            Value::String(t) if t == "$payload" => inv.map_or(Value::Null, |i| i.payload.clone()),
            Value::Object(o) if o.contains_key("$filler") => {
                json!("x".repeat(o["$filler"].as_u64().unwrap_or(0) as usize))
            }
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), self.resolve(x, inv)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.resolve(x, inv)).collect()),
            other => other.clone(),
        }
    }
}

impl FakeEndpoint for FakePlugin {
    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        self.fixture = setup.get("fixture").cloned().unwrap_or(Value::Null);
        self.ready().map(|_| ())
    }

    fn write(&mut self, _conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.link.feed(bytes)
    }

    fn read(&mut self, _conn: &str) -> Option<Vec<u8>> {
        self.pump();
        let Self { link, ch, .. } = self;
        link.read(ch)
    }

    fn closed(&self, _conn: &str) -> Option<&'static str> {
        self.link.closed
    }

    fn close_channel(&mut self, _conn: &str) {
        self.link.close("peer_closed");
        self.pump();
    }

    fn advance_clock(&mut self, ms: u64) {
        self.pump();
        self.now += ms;
        // HP LM-4, WF-5: the host enforces the Hub's deadline on its own clock; a handler over it ends with a structured failure and the host keeps running.
        let due = self
            .run
            .as_ref()
            .is_some_and(|r| r.deadline_at.is_some_and(|d| self.now >= d));
        if due {
            let id = self.run.take().and_then(|r| r.request);
            if let Some(id) = id {
                self.finish(
                    id,
                    failure(ErrorKind::TimedOut, None, "the deadline expired"),
                );
            }
        }
        self.pump();
    }

    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        let name = s(op, "op").unwrap_or("");
        if name != "reconnect" && name != "restart_process" {
            self.pump();
        }
        Ok(match name {
            "settle" => json!({}),
            "status" => json!({"state": format!("{:?}", self.state).to_lowercase()}),
            "counters" => {
                json!({"stale_dropped": self.link.stale, "ignored_advisory": self.link.ignored})
            }
            "received" => self.link.received(s(op, "lane").unwrap_or("control")),
            "memory" => {
                json!({"invocations": self.mem.invocations, "entrypoint_runs": self.mem.entrypoint_runs})
            }
            "hold_lane" | "release_lane" => {
                let lane = s(op, "lane")
                    .and_then(lane_by_name)
                    .ok_or_else(|| Unsupported(format!("lane in {name}")))?;
                self.link.hold(lane, name == "hold_lane");
                json!({"lane": lane_name(lane)})
            }
            // HP CR-5: the Hub restarted and the service reconnects with a higher epoch; the process and its memory stay.
            "reconnect" => {
                let l = self.limits();
                self.link = Link::new(l.frame_bytes as u32, lane_caps(&l));
                self.state = State::Idle;
                self.run = None;
                self.queue.clear();
                json!({})
            }
            // HP CR-5: a reload replaces the process and loses its memory.
            "restart_process" => {
                let l = test_limits(&Value::Null).expect("the test limits parse");
                self.link = Link::new(l.frame_bytes as u32, lane_caps(&l));
                self.state = State::Idle;
                self.run = None;
                self.queue.clear();
                self.mem = Memory::default();
                json!({})
            }
            other => return Err(Unsupported(other.to_string())),
        })
    }

    fn controls(&self) -> &'static [&'static str] {
        CONTROLS
    }

    fn conns(&self) -> Vec<String> {
        vec!["main".to_string()]
    }
}
