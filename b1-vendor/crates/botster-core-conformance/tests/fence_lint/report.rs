//! **R-88 (the report-fence rule).** A **cause** makes a worker send a report. A **report-derived read** reads state or
//! events that the report sets. The read is a match while a cause that it depends on is pending, because Core may not
//! have consumed the report yet.
//!
//! - A cause has an identity: its handle, its session, its class, its operation (the variable and its binding), and its
//!   route. The classes are life, input, output, client (a route's input) and route (a route's own report).
//! - A cause is armed by the first later step that pumps (OR-1). Before that, Core has not run, so no report exists.
//! - A fence names its report, and clears only the causes that it proves. A `Completed {op}` or `op_posted {for_op}`
//!   clears that operation's binding only, on the fence's handle: a variable that a later `begin` binds again is another
//!   operation.
//! - A terminal state, or a `Completed` whose every successful alternative is `NotWritten(SessionEnded)`, ends the
//!   session's causes. Another report-made state proves only a `Start`: the other life causes report a terminal state,
//!   and a state from before them also matches. `parked_work {parked: true}` proves one frame, so it clears only the
//!   earliest armed life cause of its session. A second `parked_work` read of that session clears another cause
//!   only after a step that takes a mandatory event of the session (the steward's round-4 ruling on #73, EV-5b/d):
//!   until then it can read the same frame. A class K read, a `drain` or a `poll_all` does not end the witness.
//! - The R-74 held write is a write of at least `HELD_WRITE_FILL_BYTES` on a PTY with `kernel_bytes`. The real tier
//!   ignores `kernel_bytes`, so a smaller write can complete while `pty_queued` reads a byte.
//! - A held precondition (`start_held`, `held_reports`, the R-74 held write of `pty_queued`, `scheduler_hold`) suspends
//!   the causes that it holds. It clears nothing. At the release, the causes come back unarmed.
//! - A fence counts only with an expectation that proves the report. A state of `Created`, `Starting` or `Stopping` (Core
//!   sets those at `begin`), a variable, `$any` or no state proves nothing.
//! - An event fences when it ends a wait: the target of `pump_until`, `pump_collect`, `expect_eventually` or
//!   `expect_quiet_until`, or an ordered event `expect` / `expect_set` (the driver pumps until the next event). A `$one_of`
//!   fences only what each of its alternatives requires. `ignore` and `forbid` matchers never fence.
//! - The output fences are R-80's (`output_fences` and `output_fence_covers`: a plain `Activity` wait covers only one
//!   unfenced output byte, counted across steps). An ordered event `expect` waits as a `pump_until` does, so its target goes through the same
//!   function. An `EventsLost` with `kinds` also fences the output of its session.
//! - A route-frame `expect_quiet_until` (`on` a route) is no read: the worker writes route frames to the client, and Core
//!   consumes no report for them. `drain` is no read.
//! - Not causes: a `begin` with `expect_error` (Core refuses it at once), and a `Stop` of an ended session. A read's own
//!   `barrier` is not its cause either (the `until` wait ends on the barrier's completion). The arming rule gives that
//!   with no check of its own: the lint takes the read before the step's pumps, so the barrier's cause is not armed yet.
//!
//! The steward's rulings on the pairs that P3 added (R-88 lint pairs): `Detach` and `Remove` are causes, every op that the
//! host forwards as `HostMsg::Op` is an input cause (a fatal `Signal` also causes an exit report, which needs its own
//! `await_get`), and a `cancel` is a read of its own operation and of its session's end.

use super::*;
use botster_core_conformance::HELD_WRITE_FILL_BYTES;

pub const NO_REPORT_FENCE: &str = "no report fence, R-88";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Life,
    Input,
    Output,
    Client,
    Route,
}

const ALL: [Class; 5] = [
    Class::Life,
    Class::Input,
    Class::Output,
    Class::Client,
    Class::Route,
];

/// The states that Core sets at a `begin`, before any report.
const BEGIN_STATES: [&str; 3] = ["Created", "Starting", "Stopping"];
const TERMINAL_STATES: [&str; 3] = ["Exited", "Lost", "Failed"];
const LIFE_BEGINS: [&str; 3] = ["Start", "Stop", "Remove"];
/// The ops that the host forwards to a worker as `HostMsg::Op`, answered by `Done` (botster-core-host admit.rs, the
/// `Next::Forward` arms). For a session with no worker, Core answers the setters itself, so they are causes only after a
/// `Start` or `Adopt` on the handle.
const FORWARDED: [&str; 12] = [
    "WriteInput",
    "Signal",
    "ReadScreen",
    "ReadCursor",
    "ReadModeFlags",
    "CaptureSnapshot",
    "ReadFacts",
    "SetOutputTap",
    "Resize",
    "SetSizePolicy",
    "SetColorProfile",
    "SetNotificationPolicy",
];
const LIFE_CONTROLS: [&str; 3] = ["lose_worker", "process_end_worker", "break_control"];
/// The class M (mandatory) events that are a session's own (Core EV-1's class table). Only taking one of them frees
/// the mandatory room that a parked frame waits for (EV-5b, EV-5d); taking a class K, D or L event frees none.
const SESSION_MANDATORY: [&str; 5] = [
    "Completed",
    "SessionState",
    "RouteStalled",
    "RouteResumed",
    "RouteClosed",
];
/// The steps that pump, so they arm the causes before them (OR-1).
const PUMPING: [&str; 14] = [
    "pump",
    "pump_idle",
    "await_quiet",
    "pump_until",
    "pump_collect",
    "expect_eventually",
    "expect_quiet_until",
    "await_get",
    "await_title",
    "await_control",
    "await_pty",
    "expect",
    "expect_set",
    "wait_wake",
];
/// The keys of a step that modify its action.
const MODIFIERS: [&str; 10] = [
    "because",
    "bind",
    "bind_fields",
    "on",
    "route",
    "expect",
    "ignore",
    "until",
    "barrier",
    "expect_error",
];

