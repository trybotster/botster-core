//! FakeHub: the plugin-side Hub endpoint (HP section 19, steward ruling R-1.3).
//!
//! It is scripted. It holds the protocol state of each plugin: the handshake, the registration, the epochs, the lanes, one active invocation with a bounded queue,
//! deadlines and `Cancel`, the limits, the frame bounds, `Fault` and the channel close. It models no Hub semantics that no clause states: the semantic answers
//! (a stored document, a grant check, a verified proof) come from the script in `setup`, and a control that the fake does not know is answered from `setup.controls`.
//!
//! Every branch cites its clause. Open orders are the named choice points of [`Choice`].

use crate::limits::{lane_caps, test_limits};
use crate::link::{
    lane_by_name, lane_name, lenient_id, Backpressured, Choice, Chooser, Inbound, Link,
};
use crate::{FakeEndpoint, Unsupported};
use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The native controls of FakeHub (design 6.3). The runner crate documents the closed list for a real Hub.
pub const CONTROLS: &[&str] = &[
    "status",
    "completions",
    "invoke",
    "cancel",
    "counters",
    "received",
    "published",
    "hold_lane",
    "release_lane",
    "settle",
    "expire_drain",
    "reload",
    "unload",
    "service_connected",
    "service_exited",
    "service_lost",
    "hub_restart",
    "load_plugin",
    "open_subscription",
    "hold_commits",
    "release_commits",
    "diagnostics",
    "descriptors",
    "emit_events",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Status {
    Loading,
    Loaded,
    Unloading,
    Unloaded,
    LoadFailed { code: String, names: Vec<String> },
    Crashed,
    AwaitService(&'static str),
    Quarantined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    AwaitReady,
    Active,
    Shutdown,
}

pub(crate) struct Conn {
    pub link: Link,
    pub plugin: String,
    pub phase: Phase,
    pub hello_at: u64,
    pub features: Vec<String>,
    pub candidate: bool,
    pub fenced: bool,
    pub shutdown_at: Option<u64>,
    pub seen_closed: bool,
    pub fault: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReqState {
    Queued,
    Active,
    Done,
}

pub(crate) struct Req {
    pub ticket: u64,
    pub request_id: u64,
    pub handler: String,
    pub kind: HandlerKind,
    pub origin: String,
    pub payload: Value,
    pub caller: Caller,
    pub source: Option<EventSourceRef>,
    /// The identity a client captured when it queued the action (HP IV-8), if any.
    pub descriptor_id: Option<String>,
    pub long: bool,
    pub idempotency_key: bool,
    pub state: ReqState,
    pub deadline_at: u64,
    pub conn: String,
    pub class: crate::events::Class,
    pub meta: Option<crate::events::Meta>,
}

/// The protocol state of one plugin.
pub(crate) struct P {
    pub status: Status,
    pub serving: String,
    /// The candidate is started and its `Ready` is not validated yet: the old generation still serves (HP LD-3 steps 1 and 2).
    pub preparing: bool,
    /// Admission to the old generation stopped (HP LD-3 step 3).
    pub reloading: bool,
    pub drain_expired: bool,
    pub cand: Option<(String, Ready)>,
    pub reload_error: Option<String>,
    pub instance_n: u64,
    pub reqs: Vec<Req>,
    pub completions: Vec<(u64, Value)>,
    pub published: Value,
    pub descriptors: BTreeMap<String, String>,
    pub crash_times: Vec<u64>,
    pub restart_requested: u64,
    pub last_cause: Value,
    pub exit_info: Option<Value>,
    pub bootstrap_logs: Vec<Value>,
    pub parked: Vec<(String, Lane, HostCall)>,
    pub hold_commits: bool,
    pub late_results: u64,
    pub events: crate::events::Queues,
}

/// FakeHub. Any number of plugins; a reload gives one plugin two connections.
pub struct FakeHub {
    pub(crate) ch: Chooser,
    pub(crate) now: u64,
    pub(crate) limits: PluginLimits,
    pub(crate) cfg: Value,
    pub(crate) conns: BTreeMap<String, Conn>,
    pub(crate) plugins: BTreeMap<String, P>,
    pub(crate) default: String,
    epoch_next: u64,
    next_ticket: u64,
    next_request: u64,
    call_uses: BTreeMap<String, usize>,
}

/// What a plugin has published: nothing (HP LD-6, CR-2: unload, a crash and a failed load leave nothing, so every collection is empty).
fn empty_published() -> Value {
    json!({"handlers": [], "tools": [], "prompts": [], "resources": [], "timers": [], "subscriptions": [], "declared_events": [], "entity_providers": [], "held_subscriptions": [], "gap_handler": null})
}

pub(crate) fn refusal(
    kind: ErrorKind,
    code: Option<&str>,
    message: &str,
    retryable: bool,
) -> PluginResult {
    let mut e = ErrorBody::new(kind, message);
    e.retryable = retryable;
    if let Some(c) = code {
        e = e.with_code(c);
    }
    PluginResult::Err(e)
}

fn answer(r: &PluginResult) -> Value {
    serde_json::to_value(r).expect("a result serializes")
}

fn wire_num(id: &WireId) -> u64 {
    match id {
        WireId::Num(n) => *n,
        WireId::Str(s) => s.parse().unwrap_or(0),
    }
}

fn s<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

impl FakeHub {
    /// A Hub for one seed. Call `configure` next.
    pub fn new(seed: u64) -> Self {
        let mut ch = Chooser::new(seed);
        let base = ch.base();
        FakeHub {
            ch,
            now: 0,
            limits: test_limits(&Value::Null).expect("the test limits parse"),
            cfg: Value::Null,
            conns: BTreeMap::new(),
            plugins: BTreeMap::new(),
            default: "demo".into(),
            epoch_next: base,
            next_ticket: 1,
            next_request: base * 10,
            call_uses: BTreeMap::new(),
        }
    }

    /// The choice points taken so far (design 7.2, rule 3).
    pub fn choices(&self) -> &BTreeMap<Choice, Vec<usize>> {
        self.ch.taken()
    }

    fn script(&self, key: &str) -> &Value {
        self.cfg.get(key).unwrap_or(&Value::Null)
    }

    /// Starts one plugin: its connection opens and `Hello` goes first (HP HS-1, LD-1).
    fn start_plugin(&mut self, key: &str, conn: &str, host: &Value) {
        let trusted = match host {
            Value::Null => true,
            h => {
                let shipped = self
                    .script("shipped_hosts")
                    .as_array()
                    .is_some_and(|a| a.contains(&h["path"]));
                let listed = self.script("trusted_hosts").as_array().is_some_and(|a| {
                    a.iter()
                        .any(|t| t["path"] == h["path"] && t["digest"] == h["digest"])
                });
                shipped || listed
            }
        };
        let p = P {
            status: Status::Loading,
            serving: conn.to_string(),
            preparing: false,
            reloading: false,
            drain_expired: false,
            cand: None,
            reload_error: None,
            instance_n: 0,
            reqs: Vec::new(),
            completions: Vec::new(),
            published: empty_published(),
            descriptors: BTreeMap::new(),
            crash_times: Vec::new(),
            restart_requested: 0,
            last_cause: Value::Null,
            exit_info: None,
            bootstrap_logs: Vec::new(),
            parked: Vec::new(),
            hold_commits: false,
            late_results: 0,
            events: Default::default(),
        };
        self.plugins.insert(key.to_string(), p);
        if !trusted {
            // HP TR-1: a host that is not shipped and not on the operator's list fails the load with `host_not_trusted`, and never starts.
            self.plugins.get_mut(key).expect("plugin").status = Status::LoadFailed {
                code: "host_not_trusted".into(),
                names: vec![],
            };
            return;
        }
        self.open_conn(key, conn, false, false);
    }

    fn open_conn(&mut self, key: &str, name: &str, resumed: bool, candidate: bool) {
        let epoch = Epoch(self.epoch_next);
        self.epoch_next += 1;
        let grants: Vec<Grant> = self
            .cfg
            .get("plugins")
            .and_then(|m| m.get(key))
            .and_then(|m| m.get("grants"))
            .or_else(|| self.cfg.get("grants"))
            .and_then(|g| serde_json::from_value(g.clone()).ok())
            .unwrap_or_default();
        let features: Vec<String> =
            serde_json::from_value(self.script("features").clone()).unwrap_or_default();
        let (min, current) = match self.script("protocol") {
            Value::Object(o) => (
                o.get("min").and_then(Value::as_u64).unwrap_or(1) as u32,
                o.get("current").and_then(Value::as_u64).unwrap_or(1) as u32,
            ),
            _ => (1, 1),
        };
        let bindings = serde_json::from_value(self.script("bindings").clone()).unwrap_or_default();
        let hub_id = s(&self.cfg, "hub_id").unwrap_or("hub-1").to_string();
        let p = self.plugins.get_mut(key).expect("plugin");
        if !resumed {
            p.instance_n += 1;
        }
        let hello = Hello {
            epoch,
            protocol_version: current,
            min_protocol_version: min,
            features: features.clone(),
            grants,
            limits: self.limits.clone(),
            bindings,
            priorities: v1_priorities(),
            instance: format!("inst-{}", p.instance_n),
            plugin_key: key.to_string(),
            resumed,
            hub_id,
        };
        let mut link = Link::new(self.limits.frame_bytes as u32, lane_caps(&self.limits));
        link.epoch = Some(epoch);
        // HP HS-1: `Hello` is the first frame on lane 0, and nothing is sent before it.
        link.send(Lane::Control, &Frame::Hello(hello))
            .expect("Hello fits in a frame");
        let conn = Conn {
            link,
            plugin: key.to_string(),
            phase: Phase::AwaitReady,
            hello_at: self.now,
            features,
            candidate,
            fenced: false,
            shutdown_at: None,
            seen_closed: false,
            fault: None,
        };
        self.conns.insert(name.to_string(), conn);
    }

    fn send(&mut self, conn: &str, lane: Lane, frame: Frame) {
        if let Some(c) = self.conns.get_mut(conn) {
            if c.link.send(lane, &frame).is_err() {
                // HP MD-2: a frame over `frame_bytes` is refused by the sender with `frame_too_large` and nothing is truncated; here the channel ends.
                c.link.close("frame_too_large");
            }
        }
    }

    fn epoch_of(&self, conn: &str) -> Epoch {
        self.conns
            .get(conn)
            .and_then(|c| c.link.epoch)
            .unwrap_or(Epoch(0))
    }

    fn p(&mut self, key: &str) -> &mut P {
        self.plugins.get_mut(key).expect("plugin")
    }

    /// Stops a connection's process group: the channel closes and nobody treats it as a crash (HP LD-4, HS-5).
    fn stop_group(&mut self, conn: &str, cause: &'static str) {
        if let Some(c) = self.conns.get_mut(conn) {
            c.link.close(cause);
            c.seen_closed = true;
        }
    }

    fn fail_load(&mut self, key: &str, code: &str, names: Vec<String>) {
        let p = self.p(key);
        p.status = Status::LoadFailed {
            code: code.into(),
            names,
        };
    }

    /// Completes the caller of a request exactly once (HP IV-7).
    fn finish(&mut self, key: &str, idx: usize, result: Value) {
        let p = self.p(key);
        p.reqs[idx].state = ReqState::Done;
        let ticket = p.reqs[idx].ticket;
        if !p.completions.iter().any(|(t, _)| *t == ticket) {
            p.completions.push((ticket, result));
        }
    }

    fn failure(
        kind: ErrorKind,
        code: Option<&str>,
        message: &str,
        effect: &str,
        retryable: bool,
    ) -> Value {
        let mut a = answer(&refusal(kind, code, message, retryable));
        a["error"]["effect"] = json!(effect);
        a
    }

    /// A refusal before dispatch: the caller sees `effect: none` (HP ER-5).
    fn refuse_now(
        &mut self,
        key: &str,
        ticket: u64,
        kind: ErrorKind,
        code: Option<&str>,
        message: &str,
        retryable: bool,
    ) {
        let a = Self::failure(kind, code, message, "none", retryable);
        self.p(key).completions.push((ticket, a));
    }

    // ---- frames from the plugin ----------------------------------------------------------------------------------------------------------------------

    fn pump(&mut self) {
        let names: Vec<String> = self.conns.keys().cloned().collect();
        for name in names {
            while let Some(ib) = {
                let Self { conns, ch, .. } = &mut *self;
                conns.get_mut(&name).and_then(|c| c.link.next_inbound(ch))
            } {
                self.on_inbound(&name, ib);
            }
            if self
                .conns
                .get(&name)
                .is_some_and(|c| c.link.closed.is_some() && !c.seen_closed)
            {
                self.conns.get_mut(&name).expect("conn").seen_closed = true;
                self.on_closed(&name);
            }
        }
        self.tick();
        let keys: Vec<String> = self.plugins.keys().cloned().collect();
        for k in keys {
            self.progress(&k);
        }
    }

    /// The deadlines that the clock has passed (HP HS-5, IV-3, LD-4).
    fn tick(&mut self) {
        let keys: Vec<String> = self.plugins.keys().cloned().collect();
        for k in &keys {
            // HP IV-3: at the deadline the Hub sends `Cancel` and completes the caller `timed_out`.
            let due: Vec<usize> = (0..self.plugins[k].reqs.len())
                .filter(|i| {
                    self.plugins[k].reqs[*i].state == ReqState::Active
                        && self.plugins[k].reqs[*i].deadline_at <= self.now
                })
                .collect();
            for i in due {
                self.expire(k, i);
            }
        }
        let names: Vec<String> = self.conns.keys().cloned().collect();
        for n in names {
            let (key, phase, hello_at, cand, down, closed) = {
                let c = &self.conns[&n];
                (
                    c.plugin.clone(),
                    c.phase,
                    c.hello_at,
                    c.candidate,
                    c.shutdown_at,
                    c.seen_closed,
                )
            };
            // HP HS-5: `Ready` must arrive within `limits.startup`, or the load fails with `startup_timeout` and the process group stops.
            if phase == Phase::AwaitReady && !closed && self.now >= hello_at + self.limits.startup {
                self.stop_group(&n, "stopped");
                if cand {
                    self.reload_failed(&key, "startup_timeout");
                } else {
                    self.fail_load(&key, "startup_timeout", vec![]);
                }
            }
            // HP LD-4: after `Shutdown{deadline}` the Hub stops the process group.
            if phase == Phase::Shutdown && down.is_some_and(|d| self.now >= d) && !closed {
                self.stop_group(&n, "stopped");
                if self.plugins[&key].serving == n && self.plugins[&key].status == Status::Unloading
                {
                    let p = self.p(&key);
                    p.status = Status::Unloaded;
                    p.published = empty_published();
                }
            }
        }
    }

    fn on_inbound(&mut self, conn: &str, ib: Inbound) {
        let epoch = self.epoch_of(conn);
        match ib {
            // HP VR-2: an unknown state-changing frame is refused with `Fault{unsupported_frame, type}`.
            Inbound::Unknown { ty } => self.send(
                conn,
                Lane::Control,
                Frame::Fault(Fault {
                    epoch,
                    code: "unsupported_frame".into(),
                    detail: Some(json!({"type": ty})),
                }),
            ),
            Inbound::Malformed { ty, lane, payload } => {
                // HP WF-2, WF-3, WF-4, section 2: a malformed request is refused `invalid_request` and the epoch goes on. A `HostCall` is the request that has an answer.
                if FrameType::from_byte(ty) == Some(FrameType::HostCall) {
                    if let (Some(call_id), Some(l)) =
                        (lenient_id(&payload, "call_id"), Lane::from_byte(lane))
                    {
                        let result =
                            refusal(ErrorKind::InvalidRequest, None, "malformed request", false);
                        self.send(
                            conn,
                            l,
                            Frame::HostResult(HostResult {
                                epoch,
                                call_id,
                                result,
                                effect: Effect::None,
                            }),
                        );
                    }
                }
            }
            Inbound::Frame { frame, lane } => match *frame {
                Frame::Ready(r) => self.on_ready(conn, r),
                Frame::HostCall(c) => self.on_host_call(conn, lane, c),
                Frame::Result(r) => self.on_result(conn, r),
                Frame::Fault(f) => self.on_fault(conn, f),
                _ => {}
            },
        }
    }

    fn on_fault(&mut self, conn: &str, f: Fault) {
        let (key, phase, cand) = {
            let c = &self.conns[conn];
            (c.plugin.clone(), c.phase, c.candidate)
        };
        // HP HS-3: `Fault{unsupported_protocol}` while the plugin loads fails the load.
        if phase == Phase::AwaitReady && !cand {
            self.fail_load(&key, &f.code, vec![]);
            self.stop_group(conn, "fault");
        } else {
            // HP CR-2: a `Fault` is a crash cause.
            self.conns.get_mut(conn).expect("conn").fault = Some(f.code);
            self.conns.get_mut(conn).expect("conn").link.close("fault");
        }
    }

    fn reject(&mut self, conn: &str, code: &str, names: Vec<String>) {
        let (key, cand) = (self.conns[conn].plugin.clone(), self.conns[conn].candidate);
        let epoch = self.epoch_of(conn);
        // HP HS-2, HS-4: a rejected registration is a `Shutdown{reason}` with a typed reason and a teardown; nothing it proposed stays published.
        let deadline_ms = self.limits.shutdown;
        self.send(
            conn,
            Lane::Control,
            Frame::Shutdown(Shutdown {
                epoch,
                deadline_ms,
                reason: code.to_string(),
            }),
        );
        self.conns.get_mut(conn).expect("conn").phase = Phase::Shutdown;
        self.stop_group(conn, "stopped");
        if cand {
            self.reload_failed(&key, code);
        } else {
            self.fail_load(&key, code, names);
        }
    }

    fn on_ready(&mut self, conn: &str, r: Ready) {
        let (key, cand, phase, offered) = {
            let c = &self.conns[conn];
            (c.plugin.clone(), c.candidate, c.phase, c.features.clone())
        };
        if phase != Phase::AwaitReady {
            return;
        }
        // HP HS-4: a missing required feature fails the load with `feature_missing{names}`; a missing optional one changes nothing.
        let missing: Vec<String> = r
            .features
            .missing(&offered)
            .into_iter()
            .map(String::from)
            .collect();
        if !missing.is_empty() {
            return self.reject(conn, "feature_missing", missing);
        }
        // HP IV-8: a duplicate handler id fails the load.
        let mut ids: Vec<&String> = r.handlers.iter().map(|x| &x.id).collect();
        ids.sort();
        if ids.windows(2).any(|w| w[0] == w[1]) {
            return self.reject(conn, "invalid_request", vec![]);
        }
        // HP HS-2, CP-2: whether the registration is valid against the manifest is Hub policy. The script decides.
        if let Some(code) = s(&self.cfg, "reject_registration") {
            let code = code.to_string();
            return self.reject(conn, &code, vec![]);
        }
        if cand {
            // HP LD-3 (2), (3): the candidate's `Ready` is validated, then admission to the old generation stops and the handover waits for the drain.
            let p = self.p(&key);
            p.cand = Some((conn.to_string(), r));
            p.preparing = false;
            p.reloading = true;
            return;
        }
        self.activate(&key, conn, &r);
    }

    /// Publishes the registration atomically and sends `Activated` (HP HS-2, LD-1).
    fn activate(&mut self, key: &str, conn: &str, r: &Ready) {
        let instance = format!("inst-{}", self.plugins[key].instance_n);
        let p = self.p(key);
        p.published = json!({
            "handlers": r.handlers.iter().map(|h| &h.id).collect::<Vec<_>>(),
            "tools": r.tools.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "prompts": r.prompts.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "resources": r.resources.iter().map(|t| &t.name).collect::<Vec<_>>(),
            "timers": r.timers.iter().map(|t| &t.timer_id).collect::<Vec<_>>(),
            "subscriptions": r.events.subscribed.iter().map(|t| &t.handler_id).collect::<Vec<_>>(),
            "declared_events": r.events.declared.iter().map(|e| &e.name).collect::<Vec<_>>(),
            "entity_providers": r.entity_providers.iter().map(|e| &e.kind).collect::<Vec<_>>(),
            "held_subscriptions": [],
            "gap_handler": r.events.gap_handler,
        });
        // HP IV-8: every handler has an opaque descriptor identity, new for each load.
        p.descriptors = r
            .handlers
            .iter()
            .map(|h| (h.id.clone(), format!("desc-{instance}-{}", h.id)))
            .collect();
        p.status = Status::Loaded;
        p.serving = conn.to_string();
        let epoch = self.epoch_of(conn);
        self.conns.get_mut(conn).expect("conn").phase = Phase::Active;
        self.send(conn, Lane::Control, Frame::Activated(Activated { epoch }));
    }

    fn reload_failed(&mut self, key: &str, code: &str) {
        // HP LD-3: a candidate that fails before the handover leaves the old generation serving and reports the error.
        let p = self.p(key);
        p.preparing = false;
        p.reloading = false;
        p.cand = None;
        p.reload_error = Some(code.to_string());
    }

    // ---- host calls -----------------------------------------------------------------------------------------------------------------------------------

    fn scripted(&mut self, call: &str) -> Value {
        let list = self
            .script("calls")
            .get(call)
            .cloned()
            .unwrap_or(Value::Null);
        let n = self.call_uses.entry(call.to_string()).or_default();
        let entry = match &list {
            Value::Array(a) if !a.is_empty() => a[(*n).min(a.len() - 1)].clone(),
            Value::Object(_) => list.clone(),
            _ => Value::Null,
        };
        *n += 1;
        entry
    }

    fn on_host_call(&mut self, conn: &str, lane: Lane, c: HostCall) {
        let (key, phase) = {
            let x = &self.conns[conn];
            (x.plugin.clone(), x.phase)
        };
        let epoch = self.epoch_of(conn);
        let call_id = c.call_id.clone();
        let reply = |this: &mut Self, result: PluginResult, effect: Effect| {
            this.send(
                conn,
                lane,
                Frame::HostResult(HostResult {
                    epoch,
                    call_id: call_id.clone(),
                    result,
                    effect,
                }),
            )
        };
        // HP CA-1: an unknown call is `invalid_request{unknown_call}`.
        let Some(spec) = find_call(&c.call) else {
            return reply(
                self,
                refusal(
                    ErrorKind::InvalidRequest,
                    Some("unknown_call"),
                    "unknown call",
                    false,
                ),
                Effect::None,
            );
        };
        let wrong_context = || {
            refusal(
                ErrorKind::Failed,
                Some("wrong_context"),
                "the call is outside its context",
                false,
            )
        };
        // HP CA-3: a call is made in exactly one context.
        match &c.context {
            CallContext::Request(id) => {
                let live = self.plugins[&key].reqs.iter().any(|r| {
                    r.conn == conn && r.request_id == wire_num(id) && r.state == ReqState::Active
                });
                if !live {
                    // HP CA-3: a call that names a finished, cancelled or timed-out request is answered `cancelled`, never dropped.
                    return reply(
                        self,
                        refusal(ErrorKind::Cancelled, None, "the request is not live", false),
                        Effect::None,
                    );
                }
                if !spec.contexts.contains(&Ctx::Req) {
                    return reply(self, wrong_context(), Effect::None);
                }
            }
            // HP CA-3, HS-7: `bootstrap` is for the reads of HS-7, between `Hello` and `Activated` only.
            CallContext::Bootstrap => {
                if phase != Phase::AwaitReady || !spec.contexts.contains(&Ctx::Boot) {
                    return reply(self, wrong_context(), Effect::None);
                }
                if c.call == "log.write" {
                    // HP LD-1: the bootstrap log records survive a failed load.
                    self.p(&key).bootstrap_logs.push(c.args.clone());
                }
            }
            // HP CA-3 (c): `autonomous` is a closed set, `log` only. VC-4: it has no caller.
            CallContext::Autonomous => {
                if !spec.contexts.contains(&Ctx::Auto) {
                    return reply(self, wrong_context(), Effect::None);
                }
            }
        }
        // HP LD-9: a mutating call that was admitted and is paused inside its commit waits until it is released.
        if spec.mutating && self.plugins[&key].hold_commits {
            self.p(&key).parked.push((conn.to_string(), lane, c));
            return;
        }
        let (result, effect) = self.host_answer(&c, spec.mutating);
        reply(self, result, effect);
    }

    /// The scripted answer to one call (R-1.3: the result and the effect of a semantic call come only from the script), with the size bound around it.
    fn host_answer(&mut self, c: &HostCall, mutating: bool) -> (PluginResult, Effect) {
        let entry = self.scripted(&c.call);
        // The script is part of the transcript: a call that it does not answer is a script error, not a Hub decision.
        let result: PluginResult = serde_json::from_value(
            entry.get("result").cloned().unwrap_or_else(|| {
                panic!("setup.calls has no result for `{}`: the script must answer every call it makes", c.call)
            }),
        )
        .expect("a scripted result is a valid Result (with message and retryable on an error)");
        // HP CA-5: the effect of a mutating call is stated by the script. A call that cannot mutate has no effect.
        let effect = match (s(&entry, "effect"), mutating) {
            (Some("committed"), _) => Effect::Committed,
            (Some("unknown"), _) => Effect::Unknown,
            (Some(_), _) | (None, false) => Effect::None,
            (None, true) => panic!(
                "setup.calls.{} states no effect for a mutating call",
                c.call
            ),
        };
        // HP CA-2: a result over `max_result_bytes` is `response_too_large`, never truncated. The effect of the call is the scripted one.
        if serde_json::to_vec(&result).map_or(0, |v| v.len() as u64) > c.max_result_bytes {
            let refused = refusal(
                ErrorKind::ResponseTooLarge,
                None,
                "the result is over max_result_bytes",
                false,
            );
            return (refused, effect);
        }
        (result, effect)
    }

    // ---- invocation ----------------------------------------------------------------------------------------------------------------------------------

    fn on_result(&mut self, conn: &str, r: ResultFrame) {
        let key = self.conns[conn].plugin.clone();
        let id = wire_num(&r.request_id);
        let Some(idx) = self.plugins[&key]
            .reqs
            .iter()
            .position(|q| q.conn == conn && q.request_id == id && q.state == ReqState::Active)
        else {
            // HP IV-2, IV-7: a `Result` after the deadline, after `Cancel` or for a finished request is dropped and counted.
            self.p(&key).late_results += 1;
            return;
        };
        // HP IV-7: an expired deadline rejects a `Result` even when its timer wake has not run yet.
        if self.now >= self.plugins[&key].reqs[idx].deadline_at {
            self.p(&key).late_results += 1;
            return self.expire(&key, idx);
        }
        let result = match r.result {
            PluginResult::Ok(v) => json!({"ok": true, "value": v}),
            PluginResult::Err(e) => {
                // HP ER-5: a handler failure after dispatch is `effect: unknown` (the Hub proves nothing), and not retryable without a resolving idempotency key.
                let retry = e.retryable && self.plugins[&key].reqs[idx].idempotency_key;
                let mut v = answer(&PluginResult::Err(e));
                v["error"]["effect"] = json!("unknown");
                v["error"]["retryable"] = json!(retry);
                v
            }
        };
        self.finish(&key, idx, result);
    }

    /// The deadline passed: `Cancel` goes out and the caller gets `timed_out`, effect unknown (HP IV-3, ER-5).
    fn expire(&mut self, key: &str, idx: usize) {
        let (conn, request_id) = (
            self.plugins[key].reqs[idx].conn.clone(),
            self.plugins[key].reqs[idx].request_id,
        );
        let epoch = self.epoch_of(&conn);
        self.send(
            &conn,
            Lane::Control,
            Frame::Cancel(Cancel {
                epoch,
                request_id: WireId::Num(request_id),
                reason: "deadline".into(),
            }),
        );
        let a = Self::failure(
            ErrorKind::TimedOut,
            None,
            "the deadline expired",
            "unknown",
            false,
        );
        self.finish(key, idx, a);
    }

    /// Sends the next queued request when no request is active (HP IV-4: one active `Invoke` at a time, FIFO within a class).
    fn progress(&mut self, key: &str) {
        self.try_handover(key);
        self.try_unload(key);
        let p = &self.plugins[key];
        let serving_ok = self
            .conns
            .get(&p.serving)
            .is_some_and(|c| c.phase == Phase::Active && !c.fenced);
        if p.status != Status::Loaded
            || p.reloading
            || !serving_ok
            || p.reqs.iter().any(|r| r.state == ReqState::Active)
        {
            return;
        }
        let Some(idx) = crate::events::pick_next(self, key) else {
            return;
        };
        // HP IV-8: the descriptor identity is revalidated at dispatch. A stale one is `conflict{stale_descriptor}`, never reaches the instance, and the queue never retargets it.
        let (handler, given) = (
            self.plugins[key].reqs[idx].handler.clone(),
            self.plugins[key].reqs[idx].descriptor_id.clone(),
        );
        let current = self.plugins[key].descriptors.get(&handler).cloned();
        if given.is_some() && given != current {
            let a = Self::failure(
                ErrorKind::Conflict,
                Some("stale_descriptor"),
                "the descriptor is stale",
                "none",
                false,
            );
            self.finish(key, idx, a);
            return self.progress(key);
        }
        crate::events::finalize(self, key, idx);
        let (conn, deadline) = {
            let r = &self.plugins[key].reqs[idx];
            // HP IV-3: the deadline comes from `PluginLimits` by kind; the contract fixes no number.
            (
                self.plugins[key].serving.clone(),
                if r.long {
                    self.limits.long_deadline
                } else {
                    self.limits.dispatch_deadline
                },
            )
        };
        let epoch = self.epoch_of(&conn);
        self.next_request += 1;
        let rid = self.next_request;
        let now = self.now;
        let p = self.p(key);
        let r = &mut p.reqs[idx];
        r.descriptor_id = current.clone();
        r.state = ReqState::Active;
        r.request_id = rid;
        r.conn = conn.clone();
        r.deadline_at = now + deadline;
        let inv = Invoke {
            epoch,
            request_id: WireId::Num(rid),
            handler: r.handler.clone(),
            kind: r.kind,
            payload: r.payload.clone(),
            caller: r.caller.clone(),
            origin: Origin(r.origin.clone()),
            source: r.source.clone(),
            descriptor_id: current.unwrap_or_default(),
            deadline_ms: deadline,
        };
        self.send(&conn, Lane::Invoke, Frame::Invoke(inv));
    }

    /// Admits one request for a caller. Returns the ticket (HP IV-1, IV-4, IV-8, VC-1).
    fn admit(&mut self, key: &str, spec: &Value) -> Result<u64, String> {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let handler = s(spec, "handler")
            .ok_or("invoke needs a handler")?
            .to_string();
        let kind: HandlerKind =
            serde_json::from_value(spec.get("kind").cloned().unwrap_or(json!("mcp_tool")))
                .map_err(|e| e.to_string())?;
        // HP VC-1: the Hub derives `caller` from the proof it checked. An origin with no requester, and a missing or malformed proof, is `system`, never `operator`.
        let caller = match (spec.get("caller"), s(spec, "proof")) {
            (Some(c), _) => serde_json::from_value(c.clone()).unwrap_or(Caller::System),
            (None, Some(name)) => self
                .script("proofs")
                .get(name)
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or(Caller::System),
            (None, None) => Caller::System,
        };
        let source = spec
            .get("source")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let req = |state, descriptor_id: Option<String>| Req {
            ticket,
            request_id: 0,
            handler: handler.clone(),
            kind,
            origin: s(spec, "origin").unwrap_or(kind.as_str()).to_string(),
            payload: spec.get("payload").cloned().unwrap_or(json!({})),
            caller: caller.clone(),
            source: source.clone(),
            descriptor_id,
            long: spec.get("long").and_then(Value::as_bool).unwrap_or(false),
            idempotency_key: spec.get("idempotency_key").is_some(),
            state,
            deadline_at: 0,
            conn: String::new(),
            class: crate::events::Class::Request,
            meta: None,
        };
        let p = &self.plugins[key];
        // HP LD-1, LD-4: no caller reaches a handler before `Loaded`, and none after unload starts.
        if !matches!(p.status, Status::Loaded) {
            self.refuse_now(
                key,
                ticket,
                ErrorKind::Unavailable,
                None,
                "the plugin is not loaded",
                true,
            );
            return Ok(ticket);
        }
        let _ = p;
        // HP IV-8: a queued client action carries the identity that it captured; it is revalidated at dispatch.
        let given = s(spec, "descriptor_id").map(String::from);
        // HP IV-4: the waiting queue is bounded by `invoke_queue`; a full queue refuses `backpressured` and never drops silently.
        let waiting = p
            .reqs
            .iter()
            .filter(|r| r.state == ReqState::Queued)
            .count() as u64;
        if waiting >= self.limits.invoke_queue {
            self.refuse_now(
                key,
                ticket,
                ErrorKind::Backpressured,
                None,
                "the invoke queue is full",
                true,
            );
            return Ok(ticket);
        }
        self.p(key).reqs.push(req(ReqState::Queued, given));
        Ok(ticket)
    }

    // ---- reload, unload, crash -------------------------------------------------------------------------------------------------------------------------

    /// HP LD-3 steps 4 to 7: drain, fence the old generation, activate the candidate, stop the old one.
    fn try_handover(&mut self, key: &str) {
        let p = &self.plugins[key];
        let Some((cand, ready)) = p.cand.clone() else {
            return;
        };
        let active: Vec<usize> = (0..p.reqs.len())
            .filter(|i| p.reqs[*i].state == ReqState::Active)
            .collect();
        if !active.is_empty() && !p.drain_expired {
            return;
        }
        // HP LD-3 (4): at the drain deadline the Hub sends `Cancel` for each admitted request and completes the caller `cancelled`; the effect is not zero (HP ER-5).
        for i in active {
            let (conn, rid) = (
                self.plugins[key].reqs[i].conn.clone(),
                self.plugins[key].reqs[i].request_id,
            );
            let epoch = self.epoch_of(&conn);
            self.send(
                &conn,
                Lane::Control,
                Frame::Cancel(Cancel {
                    epoch,
                    request_id: WireId::Num(rid),
                    reason: "reload".into(),
                }),
            );
            let a = Self::failure(
                ErrorKind::Cancelled,
                None,
                "cancelled at the reload drain deadline",
                "unknown",
                false,
            );
            self.finish(key, i, a);
        }
        let old = self.plugins[key].serving.clone();
        // HP LD-3 (5), LD-9: the old generation's write authority is fenced at the commit point.
        self.conns.get_mut(&old).expect("old conn").fenced = true;
        // HP LD-3 (6): the candidate is activated and the queued requests go to it.
        self.activate(key, &cand, &ready);
        self.p(key).reloading = false;
        self.p(key).cand = None;
        // HP LD-3 (7): the old generation is stopped with `Shutdown`.
        let epoch = self.epoch_of(&old);
        let deadline_ms = self.limits.shutdown;
        self.send(
            &old,
            Lane::Control,
            Frame::Shutdown(Shutdown {
                epoch,
                deadline_ms,
                reason: "reload".into(),
            }),
        );
        let now = self.now;
        let c = self.conns.get_mut(&old).expect("old conn");
        c.phase = Phase::Shutdown;
        c.shutdown_at = Some(now + deadline_ms);
    }

    /// HP LD-4: stop admitting, drain as in LD-3 steps 3 to 5, then `Shutdown{deadline}`.
    fn try_unload(&mut self, key: &str) {
        let p = &self.plugins[key];
        if p.status != Status::Unloading {
            return;
        }
        let conn = p.serving.clone();
        if self
            .conns
            .get(&conn)
            .is_none_or(|c| c.phase == Phase::Shutdown)
        {
            return;
        }
        let active: Vec<usize> = (0..p.reqs.len())
            .filter(|i| p.reqs[*i].state == ReqState::Active)
            .collect();
        if !active.is_empty() && !p.drain_expired {
            return;
        }
        for i in active {
            let (c, rid) = (
                self.plugins[key].reqs[i].conn.clone(),
                self.plugins[key].reqs[i].request_id,
            );
            let epoch = self.epoch_of(&c);
            self.send(
                &c,
                Lane::Control,
                Frame::Cancel(Cancel {
                    epoch,
                    request_id: WireId::Num(rid),
                    reason: "unload".into(),
                }),
            );
            let a = Self::failure(
                ErrorKind::Cancelled,
                None,
                "cancelled at the unload drain deadline",
                "unknown",
                false,
            );
            self.finish(key, i, a);
        }
        // HP LD-4: no `Invoke` is sent after `Shutdown`; a request still waiting is refused, nothing is stranded.
        let waiting: Vec<usize> = (0..self.plugins[key].reqs.len())
            .filter(|i| self.plugins[key].reqs[*i].state == ReqState::Queued)
            .collect();
        for i in waiting {
            let a = Self::failure(
                ErrorKind::Unavailable,
                None,
                "the plugin is unloading",
                "none",
                true,
            );
            self.finish(key, i, a);
        }
        let epoch = self.epoch_of(&conn);
        let deadline_ms = self.limits.shutdown;
        self.send(
            &conn,
            Lane::Control,
            Frame::Shutdown(Shutdown {
                epoch,
                deadline_ms,
                reason: "unload".into(),
            }),
        );
        let now = self.now;
        let c = self.conns.get_mut(&conn).expect("conn");
        c.phase = Phase::Shutdown;
        c.shutdown_at = Some(now + deadline_ms);
    }

    fn release_parked(&mut self, key: &str) {
        let parked = std::mem::take(&mut self.p(key).parked);
        for (conn, lane, c) in parked {
            let fenced = self.conns.get(&conn).is_some_and(|x| x.fenced);
            let epoch = self.epoch_of(&conn);
            // HP LD-9: the fence is checked atomically with the commit, so a call that was paused and is released after the fence is refused and never commits.
            let (result, effect) = if fenced {
                (
                    refusal(
                        ErrorKind::Unavailable,
                        Some("instance_fenced"),
                        "the instance is fenced",
                        false,
                    ),
                    Effect::None,
                )
            } else {
                self.host_answer(&c, true)
            };
            self.send(
                &conn,
                lane,
                Frame::HostResult(HostResult {
                    epoch,
                    call_id: c.call_id,
                    result,
                    effect,
                }),
            );
        }
    }

    fn on_closed(&mut self, conn: &str) {
        let (key, phase, cand) = {
            let c = &self.conns[conn];
            (c.plugin.clone(), c.phase, c.candidate)
        };
        let p = &self.plugins[&key];
        if phase == Phase::Shutdown {
            // HP LD-4: after `Shutdown` the channel ends and the plugin is unloaded.
            if p.serving == conn && p.status == Status::Unloading {
                let p = self.p(&key);
                p.status = Status::Unloaded;
                p.published = empty_published();
            }
            return;
        }
        if p.cand.as_ref().is_some_and(|(c, _)| c == conn) || (cand && (p.reloading || p.preparing))
        {
            return self.reload_failed(&key, "candidate_closed");
        }
        if p.serving != conn {
            return;
        }
        let cause = match (&self.conns[conn].fault, &p.exit_info) {
            (Some(f), _) => json!({"fault": f}),
            (None, Some(info)) => json!({"exit": info}),
            _ => json!({"lane_ended": true}),
        };
        match p.status {
            // HP CR-4: a crash during load fails the load; it is reported and not restarted by policy.
            Status::Loading => self.fail_load(&key, "plugin_crashed", vec![]),
            Status::Loaded | Status::Unloading => self.crash(&key, cause),
            _ => {}
        }
    }

    /// HP CR-2, CR-3.
    fn crash(&mut self, key: &str, cause: Value) {
        let active: Vec<usize> = (0..self.plugins[key].reqs.len())
            .filter(|i| self.plugins[key].reqs[*i].state == ReqState::Active)
            .collect();
        for i in active {
            // HP CR-2: every in-flight `Invoke` completes `failed{plugin_crashed}`; a mutation it had started is `effect: unknown`.
            let a = Self::failure(
                ErrorKind::Failed,
                Some("plugin_crashed"),
                "the plugin crashed",
                "unknown",
                false,
            );
            self.finish(key, i, a);
        }
        let now = self.now;
        let window = self.limits.restart_window;
        let p = self.p(key);
        p.parked.clear();
        p.preparing = false;
        p.reloading = false;
        p.cand = None;
        // HP CR-2: held subscriptions close and timers are dropped, because the registration is gone.
        p.published = empty_published();
        p.last_cause = cause;
        p.status = Status::Crashed;
        p.crash_times.push(now);
        let recent = p.crash_times.iter().filter(|t| now - **t < window).count();
        // HP CR-3: a crashed plugin restarts once; a second crash within `restart_window` quarantines it.
        if recent >= 2 {
            p.status = Status::Quarantined;
        } else {
            p.restart_requested += 1;
            p.status = Status::AwaitService("restarting");
        }
    }

    // ---- controls ------------------------------------------------------------------------------------------------------------------------------------

    fn target(&self, op: &Value) -> String {
        s(op, "plugin").unwrap_or(&self.default).to_string()
    }

    fn status_json(&self, key: &str) -> Value {
        let p = &self.plugins[key];
        let mut v = match &p.status {
            Status::Loading => json!({"state": "loading"}),
            Status::Loaded => json!({"state": "loaded"}),
            Status::Unloading => json!({"state": "unloading"}),
            Status::Unloaded => json!({"state": "unloaded"}),
            Status::LoadFailed { code, names } => {
                json!({"state": "load_failed", "code": code, "names": names})
            }
            Status::Crashed => json!({"state": "crashed"}),
            Status::AwaitService(what) => json!({"state": what}),
            Status::Quarantined => json!({"state": "quarantined"}),
        };
        v["restart_requested"] = json!(p.restart_requested);
        if !p.last_cause.is_null() {
            v["cause"] = p.last_cause.clone();
        }
        if let Some(e) = &p.reload_error {
            v["reload_error"] = json!(e);
        }
        v
    }

    fn do_cancel(&mut self, key: &str, ticket: u64) -> Value {
        let Some(idx) = self.plugins[key]
            .reqs
            .iter()
            .position(|r| r.ticket == ticket)
        else {
            return json!({"unknown_ticket": true});
        };
        match self.plugins[key].reqs[idx].state {
            ReqState::Done => json!({"already_complete": true}),
            // HP ER-5: a refusal before dispatch is `effect: none`.
            ReqState::Queued => {
                let a = Self::failure(
                    ErrorKind::Cancelled,
                    None,
                    "cancelled before dispatch",
                    "none",
                    false,
                );
                self.finish(key, idx, a);
                json!({"cancelled": "queued"})
            }
            ReqState::Active => {
                let (conn, rid) = (
                    self.plugins[key].reqs[idx].conn.clone(),
                    self.plugins[key].reqs[idx].request_id,
                );
                let epoch = self.epoch_of(&conn);
                self.send(
                    &conn,
                    Lane::Control,
                    Frame::Cancel(Cancel {
                        epoch,
                        request_id: WireId::Num(rid),
                        reason: "caller".into(),
                    }),
                );
                // HP ER-5: a cancellation after dispatch is `effect: unknown`, never reported as zero-effect.
                let a = Self::failure(
                    ErrorKind::Cancelled,
                    None,
                    "cancelled by the caller",
                    "unknown",
                    false,
                );
                self.finish(key, idx, a);
                json!({"cancelled": "active"})
            }
        }
    }

    fn scripted_control(&mut self, name: &str, op: &Value) -> Result<Value, Unsupported> {
        let Some(entry) = self.script("controls").get(name).cloned() else {
            return Err(Unsupported(name.to_string()));
        };
        // R-1.3: a semantic control sends the frames that the script lists. The fake invents none.
        for f in entry
            .get("send")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let conn = s(&f, "conn").unwrap_or("main").to_string();
            let epoch = self.epoch_of(&conn);
            let (lane, ty) = (
                f["lane"].as_u64().unwrap_or(0) as u8,
                f["type"].as_u64().unwrap_or(0) as u8,
            );
            let mut payload = f["payload"].clone();
            if let Value::Object(m) = &mut payload {
                m.insert("epoch".into(), json!(epoch.0));
            }
            let bytes = encode_raw(
                lane,
                ty,
                &serde_json::to_vec(&payload).expect("payload"),
                u32::MAX,
            )
            .expect("raw frame");
            if let (Some(c), Some(l)) = (self.conns.get_mut(&conn), Lane::from_byte(lane)) {
                c.link.send_wire(l, bytes);
            }
        }
        // A scripted control may withdraw or add a proof (a session restarted, a token issued).
        for name in entry
            .get("revoke")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            if let (Some(n), Some(Value::Object(m))) = (name.as_str(), self.cfg.get_mut("proofs")) {
                m.remove(n);
            }
        }
        if let (Some(Value::Object(add)), Some(Value::Object(m))) =
            (entry.get("grant"), self.cfg.get_mut("proofs"))
        {
            m.extend(add.clone());
        }
        let _ = op;
        Ok(entry.get("returns").cloned().unwrap_or(json!({})))
    }
}

impl FakeEndpoint for FakeHub {
    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        self.cfg = setup.clone();
        self.limits = test_limits(setup.get("limits").unwrap_or(&Value::Null))?;
        self.default = s(setup, "plugin_key").unwrap_or("demo").to_string();
        if setup.get("start").and_then(Value::as_bool) != Some(false) {
            let key = self.default.clone();
            self.start_plugin(
                &key,
                "main",
                &setup.get("host").cloned().unwrap_or(Value::Null),
            );
        }
        Ok(())
    }

    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.conns
            .get_mut(conn)
            .map_or(Ok(()), |c| c.link.feed(bytes))
    }

    fn read(&mut self, conn: &str) -> Option<Vec<u8>> {
        self.pump();
        let Self { conns, ch, .. } = self;
        conns.get_mut(conn)?.link.read(ch)
    }

    fn closed(&self, conn: &str) -> Option<&'static str> {
        self.conns.get(conn).and_then(|c| c.link.closed)
    }

    fn close_channel(&mut self, conn: &str) {
        if let Some(c) = self.conns.get_mut(conn) {
            // HP CR-1: a plugin that closes its channel is crashed; the Hub sees `LaneEnded`.
            c.link.close("peer_closed");
        }
        self.pump();
    }

    fn advance_clock(&mut self, ms: u64) {
        // HP IV-7: the clock moves; the timer wake runs at the next processing step, after the frames that are already waiting.
        self.now += ms;
        self.pump();
    }

    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        let name = s(op, "op").unwrap_or("").to_string();
        let key = self.target(op);
        let conn = s(op, "conn").map(String::from);
        let known = |this: &Self| {
            this.plugins.contains_key(&key)
                || name == "load_plugin"
                || this.script("controls").get(&name).is_some()
        };
        if !known(self) {
            return Err(Unsupported(name));
        }
        if !matches!(name.as_str(), "invoke" | "cancel") {
            self.pump();
        }
        let serving = self
            .plugins
            .get(&key)
            .map(|p| p.serving.clone())
            .unwrap_or_default();
        let conn = conn.unwrap_or(serving);
        Ok(match name.as_str() {
            "settle" => {
                self.pump();
                json!({})
            }
            "status" => self.status_json(&key),
            "completions" => Value::Array(
                self.plugins[&key]
                    .completions
                    .iter()
                    .map(|(t, a)| json!({"ticket": t, "answer": a}))
                    .collect(),
            ),
            "invoke" => {
                // HP IV-4, IV-6: concurrent callers are admitted in a seeded order.
                let list: Vec<Value> = match op.get("batch") {
                    Some(Value::Array(b)) => b.clone(),
                    _ => vec![op.clone()],
                };
                let mut order: Vec<usize> = (0..list.len()).collect();
                let mut admitted: Vec<u64> = vec![0; list.len()];
                while !order.is_empty() {
                    let at = self.ch.pick(Choice::ConcurrentInvoke, order.len());
                    let i = order.remove(at);
                    admitted[i] = self.admit(&key, &list[i]).map_err(Unsupported)?;
                }
                self.pump();
                if op.get("batch").is_some() {
                    json!({"tickets": admitted})
                } else {
                    json!({"ticket": admitted[0]})
                }
            }
            "cancel" => {
                let ticket = op["ticket"].as_u64().unwrap_or(0);
                // HP IV-7: when a `Result` and a `Cancel` cross, whichever the Hub processes first wins. A frame that is already written and not yet processed races the cancel.
                let crossing = self.conns.get(&conn).is_some_and(|c| c.link.has_frame());
                let cancel_first = crossing && self.ch.pick(Choice::CancelRace, 2) == 1;
                if cancel_first {
                    let r = self.do_cancel(&key, ticket);
                    self.pump();
                    r
                } else {
                    self.pump();
                    self.do_cancel(&key, ticket)
                }
            }
            "counters" => {
                let (mut stale, mut ignored, mut refused) = (0, 0, 0);
                for c in self.conns.values().filter(|c| c.plugin == key) {
                    stale += c.link.stale;
                    ignored += c.link.ignored;
                    refused += c.link.refused_buffered;
                }
                json!({"stale_dropped": stale, "ignored_advisory": ignored, "late_results": self.plugins[&key].late_results, "refused_frame_bytes_buffered": refused})
            }
            "received" => self.conns.get(&conn).map_or(json!([]), |c| {
                c.link.received(s(op, "lane").unwrap_or("control"))
            }),
            "published" => self.plugins[&key].published.clone(),
            "open_subscription" => {
                // HP CR-2, LD-6: a client holds a subscription to a family of the plugin; it closes when the plugin crashes or unloads.
                let family = s(op, "family").unwrap_or("family").to_string();
                let held = self.p(&key).published["held_subscriptions"]
                    .as_array_mut()
                    .expect("held list");
                held.push(json!(family));
                json!({})
            }
            "descriptors" => json!(self.plugins[&key].descriptors),
            "diagnostics" => {
                let code = match &self.plugins[&key].status {
                    Status::LoadFailed { code, .. } => json!(code),
                    _ => Value::Null,
                };
                json!({"reason": code, "bootstrap_logs": self.plugins[&key].bootstrap_logs})
            }
            "hold_lane" | "release_lane" => {
                let lane = s(op, "lane")
                    .and_then(lane_by_name)
                    .ok_or_else(|| Unsupported(format!("lane in {name}")))?;
                let on = name == "hold_lane";
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.link.hold(lane, on);
                }
                json!({"lane": lane_name(lane), "held": on})
            }
            "expire_drain" => {
                self.p(&key).drain_expired = true;
                self.pump();
                json!({})
            }
            "reload" => {
                if self.plugins[&key].status != Status::Loaded
                    || self.plugins[&key].reloading
                    || self.plugins[&key].preparing
                {
                    return Ok(json!({"refused": "not_loaded"}));
                }
                // HP LD-3 (1): the candidate starts as generation n+1 while the old generation keeps serving.
                let cand = s(op, "as").unwrap_or("cand").to_string();
                let p = self.p(&key);
                p.preparing = true;
                p.drain_expired = false;
                p.reload_error = None;
                self.open_conn(&key, &cand, false, true);
                json!({"candidate": cand})
            }
            "unload" => {
                // HP LD-4: stop admitting, then drain.
                self.p(&key).status = Status::Unloading;
                self.p(&key).drain_expired = false;
                self.pump();
                json!({})
            }
            "hold_commits" => {
                self.p(&key).hold_commits = true;
                json!({})
            }
            "release_commits" => {
                self.p(&key).hold_commits = false;
                self.release_parked(&key);
                json!({})
            }
            "service_exited" => {
                // HP CR-1: Core reports the exit (`ServiceState{Exited}`). It states a code or a signal and no limit cause (Core SV-5).
                let info = json!({"code": op.get("code"), "signal": op.get("signal")});
                let p = self.p(&key);
                p.exit_info = Some(info);
                let serving = p.serving.clone();
                self.stop_group(&serving, "exited");
                self.on_closed(&serving);
                self.plugins.get_mut(&key).expect("plugin").exit_info = None;
                json!({})
            }
            "hub_restart" => {
                // HP CR-5: the Hub process restarts. Its requests are gone; the guardian kept the service.
                let serving = self.plugins[&key].serving.clone();
                self.conns.remove(&serving);
                let p = self.p(&key);
                p.reqs.clear();
                p.parked.clear();
                p.status = Status::AwaitService("adopting");
                json!({})
            }
            "service_lost" => {
                // HP CR-5: a `Lost` plugin is started fresh.
                let p = self.p(&key);
                p.status = Status::AwaitService("starting");
                p.published = empty_published();
                json!({})
            }
            "service_connected" => {
                let resumed = self.plugins[&key].status == Status::AwaitService("adopting");
                let serving = self.plugins[&key].serving.clone();
                if !matches!(self.plugins[&key].status, Status::AwaitService(_)) {
                    return Ok(json!({"refused": "no service is awaited"}));
                }
                // HP HS-6, CR-3, CR-5: a restart is a new instance with a full handshake; an adopted service keeps its instance and gets a higher epoch with `resumed: true`.
                self.p(&key).status = Status::Loading;
                self.open_conn(&key, &serving, resumed, false);
                json!({"resumed": resumed})
            }
            "load_plugin" => {
                let k = s(op, "plugin_key").unwrap_or("other").to_string();
                let conn = s(op, "as").unwrap_or(&k).to_string();
                self.start_plugin(&k, &conn, op.get("host").unwrap_or(&Value::Null));
                json!({"conn": conn})
            }
            "emit_events" => crate::events::emit(self, &key, op),
            other => return self.scripted_control(other, op),
        })
    }

    fn controls(&self) -> &'static [&'static str] {
        CONTROLS
    }

    fn conns(&self) -> Vec<String> {
        self.conns.keys().cloned().collect()
    }
}