/// The action key of a step: its first key that is not a modifier, or `expect` for an event expectation alone.
fn first_key(step: &Value) -> Option<&str> {
    let m = step.as_object()?;
    m.keys()
        .map(String::as_str)
        .find(|k| !MODIFIERS.contains(k))
        .or_else(|| m.contains_key("expect").then_some("expect"))
}

fn handle_of(spec: Option<&Value>) -> String {
    spec.and_then(|s| s.get("handle"))
        .and_then(Value::as_str)
        .unwrap_or("h")
        .to_string()
}

/// The handle of a step: the handle of its `on` object, or of its action; `h` when it names none.
fn step_handle(step: &Value) -> String {
    match step.get("on") {
        Some(on @ Value::Object(_)) => handle_of(Some(on)),
        _ => handle_of(first_key(step).and_then(|k| step.get(k))),
    }
}

/// The state name of a state matcher: a string, or the one key of an object.
fn state_name(st: &Value) -> Option<&str> {
    match st {
        Value::String(s) => Some(s),
        Value::Object(m) if m.len() == 1 => m.keys().next().map(String::as_str),
        _ => None,
    }
}

/// The report-made state that a matcher proves: none for no state, a variable, `$any`, a `$one_of` or a begin state.
fn report_state(st: Option<&Value>) -> Option<&str> {
    let n = state_name(st?)?;
    (!n.starts_with('$') && !BEGIN_STATES.contains(&n)).then_some(n)
}

/// The states that a `get` expectation accepts.
fn accepted_states(expect: Option<&Value>) -> Option<BTreeSet<String>> {
    let st = expect?.get("state")?;
    let alternatives = match st.get("$one_of").and_then(Value::as_array) {
        Some(a) => a.iter().collect(),
        None => vec![st],
    };
    Some(
        alternatives
            .into_iter()
            .filter_map(state_name)
            .map(str::to_string)
            .collect(),
    )
}

/// The alternatives of a matcher: every choice of one alternative at each `$one_of`.
fn alternatives(v: &Value) -> Vec<Value> {
    match v {
        Value::Object(m) => match m.get("$one_of").and_then(Value::as_array) {
            Some(a) if m.len() == 1 => a.iter().flat_map(alternatives).collect(),
            _ => m
                .iter()
                .fold(vec![serde_json::Map::new()], |acc, (k, x)| {
                    let xs = alternatives(x);
                    acc.iter()
                        .flat_map(|o| {
                            xs.iter().map(move |x| {
                                let mut o = o.clone();
                                o.insert(k.clone(), x.clone());
                                o
                            })
                        })
                        .collect()
                })
                .into_iter()
                .map(Value::Object)
                .collect(),
        },
        Value::Array(a) => a
            .iter()
            .fold(vec![vec![]], |acc, x| {
                let xs = alternatives(x);
                acc.iter()
                    .flat_map(|o| {
                        xs.iter().map(move |x| {
                            let mut o = o.clone();
                            o.push(x.clone());
                            o
                        })
                    })
                    .collect()
            })
            .into_iter()
            .map(Value::Array)
            .collect(),
        _ => vec![v.clone()],
    }
}

/// A `Completed` result matcher that every successful alternative of is `NotWritten(SessionEnded)`.
fn ends_session(result: &Value) -> bool {
    alternatives(result)
        .iter()
        .all(|a| a.to_string().contains(r#"{"NotWritten":"SessionEnded"}"#))
}

/// The least value that a number matcher accepts: a number, or the lower bound of `$between`.
fn lower_bound(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    v.as_u64().or_else(|| v.pointer("/$between/0")?.as_u64())
}

/// The classes of the reports that can post a named event.
fn event_classes(name: &str) -> Option<&'static [Class]> {
    Some(match name {
        "SessionState" | "Exited" | "Lost" => &[Class::Life],
        "TitleChanged" | "ModesChanged" | "Bell" | "Silent" | "Notification" | "ClipboardWrite"
        | "CwdChanged" | "PromptMark" => &[Class::Output],
        "EventsLost" => &ALL,
        "RouteClosed" | "SizeChanged" => &[Class::Route, Class::Life],
        "RouteStalled" | "RouteResumed" => &[Class::Route],
        "FocusChanged" => &[Class::Client],
        _ => return None,
    })
}

/// The named events `(name, body)` that every match of `v` requires. A `$one_of` requires only what each of its
/// alternatives requires (as R-80's `output_fences`). Any other `$` operator, and an `ignore` or `forbid` matcher,
/// requires nothing.
fn required_events(v: &Value) -> Vec<(String, Value)> {
    let mut out = vec![];
    match v {
        Value::Object(m) => {
            if let Some(alternatives) = m.get("$one_of").and_then(Value::as_array) {
                let mut each = alternatives.iter().map(required_events);
                if let Some(first) = each.next() {
                    out.extend(each.fold(first, |all, alt| {
                        all.into_iter().filter(|e| alt.contains(e)).collect()
                    }));
                }
            }
            for (k, x) in m {
                if k == "ignore" || k == "forbid" || k.starts_with('$') {
                    continue;
                }
                if k.starts_with(|c: char| c.is_ascii_uppercase()) && x.is_object() {
                    out.push((k.clone(), x.clone()));
                }
                out.extend(required_events(x));
            }
        }
        Value::Array(a) => a.iter().for_each(|x| out.extend(required_events(x))),
        _ => {}
    }
    out
}

/// An operation: the variable that a `begin` binds, and which binding of it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OpId {
    var: String,
    binding: usize,
}

#[derive(Debug, Clone)]
struct Op {
    id: OpId,
    handle: String,
    session: Option<String>,
    class: Option<Class>,
    kind: String,
}

/// A held precondition, keyed by what releases it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Hold {
    Start(String),
    Reports(String),
    Pty(String),
    Scheduler(String),
}

#[derive(Debug, Clone)]
struct Cause {
    step: usize,
    handle: String,
    session: String,
    class: Class,
    kind: String,
    op: Option<OpId>,
    route: Option<String>,
    /// For a client cause: no earlier client cause is on its route. `route_reports.activity_before_close` is sticky, so
    /// it proves only the first.
    first_on_route: bool,
    /// For a `WriteInput`: its payload is larger than the `kernel_bytes` of a blocked PTY (the R-74 held write).
    filler: bool,
    armed: bool,
    held: Option<Hold>,
}

impl Cause {
    fn new(step: usize, handle: &str, session: &str, class: Class, kind: &str) -> Cause {
        Cause {
            step,
            handle: handle.to_string(),
            session: session.to_string(),
            class,
            kind: kind.to_string(),
            op: None,
            route: None,
            first_on_route: false,
            filler: false,
            armed: false,
            held: None,
        }
    }
}

/// What a report-derived read accepts, beyond its classes.
enum Accepts {
    Any,
    /// The states that a `get` accepts. A `Start` or `Stop` whose begin state and report state are both accepted does
    /// not decide the read.
    States(BTreeSet<String>),
    /// A `cancel` reads its own operation, and its session's end (every life cause but a `Start`).
    Op(String),
}

struct Read {
    what: String,
    handle: String,
    sessions: Option<BTreeSet<String>>,
    classes: Vec<Class>,
    accepts: Accepts,
}

#[derive(Default)]
struct State {
    /// route variable -> (handle, session, binding) of its latest `attach_route`
    routes: HashMap<String, (String, String, String)>,
    /// (handle, route variable) -> the binding of its latest `attach_route` on that handle
    route_bindings: HashMap<(String, String), String>,
    /// binding -> the session that its `attach_route` attached
    binding_sessions: HashMap<String, String>,
    attaches: usize,
    /// variable -> its current operation
    ops: HashMap<String, Op>,
    bindings: HashMap<String, usize>,
    pending: Vec<Cause>,
    /// (handle, session) with a worker
    started: BTreeSet<(String, String)>,
    ended: BTreeSet<(String, String)>,
    /// The variables that a `scheduler_hold` holds.
    held_ops: BTreeSet<String>,
    /// (handle, session) -> the output that no output fence has met (R-80's `Unfenced`)
    unread: HashMap<(String, String), Unfenced>,
    /// session -> the `kernel_bytes` of its blocked PTY
    blocked: HashMap<String, u64>,
    client_sends: HashMap<String, usize>,
    /// The byte lengths that a `let` of `repeat` fixes.
    lengths: HashMap<String, u64>,
    /// (handle, session) whose parked frame a `parked_work` fence has used. No step since took a mandatory event of
    /// the session, so the frame can be the same one, and another `parked_work` read proves nothing more.
    parked_seen: BTreeSet<(String, String)>,
}

impl State {
    /// The handle, the session and the binding of a route variable. A cause names the binding, so a variable that a later
    /// `attach_route` binds again is another route.
    fn route(&self, r: Option<&Value>) -> Option<(String, String, String)> {
        let r = var_name(r?.as_str()?);
        self.routes.get(&r).cloned()
    }

    /// The binding that a route variable names in a fence on handle `h`.
    fn route_on(&self, h: &str, r: &str) -> Option<String> {
        self.route_bindings
            .get(&(h.to_string(), var_name(r)))
            .cloned()
    }

    fn attach(&mut self, h: String, r: String, session: String) {
        self.attaches += 1;
        let binding = format!("{r}#{}", self.attaches);
        self.route_bindings
            .insert((h.clone(), r.clone()), binding.clone());
        self.binding_sessions
            .insert(binding.clone(), session.clone());
        self.routes.insert(r, (h, session, binding));
    }

    fn current(&self, var: &str) -> Option<&Op> {
        self.ops.get(&var_name(var))
    }

    fn payload_len(&self, body: &Value) -> Option<u64> {
        let hex = body.pointer("/payload/bytes/bytes/$bytes_hex")?.as_str()?;
        if is_variable(hex) {
            self.lengths.get(&var_name(hex)).copied()
        } else {
            Some(hex.len() as u64 / 2)
        }
    }

    /// The causes of a step. `op` is the operation that the step's `begin` binds.
    fn causes(&mut self, i: usize, step: &Value, op: Option<OpId>) -> Vec<Cause> {
        let mut out = vec![];
        let h = step_handle(step);
        if let Some((name, body)) = step
            .pointer("/begin/op")
            .and_then(Value::as_object)
            .and_then(|m| m.iter().next())
        {
            if step.get("expect_error").is_none() {
                let session = body
                    .get("id")
                    .or_else(|| body.get("session"))
                    .and_then(Value::as_str);
                let name = name.as_str();
                if name == "Detach" {
                    if let Some((rh, rs, r)) = self.route(body.get("route")) {
                        out.push(Cause {
                            op,
                            route: Some(r),
                            ..Cause::new(i, &rh, &rs, Class::Route, name)
                        });
                    }
                } else if let Some(s) = session {
                    if LIFE_BEGINS.contains(&name) {
                        out.push(Cause {
                            op,
                            ..Cause::new(i, &h, s, Class::Life, name)
                        });
                    } else if FORWARDED.contains(&name)
                        && self.started.contains(&(h.clone(), s.to_string()))
                    {
                        let mut c = Cause {
                            op,
                            ..Cause::new(i, &h, s, Class::Input, name)
                        };
                        c.filler = name == "WriteInput"
                            && self.blocked.get(s).is_some_and(|&k| {
                                self.payload_len(body)
                                    .is_some_and(|n| n > k && n >= HELD_WRITE_FILL_BYTES)
                            });
                        out.push(c);
                        if name == "Signal" {
                            out.push(Cause::new(i, &h, s, Class::Life, "Signal exit"));
                        }
                    }
                }
            }
        }
        if let Some(c) = step.get("control") {
            let cop = c.get("op").and_then(Value::as_str).unwrap_or("");
            if let Some(s) = c.get("session").and_then(Value::as_str) {
                if LIFE_CONTROLS.contains(&cop) {
                    out.push(Cause::new(i, &h, s, Class::Life, cop));
                }
                if PROGRAM_OUTPUT_OPS.contains(&cop) {
                    out.push(Cause::new(i, &h, s, Class::Output, cop));
                    let key = (h.clone(), s.to_string());
                    let out = Unfenced::after(self.unread.get(&key), c);
                    self.unread.insert(key, out);
                }
            }
        }
        for k in ["route_send", "route_send_raw"] {
            if let Some(send) = step.get(k) {
                let r = Value::String(route_of(step).to_string());
                if let Some((rh, rs, r)) = self.route(Some(&r)) {
                    let focus = send.get("kind").and_then(Value::as_str) == Some("focus");
                    let kind = if focus { "focus" } else { k };
                    let mut c = Cause {
                        route: Some(r.clone()),
                        ..Cause::new(i, &rh, &rs, Class::Client, kind)
                    };
                    let n = self.client_sends.entry(r).or_default();
                    c.first_on_route = *n == 0;
                    *n += 1;
                    out.push(c);
                }
            }
        }
        out
    }

    fn clear(&mut self, pred: impl Fn(&Cause) -> bool) {
        self.pending.retain(|x| !pred(x));
    }

    fn terminal(&mut self, h: &str, s: &str) {
        self.clear(|x| x.handle == h && x.session == s);
        self.ended.insert((h.to_string(), s.to_string()));
    }

    /// A report-made state of a session on a handle: a terminal state ends every cause of the session. Another state
    /// proves only a `Start`'s report: every other life cause reports a terminal state, and a state from before that
    /// cause also matches.
    fn state_fence(&mut self, h: &str, s: &str, state: Option<&Value>) {
        match report_state(state) {
            Some(n) if TERMINAL_STATES.contains(&n) => self.terminal(h, s),
            Some(_) => self.clear(|x| {
                x.handle == h && x.session == s && x.class == Class::Life && x.kind == "Start"
            }),
            None => {}
        }
    }

    /// The fence of one event that ends a wait on handle `h`. The output events are R-80's (`output_fence`).
    fn event_fence(&mut self, h: &str, name: &str, body: &Value) {
        let id = body.get("id").and_then(Value::as_str).unwrap_or("");
        match name {
            "Completed" => {
                let Some(op) = body
                    .get("op")
                    .and_then(Value::as_str)
                    .and_then(|v| self.current(v))
                    .filter(|op| op.handle == h)
                    .cloned()
                else {
                    return;
                };
                self.clear(|x| x.op.as_ref() == Some(&op.id));
                let ended = body.get("result").is_some_and(ends_session);
                if let Some(s) = &op.session {
                    if op.kind == "Remove" || ended {
                        self.terminal(&op.handle, s);
                    }
                }
            }
            "SessionState" => self.state_fence(h, id, body.get("state")),
            "Activity" => match body.get("source") {
                Some(Value::String(src)) if src == "Client" => {
                    self.clear(|x| x.handle == h && x.session == id && x.class == Class::Client)
                }
                Some(Value::Object(src)) => {
                    if let Some(r) = src.get("Client").and_then(Value::as_str) {
                        let r = self.route_on(h, r);
                        self.clear(|x| {
                            x.handle == h
                                && x.session == id
                                && x.class == Class::Client
                                && r.is_some()
                                && x.route == r
                        });
                    }
                }
                _ => {}
            },
            "EventsLost" if body.get("kinds").is_some() => {
                self.clear(|x| x.handle == h && x.session == id && x.class == Class::Output);
                self.unread.remove(&(h.to_string(), id.to_string()));
            }
            "FocusChanged" => self.clear(|x| x.handle == h && x.session == id && x.kind == "focus"),
            "RouteClosed" | "RouteStalled" | "RouteResumed" => {
                if let Some(r) = body.get("route").and_then(Value::as_str) {
                    let r = self.route_on(h, r);
                    self.clear(|x| {
                        x.handle == h && x.class == Class::Route && r.is_some() && x.route == r
                    });
                }
            }
            _ => {}
        }
    }

    /// R-80's output fences of a wait on handle `h`.
    fn output_fence(&mut self, h: &str, wait: &Value) {
        for (s, event) in output_fences(wait) {
            let key = (h.to_string(), s.clone());
            let fences = self
                .unread
                .get(&key)
                .is_some_and(|out| output_fence_covers(event, out));
            if fences {
                self.clear(|x| x.handle == h && x.session == s && x.class == Class::Output);
                self.unread.remove(&key);
            }
        }
    }

    fn wait(&mut self, h: &str, target: &Value, as_wait: &Value) {
        self.output_fence(h, as_wait);
        for (name, body) in required_events(target) {
            self.takes(h, &name, &body);
            self.event_fence(h, &name, &body);
        }
    }

    /// A step on handle `h` takes the event `name`: a mandatory event of a session ends its `parked_work` witness,
    /// because it can free the room that the parked frame waits for. The parked work's own event is one of them.
    /// The event names the session on `h` only, as `event_fence` reads it: a `Completed` of an operation on `h`, and a
    /// route event of the route's binding on `h`.
    fn takes(&mut self, h: &str, name: &str, body: &Value) {
        if !SESSION_MANDATORY.contains(&name) {
            return;
        }
        let session = match name {
            "SessionState" => body.get("id").and_then(Value::as_str).map(str::to_string),
            "Completed" => body
                .get("op")
                .and_then(Value::as_str)
                .and_then(|v| self.current(v))
                .filter(|op| op.handle == h)
                .and_then(|op| op.session.clone()),
            _ => body
                .get("route")
                .and_then(Value::as_str)
                .and_then(|r| self.route_on(h, r))
                .and_then(|b| self.binding_sessions.get(&b).cloned()),
        };
        if let Some(s) = session {
            self.parked_seen.remove(&(h.to_string(), s));
        }
    }

    fn hold(&mut self, pred: impl Fn(&Cause) -> bool, hold: Hold) {
        for x in self.pending.iter_mut().filter(|x| pred(x)) {
            x.held = Some(hold.clone());
        }
    }

    fn release(&mut self, hold: &Hold) {
        for x in self.pending.iter_mut() {
            if x.held.as_ref() == Some(hold) {
                x.held = None;
                x.armed = false;
            }
        }
    }

    fn fences(&mut self, step: &Value) {
        let Some(k) = first_key(step) else { return };
        let spec = &step[k];
        let h = step_handle(step);
        let on_route = step.get("on").is_some_and(Value::is_string);
        match k {
            "await_get" => {
                let s = spec.get("id").and_then(Value::as_str).unwrap_or("");
                if spec.get("expect_error").is_some() {
                    self.terminal(&h, s);
                } else {
                    self.state_fence(&h, s, spec.pointer("/expect/state"));
                }
            }
            "await_control" => self.control_fence(&h, spec),
            "poll_events" | "poll_find" => {
                let named = step.get("expect").into_iter().chain(spec.get("event"));
                for (name, body) in named.flat_map(required_events) {
                    self.takes(&h, &name, &body);
                }
            }
            "await_title" => self.output_fence(&h, step),
            "pump_until" | "pump_collect" => self.wait(&h, spec, step),
            "expect_eventually" => {
                self.wait(&h, spec, &json!({"pump_until": {"event": spec}}));
            }
            "expect" if !on_route => {
                self.wait(&h, spec, &json!({"pump_until": {"event": spec}}));
            }
            "expect_set" if !on_route => {
                self.wait(&h, spec, &json!({"pump_collect": {"events": spec}}));
            }
            "expect_quiet_until" if !on_route => {
                if let Some(until) = step.get("until") {
                    self.wait(&h, until, &json!({"pump_until": {"event": until}}));
                }
            }
            _ if k.starts_with("await_") => self.output_fence(&h, step),
            _ => {}
        }
    }

    fn control_fence(&mut self, h: &str, spec: &Value) {
        let c = &spec["control"];
        let e = &spec["expect"];
        let op = c.get("op").and_then(Value::as_str).unwrap_or("");
        let s = c
            .get("session")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match op {
            // Core read the frame and holds it (R-87): the report's effect waits only for queue room.
            // It proves one frame, so it clears only the earliest armed life cause, and a second read of the same frame
            // clears nothing.
            "parked_work" if e.get("parked") == Some(&Value::Bool(true)) => {
                if !self.parked_seen.insert((h.to_string(), s.clone())) {
                    return;
                }
                let first = self
                    .pending
                    .iter()
                    .filter(|x| {
                        x.handle == h && x.session == s && x.class == Class::Life && x.armed
                    })
                    .map(|x| x.step)
                    .min();
                self.clear(|x| {
                    Some(x.step) == first
                        && x.handle == h
                        && x.session == s
                        && x.class == Class::Life
                })
            }
            "start_held" if e.get("held") == Some(&Value::Bool(true)) => self.hold(
                |x| x.handle == h && x.session == s && x.kind == "Start",
                Hold::Start(s.clone()),
            ),
            "held_reports" if lower_bound(e.get("count")).is_some_and(|n| n >= 1) => self.hold(
                |x| x.handle == h && x.session == s && x.class == Class::Life,
                Hold::Reports(s.clone()),
            ),
            // R-74's held write: a blocked PTY, a write larger than its kernel bytes, and a queue of at least one byte.
            // The queue holds that write and every write after it.
            "pty_queued" if lower_bound(e.get("bytes")).is_some_and(|n| n >= 1) => {
                let filler = self
                    .pending
                    .iter()
                    .filter(|x| x.handle == h && x.session == s && x.filler)
                    .map(|x| x.step)
                    .min();
                if let (Some(first), true) = (filler, self.blocked.contains_key(&s)) {
                    self.hold(
                        |x| {
                            x.handle == h
                                && x.session == s
                                && x.kind == "WriteInput"
                                && x.step >= first
                        },
                        Hold::Pty(s.clone()),
                    );
                }
            }
            "route_reports" => {
                let r = c
                    .get("route")
                    .and_then(Value::as_str)
                    .and_then(|r| self.route_on(h, r));
                if e.get("activity_before_close") == Some(&Value::Bool(true)) {
                    self.clear(|x| {
                        x.handle == h
                            && x.session == s
                            && x.class == Class::Client
                            && x.first_on_route
                            && r.is_some()
                            && x.route == r
                    });
                }
                if e.get("closed") == Some(&Value::Bool(true)) {
                    self.clear(|x| {
                        x.handle == h
                            && x.session == s
                            && x.class == Class::Route
                            && r.is_some()
                            && x.route == r
                    });
                }
            }
            "op_posted" if e.get("posted") == Some(&Value::Bool(true)) => {
                if let Some(op) = c
                    .get("for_op")
                    .and_then(Value::as_str)
                    .and_then(|v| self.current(v))
                    .filter(|op| op.handle == h)
                    .map(|op| op.id.clone())
                {
                    self.clear(|x| x.op.as_ref() == Some(&op));
                }
            }
            _ => {}
        }
    }

    /// The releases and the holds that a `control` step makes, before its read.
    fn controls(&mut self, step: &Value) {
        let Some(c) = step.get("control") else { return };
        let op = c.get("op").and_then(Value::as_str).unwrap_or("");
        let s = c
            .get("session")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let for_op = c.get("for_op").and_then(Value::as_str).map(var_name);
        match op {
            "release_start" | "release_start_at" => self.release(&Hold::Start(s)),
            "release_worker_report" => self.release(&Hold::Reports(s)),
            "pty_blocked" if c.get("on") == Some(&Value::Bool(false)) => {
                self.blocked.remove(&s);
                self.release(&Hold::Pty(s));
            }
            "pty_blocked" if c.get("on") == Some(&Value::Bool(true)) => {
                if let Some(k) = c.get("kernel_bytes").and_then(Value::as_u64) {
                    self.blocked.insert(s, k);
                }
            }
            "scheduler_hold" => {
                if let Some(v) = for_op {
                    self.held_ops.insert(v.clone());
                    let id = self.current(&v).map(|op| op.id.clone());
                    self.hold(|x| id.is_some() && x.op == id, Hold::Scheduler(v));
                }
            }
            "scheduler_release" => {
                if let Some(v) = for_op {
                    if self.held_ops.remove(&v) {
                        self.release(&Hold::Scheduler(v));
                    }
                }
            }
            _ => {}
        }
    }

    /// The report-derived read of a step, if any. `prev` is the step before it.
    fn read(&self, step: &Value, prev: Option<&Value>) -> Option<Read> {
        let k = first_key(step)?;
        let h = step_handle(step);
        let read =
            |what: &str, sessions: Option<BTreeSet<String>>, classes: &[Class], accepts| Read {
                what: what.to_string(),
                handle: h.clone(),
                sessions,
                classes: classes.to_vec(),
                accepts,
            };
        let one = |s: Option<&str>| s.map(|s| BTreeSet::from([s.to_string()]));
        if let Some(call) = step.get("call") {
            let args = &call["args"];
            match call.get("method").and_then(Value::as_str)? {
                "get" if step.get("expect").is_some() || step.get("expect_error").is_some() => {
                    let s = args
                        .get("id")
                        .or_else(|| args.get("session"))
                        .and_then(Value::as_str);
                    let accepts =
                        accepted_states(step.get("expect")).map_or(Accepts::Any, Accepts::States);
                    return Some(read("call get", one(s), &[Class::Life], accepts));
                }
                m @ ("list" | "status") => {
                    return Some(read(
                        &format!("call {m}"),
                        None,
                        &[Class::Life],
                        Accepts::Any,
                    ))
                }
                "cancel" => {
                    // A cancel that accepts both the early and the late answer does not depend on the report.
                    let answers: BTreeSet<&str> = step
                        .pointer("/expect/$one_of")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    if answers.contains("Admitted") && answers.contains("TooLate") {
                        return None;
                    }
                    let v = args.get("op").and_then(Value::as_str)?;
                    let s = self.current(v).and_then(|op| op.session.as_deref());
                    return Some(read(
                        "call cancel",
                        one(s),
                        &[Class::Input, Class::Life],
                        Accepts::Op(var_name(v)),
                    ));
                }
                _ => return None,
            }
        }
        if let Some(c) = step.get("control") {
            let op = c.get("op").and_then(Value::as_str).unwrap_or("");
            let checked = ["expect", "bind", "bind_fields"]
                .iter()
                .any(|m| step.get(m).is_some());
            if [
                "route_reports",
                "parked_work",
                "held_reports",
                "registry_row",
            ]
            .contains(&op)
                && checked
            {
                let s = c.get("session").and_then(Value::as_str);
                return Some(read(&format!("control {op}"), one(s), &ALL, Accepts::Any));
            }
        }
        match k {
            "expect_quiet_until" if !step.get("on").is_some_and(Value::is_string) => {
                let (classes, sessions) = self.scope(std::slice::from_ref(&step[k]));
                Some(read(k, sessions, &classes, Accepts::Any))
            }
            "forbid_events" if step[k].get("clear").is_none() => {
                let events = step[k]
                    .get("events")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let (classes, sessions) = self.scope(&events);
                Some(read(k, sessions, &classes, Accepts::Any))
            }
            "wait_wake" => {
                let spurious = prev.and_then(|p| control(p, "wake_spurious")).is_some();
                (!spurious).then(|| read(k, None, &ALL, Accepts::Any))
            }
            "poll_events" | "poll_all" | "poll_find" | "collect_events" => {
                Some(read(k, None, &ALL, Accepts::Any))
            }
            _ if step.get("pump").is_some()
                && ["events_posted", "more"]
                    .iter()
                    .any(|f| step.pointer(&format!("/expect/{f}")).is_some()) =>
            {
                Some(read("pump expect", None, &ALL, Accepts::Any))
            }
            _ => None,
        }
    }

    /// The classes and sessions that a list of named-event matchers can see posted; every class and every session when
    /// a matcher names no event that the lint knows.
    fn scope(&self, matchers: &[Value]) -> (Vec<Class>, Option<BTreeSet<String>>) {
        let unknown = (ALL.to_vec(), None);
        let mut events: Vec<(String, Value)> = vec![];
        for m in matchers {
            match m.get("$one_of").and_then(Value::as_array) {
                Some(alternatives) => {
                    for a in alternatives {
                        match single_event(a) {
                            Some(e) => events.push(e),
                            None => return unknown,
                        }
                    }
                }
                None => match single_event(m) {
                    Some(e) => events.push(e),
                    None => return unknown,
                },
            }
        }
        let mut classes = vec![];
        let mut sessions = Some(BTreeSet::new());
        for (name, body) in &events {
            let op = body
                .get("op")
                .and_then(Value::as_str)
                .and_then(|v| self.current(v));
            match name.as_str() {
                "Activity" => match body.get("source") {
                    Some(Value::String(s)) if s == "Output" => classes.push(Class::Output),
                    Some(Value::String(s)) if s == "Client" => classes.push(Class::Client),
                    Some(Value::Object(_)) => classes.push(Class::Client),
                    _ => classes.extend([Class::Output, Class::Client]),
                },
                "Completed" => match op.and_then(|op| op.class) {
                    Some(c) => classes.push(c),
                    None => classes.extend(ALL),
                },
                n => match event_classes(n) {
                    Some(c) => classes.extend_from_slice(c),
                    None => return unknown,
                },
            }
            let session = body
                .get("id")
                .or_else(|| body.get("session"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| self.route(body.get("route")).map(|(_, s, _)| s))
                .or_else(|| {
                    (name == "Completed")
                        .then(|| op.and_then(|op| op.session.clone()))
                        .flatten()
                });
            match session {
                Some(s) if !is_variable(&s) => {
                    if let Some(ss) = sessions.as_mut() {
                        ss.insert(s);
                    }
                }
                _ => sessions = None,
            }
        }
        (classes, sessions)
    }

    /// The latest pending cause that a read depends on.
    fn matches(&self, r: &Read) -> Option<&Cause> {
        self.pending
            .iter()
            .filter(|x| {
                x.handle == r.handle
                    && x.armed
                    && x.held.is_none()
                    && r.classes.contains(&x.class)
                    && r.sessions.as_ref().is_none_or(|ss| ss.contains(&x.session))
            })
            .filter(|x| match &r.accepts {
                Accepts::Any => true,
                Accepts::Op(v) => {
                    let own = self.current(v).map(|op| &op.id);
                    (own.is_some() && x.op.as_ref() == own)
                        || (x.class == Class::Life && x.kind != "Start")
                }
                Accepts::States(states) => {
                    let pair = match x.kind.as_str() {
                        "Start" => Some(("Starting", "Running")),
                        "Stop" => Some(("Stopping", "Exited")),
                        _ => None,
                    };
                    !pair.is_some_and(|(b, e)| states.contains(b) && states.contains(e))
                }
            })
            .max_by_key(|x| x.step)
    }

    /// The bookkeeping of a step's `begin`: a new binding of its variable, and the handles with a worker.
    fn bind(&mut self, step: &Value) -> Option<OpId> {
        let begin = step.pointer("/begin/op")?;
        let h = step_handle(step);
        if begin.as_str() == Some("AdoptAll") {
            // Every adopted session has a worker on this handle.
            let sessions: Vec<String> = self.started.iter().map(|(_, s)| s.clone()).collect();
            self.started
                .extend(sessions.into_iter().map(|s| (h.clone(), s)));
            return None;
        }
        let (name, body) = begin.as_object()?.iter().next()?;
        if (name == "Start" || name == "Adopt") && step.get("expect_error").is_none() {
            if let Some(s) = body
                .get("id")
                .or_else(|| body.get("session"))
                .and_then(Value::as_str)
            {
                self.started.insert((h.clone(), s.to_string()));
            }
        }
        let var = var_name(step.get("bind")?.as_str()?);
        let n = self.bindings.entry(var.clone()).or_default();
        *n += 1;
        let id = OpId {
            var: var.clone(),
            binding: *n,
        };
        let session = body
            .get("id")
            .or_else(|| body.get("session"))
            .and_then(Value::as_str);
        let (handle, session, class) = if name == "Detach" {
            match self.route(body.get("route")) {
                Some((rh, rs, _)) => (rh, Some(rs), Some(Class::Route)),
                None => (h, None, None),
            }
        } else if LIFE_BEGINS.contains(&name.as_str()) {
            (h, session.map(str::to_string), Some(Class::Life))
        } else {
            let worker =
                session.is_some_and(|s| self.started.contains(&(h.clone(), s.to_string())));
            let class = (FORWARDED.contains(&name.as_str()) && worker).then_some(Class::Input);
            (h, session.map(str::to_string), class)
        };
        self.ops.insert(
            var,
            Op {
                id: id.clone(),
                handle,
                session,
                class,
                kind: name.clone(),
            },
        );
        Some(id)
    }
}

/// The one named event of a matcher: an object with one key that starts with a capital letter.
fn single_event(m: &Value) -> Option<(String, Value)> {
    let o = m.as_object().filter(|o| o.len() == 1)?;
    let (k, v) = o.iter().next()?;
    (k.starts_with(|c: char| c.is_ascii_uppercase()) && v.is_object())
        .then(|| (k.clone(), v.clone()))
}

/// The R-88 findings of a transcript: one for each report-derived read that a pending cause decides.
pub fn lint(id: &str, steps: &[Value]) -> Vec<Finding> {
    let mut out = vec![];
    let mut t = State::default();
    let mut prev: Option<Value> = None;
    for (i, step) in steps.iter().enumerate() {
        let mut parts = vec![];
        if let Some(b) = step.get("barrier") {
            parts.push((b.clone(), true));
        }
        let mut own = step.clone();
        if let Some(m) = own.as_object_mut() {
            m.remove("barrier");
        }
        parts.push((own, false));
        for (s, is_barrier) in parts {
            if let Some(a) = s.get("attach_route") {
                let r = a
                    .get("route")
                    .and_then(Value::as_str)
                    .map_or("r1".to_string(), var_name);
                let session = a
                    .get("session")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                t.attach(handle_of(Some(a)), r, session);
            }
            if let Some(name) = s.pointer("/let").and_then(Value::as_object) {
                for (var, v) in name {
                    let len = v
                        .pointer("/repeat/0")
                        .and_then(Value::as_str)
                        .zip(v.pointer("/repeat/1").and_then(Value::as_u64))
                        .map(|(hex, n)| hex.len() as u64 / 2 * n);
                    match len {
                        Some(n) => t.lengths.insert(var_name(var), n),
                        None => t.lengths.remove(&var_name(var)),
                    };
                }
            }
            t.controls(&s);
            if let Some(r) = t.read(&s, prev.as_ref()) {
                if let Some(x) = t.matches(&r) {
                    let sessions = r
                        .sessions
                        .as_ref()
                        .map_or("every session".to_string(), |ss| {
                            ss.iter().cloned().collect::<Vec<_>>().join(", ")
                        });
                    out.push(Finding {
                        id: id.to_string(),
                        step: i,
                        rule: NO_REPORT_FENCE,
                        what: format!(
                            "{} of {sessions} on {} at step {i} reads a report of the {} of {} at step {} ({:?}) with no fence of it (R-88)",
                            r.what, r.handle, x.kind, x.session, x.step, x.class
                        ),
                    });
                }
            }
            t.fences(&s);
            if first_key(&s).is_some_and(|k| PUMPING.contains(&k)) || s.get("pump").is_some() {
                for x in t.pending.iter_mut() {
                    x.armed = true;
                }
            }
            let op = t.bind(&s);
            for mut x in t.causes(i, &s, op) {
                if x.kind == "Stop" && t.ended.contains(&(x.handle.clone(), x.session.clone())) {
                    continue;
                }
                if let Some(v) = x.op.as_ref().map(|op| op.var.clone()) {
                    if t.held_ops.contains(&v) {
                        x.held = Some(Hold::Scheduler(v));
                    }
                }
                t.pending.push(x);
            }
            if !is_barrier {
                prev = Some(s);
            }
        }
    }
    out
}
