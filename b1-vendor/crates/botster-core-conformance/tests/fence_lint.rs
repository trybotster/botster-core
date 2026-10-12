//! The fence and line-discipline lint of the Core transcripts (steward rulings R-56, R-57, R-60, R-61, R-62 Q3 and R-83).
//!
//! - **R-56.** A `pty_input` read that asserts or captures a log follows an `await_pty` of its session, with no write to that
//!   session between them. For a literal log, `min_bytes` is at least the log's length. An empty literal is an absence check, which
//!   races the other way, so it is not this pattern.
//! - **R-57.** An `await_title` has an earlier `pty_output` of its session that sets exactly that title (`OSC 0` or `OSC 2`, ended by
//!   `ST` or `BEL`). No `Start`, `Adopt`, `AdoptAll` or `open` falls between that sentinel and the wait. The vocabulary does not say
//!   whether an `open` adopts, so every `open` counts.
//! - **R-57 sweep (R-61 (a), PR C).** After a session's last `pty_output`, the first step that reads the session's events or state
//!   (`defs::output_read`) either waits for a new value, which the output alone can produce, or follows an `await_title` of the
//!   session. A read that does not wait, or a wait for an `EventsLost` marker that the output extends, needs that fence.
//! - **The attach rule (R-61 (a)(1), the steward's ou_9 ruling).** An attach takes its baseline at once, as a sync call of the
//!   session does. So a route whose exact output (R-61 (c)) a later step asserts is attached after an `await_title` of every
//!   earlier `pty_output` of its session, on any handle, that no wait has met. Otherwise the worker can read that output after
//!   the baseline, and the route receives it as live output.
//! - **The model-read rule (R-80, which replaces R-77's guard rule; the class of R-57 and R-61 (a)(1)).** After a
//!   `pty_output`, `pty_chunk` or `program_write_once` of a session, a model read of the session (`ReadScreen`,
//!   `CaptureSnapshot`, `terminal_state`, or a `begin` whose operation has a `model_rev` guard) follows an output fence of the
//!   session: a `pump_until`, `pump_collect` or `await_*` wait for `Activity {id: S, source: Output}`, or an `await_title` of
//!   the session. `await_quiet`, `drain`, the clock, `pump`, `let`, `check`, another session's wait, and the read's own reply
//!   wait are no fences. The rule does not judge whether an assertion depends on the output: a match is fenced, or listed
//!   with a reason.
//! - **R-60.** An `await_control` of `program_ready` comes before the first host input to a session whose output or input log a later
//!   step asserts. An exact output assertion contains no `0a`, and an exact input assertion contains no `0d` (R-61 (c)). The
//!   `pty_output` bytes of a session that a later `oracle_resume` reads contain no `0a` (R-62 Q3).
//! - **R-83 (the route read rule).** A `route_expect_pty` reads its route's session's input log, so it follows an `await_pty` of
//!   that session, with no write to it between them. A fence that the lint knows covers the read's floor (`defs::route_pty_least`);
//!   a run-time count (a route matcher's `written_bytes`, or an `oracle_encode` sum) is the reviewer's check. Only an empty
//!   sequence is an absence check.
//! - **R-88 (the report-fence rule).** A read of state or events that a worker's report sets follows a fence that proves
//!   Core consumed that report. The rule is in `fence_lint/report.rs`, with its definitions.
//!
//! A step is a top-level step of the transcript, numbered from 0 as the runner numbers it. Its `barrier` is part of it.
//!
//! The exception list `conformance/core-fence-exceptions.txt` may only shrink. The lint fails on a finding that the list does not
//! name, and on an entry that no finding needs. Only the R-56 capture rule, the `program_ready` rule, the R-57 sweep, the
//! model-read rule, the R-83 route read rule and the R-88 report-fence rule take exceptions. The route read rule takes one only: the transcript that R-81
//! rewrites. The attach rule takes none: the steward names any exception to it.

use std::collections::{BTreeSet, HashMap};

use botster_conformance::matcher::is_variable;
use serde_json::{json, Value};

/// The R-88 report-fence rule.
#[path = "fence_lint/report.rs"]
mod report;

/// The definitions of the lint (steward ruling R-61), in one place.
mod defs {
    use super::*;

    /// The sessions that a step writes into (R-61 (b)): a `WriteInput` that names the session, a `route_send` or `route_send_raw` on
    /// a route attached to it, and a `pty_output` whose bytes contain `1b` (a terminal query can make the worker write a reply into
    /// the session's input, EV-8). `pty_chunk` and `program_write_once` are program output, not writes.
    pub fn writes(step: &Value, routes: &Routes) -> BTreeSet<String> {
        let mut out = host_inputs(step, routes);
        if let Some((session, hex)) = pty_output(step) {
            if hex_bytes(hex).any(|b| b == "1b") {
                out.insert(session.to_string());
            }
        }
        out
    }

    /// The host input of a step (R-60): a `WriteInput` that names the session, and a `route_send` or `route_send_raw` on a route
    /// attached to it.
    pub fn host_inputs(step: &Value, routes: &Routes) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Some(session) = step
            .pointer("/begin/op/WriteInput/session")
            .and_then(Value::as_str)
        {
            out.insert(session.to_string());
        }
        if step.get("route_send").is_some() || step.get("route_send_raw").is_some() {
            if let Some(session) = routes.get(route_of(step)) {
                out.insert(session.clone());
            }
        }
        out
    }

    /// The literal bytes of an exact output assertion, with the route or session that it reads (R-61 (c)):
    /// `route_expect_output` (`bytes_hex` and the `bytes_hex` parts of `parts`), an `expect` or `expect_eventually` of an `output`
    /// or `output_deflate` frame, and a `check` of the bytes of a `tap_drain` (TP-1).
    pub fn exact_outputs(
        step: &Value,
        routes: &Routes,
        tap: &HashMap<String, String>,
    ) -> Vec<(Option<String>, String)> {
        let mut out = vec![];
        if let Some(spec) = step.get("route_expect_output") {
            let session = routes.get(route_of(step)).cloned();
            literals(spec, &mut out_for(&mut out, &session));
        }
        for key in ["expect", "expect_eventually"] {
            let Some(frame) = step.get(key) else { continue };
            if matches!(
                frame.get("frame").and_then(Value::as_str),
                Some("output" | "output_deflate")
            ) {
                let session = step
                    .get("on")
                    .and_then(Value::as_str)
                    .and_then(|route| routes.get(route))
                    .cloned();
                literals(frame, &mut out_for(&mut out, &session));
            }
        }
        if let Some(check) = step.get("check").and_then(Value::as_object) {
            for operands in check.values().filter_map(Value::as_array) {
                let session = operands
                    .iter()
                    .filter_map(Value::as_str)
                    .find_map(|var| tap.get(var))
                    .cloned();
                if session.is_some() {
                    for operand in operands {
                        literals(operand, &mut out_for(&mut out, &session));
                    }
                }
            }
        }
        out
    }

    /// The route that an exact output assertion of a step reads (R-61 (c)): a `route_expect_output`, or an `expect` or
    /// `expect_eventually` of an `output` or `output_deflate` frame on a route, with literal bytes. A `tap_drain` reads the PTY,
    /// not a route.
    pub fn exact_output_route(step: &Value) -> Option<&str> {
        // A variable reference (`"bytes_hex": "$bytes"`) is bound at run time, so it is no literal byte (#49 R1-1).
        let has_literal = |v: &Value| {
            let mut any = false;
            literals(v, &mut |hex| any |= !is_variable(hex));
            any
        };
        if let Some(spec) = step.get("route_expect_output") {
            return has_literal(spec).then(|| route_of(step));
        }
        ["expect", "expect_eventually"].iter().find_map(|key| {
            let frame = step.get(*key)?;
            let output = matches!(
                frame.get("frame").and_then(Value::as_str),
                Some("output" | "output_deflate")
            );
            (output && has_literal(frame))
                .then(|| step.get("on").and_then(Value::as_str))
                .flatten()
        })
    }

    /// The sessions whose output a step asserts (R-60), whether its bytes are literal or computed: a `route_expect_output` in any
    /// form (`bytes_hex`, or `parts` with `fill` or variables), an `expect` or `expect_eventually` of an `output` or
    /// `output_deflate` frame whose payload is more than a type or `$any` match, and a `check` of the bytes of a `tap_drain`.
    pub fn output_assertions(
        step: &Value,
        routes: &Routes,
        tap: &HashMap<String, String>,
    ) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if step.get("route_expect_output").is_some() {
            out.extend(routes.get(route_of(step)).cloned());
        }
        for key in ["expect", "expect_eventually"] {
            let Some(frame) = step.get(key) else { continue };
            if matches!(
                frame.get("frame").and_then(Value::as_str),
                Some("output" | "output_deflate")
            ) && frame.get("payload").is_some_and(constrains_bytes)
            {
                out.extend(
                    step.get("on")
                        .and_then(Value::as_str)
                        .and_then(|route| routes.get(route))
                        .cloned(),
                );
            }
        }
        if let Some(check) = step.get("check").and_then(Value::as_object) {
            for operands in check.values().filter_map(Value::as_array) {
                out.extend(
                    operands
                        .iter()
                        .filter_map(Value::as_str)
                        .find_map(|var| tap.get(var))
                        .cloned(),
                );
            }
        }
        out
    }

    /// Whether a payload matcher says anything about the bytes: anything but `{"$any": ..}` or `{"$type": ..}`.
    fn constrains_bytes(payload: &Value) -> bool {
        !payload
            .as_object()
            .is_some_and(|m| m.len() == 1 && (m.contains_key("$any") || m.contains_key("$type")))
    }

    /// The literal bytes of an exact input assertion, with the session that it reads (R-61 (c)): the `pty_input` and
    /// `route_expect_pty` literals.
    pub fn exact_inputs(step: &Value, routes: &Routes) -> Vec<(Option<String>, String)> {
        let mut out = vec![];
        if let Some(session) = pty_input(step) {
            if let Some(expect) = step.get("expect") {
                literals(expect, &mut out_for(&mut out, &Some(session.to_string())));
            }
        }
        if let Some(spec) = step.get("route_expect_pty") {
            let session = routes.get(route_of(step)).cloned();
            literals(spec, &mut out_for(&mut out, &session));
        }
        out
    }

    /// The op and the bytes of a sent `bytes`, `text` or `paste_chunk` frame with an op that is not 0, typed or raw: what
    /// `route_expect_pty`'s `written_by_op` asserts exactly (R-73 item 5, R-61 (c)).
    pub fn sent_input(step: &Value) -> Option<(u64, Vec<u8>)> {
        sent_body(step).filter(|(op, _)| *op != 0)
    }

    /// The op (0 for none) and the bytes of a sent `bytes`, `text` or `paste_chunk` frame, typed or raw.
    pub fn sent_body(step: &Value) -> Option<(u64, Vec<u8>)> {
        use botster_route_codec::prelude::*;
        match sent_frame(step)? {
            ToWorker::Bytes { op, bytes } | ToWorker::PasteChunk { op, bytes } => {
                Some((op.0, bytes.to_vec()))
            }
            ToWorker::Text { op, text } => Some((op.0, text.into_bytes())),
            _ => None,
        }
    }

    /// The frame that a `route_send` or a `route_send_raw` sends, when it decodes.
    pub fn sent_frame(step: &Value) -> Option<botster_route_codec::prelude::ToWorker> {
        use botster_route_codec::prelude::*;
        let frame = if let Some(typed) = step.get("route_send") {
            serde_json::from_value::<ToWorker>(typed.clone()).ok()?
        } else {
            let raw = step.pointer("/route_send_raw/bytes_hex")?.as_str()?;
            let bounds = FrameBounds {
                max_frame: u64::MAX,
                max_screen: u64::MAX,
                max_history: u64::MAX,
            };
            match ToWorker::decode(&hex_decode(raw).ok()?, &bounds).ok()? {
                Decoded::Frame(frame) => frame,
                _ => return None,
            }
        };
        Some(frame)
    }

    /// The ops that a `route_expect_pty` names by `written_by_op`.
    pub fn ops_by_relation(step: &Value) -> Vec<u64> {
        fn walk(v: &Value, out: &mut Vec<u64>) {
            match v {
                Value::Object(m) => {
                    if let Some(op) = m.get("written_by_op").and_then(Value::as_str) {
                        out.extend(op.parse::<u64>().ok());
                    }
                    m.values().for_each(|x| walk(x, out));
                }
                Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                _ => {}
            }
        }
        let mut out = vec![];
        if let Some(spec) = step.get("route_expect_pty") {
            walk(spec, &mut out);
        }
        out
    }

    /// The fewest bytes that a `route_expect_pty` accepts: over its sequences (one, or each of `one_of`), the least sum of its
    /// literal parts' bytes and of its relation parts' floors. A `var` part counts no bytes (its value is a run-time string). A
    /// `written_by_op` part of an op with known bytes counts the shortest of that op's known runs (`known`: raw `bytes`, `text`,
    /// a committed paste's body; R-73 follow-up 2 (2)); any other relation part counts 1, because a run is not empty (R-83).
    /// `known` holds the runs of the read's session. None when the step is no route read, or when it accepts an empty sequence
    /// (an absence check, which needs no fence).
    pub fn route_pty_least(step: &Value, known: &HashMap<u64, Vec<u64>>) -> Option<u64> {
        let spec = step.get("route_expect_pty")?;
        let seqs: Vec<&Value> = match spec.get("one_of").and_then(Value::as_array) {
            Some(many) => many.iter().collect(),
            None => vec![spec],
        };
        if seqs
            .iter()
            .any(|seq| seq.as_array().is_some_and(Vec::is_empty))
        {
            return None;
        }
        seqs.iter()
            .map(|seq| {
                seq.as_array().map_or(0, |parts| {
                    parts
                        .iter()
                        .map(|part| match part.get("hex").and_then(Value::as_str) {
                            Some(hex) => (hex.len() / 2) as u64,
                            None if part.get("var").is_some() => 0,
                            None => part
                                .get("written_by_op")
                                .and_then(Value::as_str)
                                .and_then(|op| op.parse::<u64>().ok())
                                .and_then(|op| known.get(&op))
                                .and_then(|runs| runs.iter().min().copied())
                                .unwrap_or(1)
                                .max(1),
                        })
                        .sum()
                })
            })
            .min()
    }

    /// Whether a `route_expect_pty` has a `written_without_op` part.
    pub fn asserts_without_op(step: &Value) -> bool {
        fn walk(v: &Value) -> bool {
            match v {
                Value::Object(m) => m.contains_key("written_without_op") || m.values().any(walk),
                Value::Array(a) => a.iter().any(walk),
                _ => false,
            }
        }
        step.get("route_expect_pty").is_some_and(walk)
    }

    /// Whether a step reads a session's input log (R-60): a `pty_input` of the session, or a `route_expect_pty` on a route attached
    /// to it.
    pub fn reads_input_log(step: &Value, routes: &Routes) -> Option<String> {
        if let Some(session) = pty_input(step) {
            return Some(session.to_string());
        }
        step.get("route_expect_pty")
            .and_then(|_| routes.get(route_of(step)))
            .cloned()
    }

    fn out_for<'a>(
        out: &'a mut Vec<(Option<String>, String)>,
        session: &'a Option<String>,
    ) -> impl FnMut(&str) + 'a {
        move |hex| out.push((session.clone(), hex.to_string()))
    }

    /// The outputs that a read reaches: those of a Core handle (`None`: any handle), of a session (`None`: every session).
    /// An event read takes the queue of its handle; a sync read or a control reads one session of its handle; a route read
    /// reads the session of its route, on the handle that attached the route.
    pub struct Scope {
        pub handle: Option<String>,
        pub session: Option<String>,
    }

    impl Scope {
        /// Whether the read reaches the output of `session` on `handle`.
        pub fn reaches(&self, handle: &str, session: &str) -> bool {
            self.handle.as_deref().is_none_or(|h| h == handle)
                && self.session.as_deref().is_none_or(|s| s == session)
        }
    }

    /// The handle that a step names, as the driver reads it: `handle` in `spec`, else `h`.
    pub fn handle_of(spec: &Value) -> String {
        spec.get("handle")
            .and_then(Value::as_str)
            .unwrap_or("h")
            .to_string()
    }

    /// A step that reads a session's events or state, the sessions it reads, and whether it needs an `await_title` fence after
    /// the session's last `pty_output` (R-61 (a), with the steward's answers for PR C):
    /// - **A wait for a new value** needs no fence: `pump_until`, `pump_collect`, `expect_eventually`, `route_expect_output`,
    ///   `await_get`, `await_control`, a `tap_drain` with `accounted`, and `expect`, `expect_set` or `expect_closed` on a route
    ///   (a route read blocks until the next frames arrive). A wait whose matcher names `EventsLost` needs the fence: the
    ///   output extends the marker's `kinds`, so an earlier state can meet it (shape 2).
    /// - **A read that does not wait** needs the fence (shape 1): `drain`, `forbid_events`, `poll_events`, `collect_events`,
    ///   `poll_all`, `poll_find`, `expect_quiet_until` (an absence check), `expect` or `expect_set` on the events, a `pump` with
    ///   `expect`, a `tap_drain` without `accounted`, a sync `call` of the session (`list`, `status` and `diagnostics` read every
    ///   session), and a control of the session whose result the step checks or binds (`oracle_state` and `oracle_screen` read
    ///   the model that the output feeds).
    /// - **Not a read:** a setter, which observes nothing even when its result is checked (`pty_accept`, and `control_queue
    ///   {on}` with no `expect`; the steward: a control is a read only when its result is checked or bound), the program output
    ///   controls, and the input side (`pty_input`, `await_pty`, `route_expect_pty`): R-56 orders those, with R-61 (b).
    ///   `await_quiet`, `pump_idle`, clock, `let`, `check`, `begin` and route sends read nothing. `await_title` is the fence.
    pub fn output_read(
        step: &Value,
        routes: &Routes,
        route_handles: &HashMap<String, String>,
    ) -> Option<(Scope, bool)> {
        const MODIFIERS: [&str; 10] = [
            "because",
            "ignore",
            "bind",
            "bind_fields",
            "on",
            "until",
            "ack",
            "allow_invalid",
            "expect_error",
            "expect_refused",
        ];
        let obj = step.as_object()?;
        let kind = obj
            .keys()
            .map(String::as_str)
            .find(|k| !MODIFIERS.contains(k) && *k != "expect")
            .or_else(|| obj.contains_key("expect").then_some("expect"))?;
        let spec = &step[kind];
        // The queue of the step's handle (an order step names it in `on`), every session.
        let queue = |on: &Value| {
            Some(Scope {
                handle: Some(handle_of(on)),
                session: None,
            })
        };
        // One session of the step's handle; none when the session is not named.
        let one = |on: &Value, session: Option<&Value>| {
            session.and_then(Value::as_str).map(|s| Scope {
                handle: Some(handle_of(on)),
                session: Some(s.to_string()),
            })
        };
        // A route scope reads the session that the route is attached to, on the handle that attached it (#42 R2-1); a route
        // that is not attached reads none.
        let route_session = |r: &str| {
            routes.get(r).map(|s| Scope {
                handle: route_handles.get(r).cloned(),
                session: Some(s.clone()),
            })
        };
        let on = step.get("on");
        let route = on.and_then(Value::as_str).map(route_session);
        let events_on = on.filter(|v| v.is_object()).unwrap_or(&Value::Null);
        // Shape 2 looks only at the matchers that can end the wait: an `EventsLost` in an `ignore` or `forbid` matcher
        // never ends it (#42 R1-1).
        let mut target = spec.clone();
        if let Some(m) = target.as_object_mut() {
            m.remove("ignore");
            m.remove("forbid");
        }
        let names_events_lost = mentions(&target, "EventsLost");
        let wait = |scope: Option<Scope>| scope.map(|s| (s, names_events_lost));
        let no_wait = |scope: Option<Scope>| scope.map(|s| (s, true));
        match kind {
            "pump_until" | "pump_collect" => wait(queue(spec)),
            "expect_eventually" => wait(route.unwrap_or_else(|| queue(events_on))),
            "expect" | "expect_set" | "expect_closed" => match route {
                Some(scope) => wait(scope),
                None => no_wait(queue(events_on)),
            },
            "route_expect_output" => wait(route_session(route_of(step))),
            "await_get" => wait(one(spec, spec.get("id"))),
            "await_control" => {
                wait(one(spec, spec.pointer("/control/session")).or_else(|| queue(spec)))
            }
            "tap_drain" if spec.get("accounted").is_some() => wait(one(spec, spec.get("session"))),
            "tap_drain" => no_wait(one(spec, spec.get("session"))),
            "drain" | "forbid_events" | "poll_events" | "collect_events" | "poll_all"
            | "poll_find" => no_wait(queue(spec)),
            "expect_quiet_until" => no_wait(route.unwrap_or_else(|| queue(events_on))),
            "pump" if obj.contains_key("expect") => no_wait(queue(spec)),
            "call" => match spec.get("method").and_then(Value::as_str)? {
                "list" | "status" | "diagnostics" => no_wait(queue(spec)),
                // The driver's session argument: `id`, else `session` (#42 R1-3).
                "get" | "terminal_state" | "read_page" | "snapshot_formats"
                | "terminal_identity" => {
                    let args = spec.get("args");
                    let session = args
                        .and_then(|a| a.get("id"))
                        .or(args.and_then(|a| a.get("session")));
                    no_wait(one(spec, session))
                }
                _ => None,
            },
            "control" => {
                let op = spec.get("op").and_then(Value::as_str)?;
                let observes = ["expect", "bind", "bind_fields", "expect_refused"]
                    .iter()
                    .any(|k| obj.contains_key(*k));
                // A setter observes nothing, even when its result is checked (#42 R1-4).
                let not_output = [
                    "pty_output",
                    "pty_chunk",
                    "program_write_once",
                    "pty_input",
                    "pty_accept",
                ];
                if observes && !not_output.contains(&op) {
                    no_wait(one(spec, spec.get("session")))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Whether a value has an object key `key` at any depth.
    fn mentions(v: &Value, key: &str) -> bool {
        match v {
            Value::Object(m) => m.contains_key(key) || m.values().any(|x| mentions(x, key)),
            Value::Array(a) => a.iter().any(|x| mentions(x, key)),
            _ => false,
        }
    }

    /// Every literal hex string in a value: the values of `$bytes_hex`, `bytes_hex` and `hex`.
    fn literals(v: &Value, push: &mut impl FnMut(&str)) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    match (k.as_str(), x) {
                        ("$bytes_hex" | "bytes_hex" | "hex", Value::String(hex)) => push(hex),
                        _ => literals(x, push),
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| literals(x, push)),
            _ => {}
        }
    }
}

/// The sessions of the routes attached so far: `attach_route` maps its route to its session.
type Routes = HashMap<String, String>;

/// The units that the runner executes, in order, each with its top-level step index: a step's `barrier` runs first
/// (`expect_quiet_until` runs it, then waits), then the step without it. Every rule applies to each unit.
fn units(steps: &[Value]) -> Vec<(usize, Value)> {
    fn push(i: usize, step: &Value, out: &mut Vec<(usize, Value)>) {
        if let Some(barrier) = step.get("barrier") {
            push(i, barrier, out);
        }
        let mut own = step.clone();
        if let Some(m) = own.as_object_mut() {
            m.remove("barrier");
        }
        out.push((i, own));
    }
    let mut out = vec![];
    for (i, step) in steps.iter().enumerate() {
        push(i, step, &mut out);
    }
    out
}

/// A copy of a value with every hex literal (`$bytes_hex`, `bytes_hex`, `hex`) in lower case: the matcher compares hex
/// digits without case, so the byte rules do too.
fn lowercase_hex(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| {
                    let x = match (k.as_str(), x) {
                        ("$bytes_hex" | "bytes_hex" | "hex", Value::String(hex)) => {
                            Value::String(hex.to_ascii_lowercase())
                        }
                        _ => lowercase_hex(x),
                    };
                    (k.clone(), x)
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(lowercase_hex).collect()),
        _ => v.clone(),
    }
}

/// The route of a route step; `r1` when it names none, as in the driver.
fn route_of(step: &Value) -> &str {
    step.get("route").and_then(Value::as_str).unwrap_or("r1")
}

fn control<'a>(step: &'a Value, op: &str) -> Option<&'a Value> {
    step.get("control")
        .filter(|c| c.get("op").and_then(Value::as_str) == Some(op))
}

fn pty_input(step: &Value) -> Option<&str> {
    control(step, "pty_input")?.get("session")?.as_str()
}

fn pty_output(step: &Value) -> Option<(&str, &str)> {
    let c = control(step, "pty_output")?;
    Some((c.get("session")?.as_str()?, c.get("bytes_hex")?.as_str()?))
}

/// The prints of a session's own program, followed through the transcript. A program can print an R-57 title sentinel
/// (steward ruling R-88 (E), `a5_2` s79), but only after all the output that the fence covers, in the same output stream
/// (the steward on #75): so only the end of the program's last print can be the sentinel. Its step is the step at which that
/// print runs:
/// - a `print` item runs when the items before it have run and the session has started;
/// - a `print_after_input` item runs at the host input after which the input that the probe read since the last match
///   contains its `match_hex` (the probe matches its reader's log).
#[derive(Default)]
struct Program {
    /// The `match_hex` (`None` for `print`) and the `bytes_hex` of each print item, in order.
    prints: Vec<(Option<String>, String)>,
    /// The hex of the input that the probe read since the last match.
    log: String,
    /// The step at which each print ran, for the prints that ran, in order.
    ran: Vec<usize>,
    started: bool,
    /// An input whose bytes the lint does not know (a key, a mouse event, a raw frame) came before the last print ran: the
    /// lint cannot place the prints that did not run yet.
    unknown: bool,
}

impl Program {
    /// The session that a `Create` names, with its program.
    fn of(step: &Value) -> Option<(&str, Program)> {
        let create = step.pointer("/begin/op/Create")?;
        let session = create.get("session")?.as_str()?;
        let items = create.pointer("/request/program")?.as_array()?;
        let prints = items
            .iter()
            .filter_map(|item| {
                if let Some(p) = item.get("print") {
                    Some((None, p.get("bytes_hex")?.as_str()?.to_string()))
                } else {
                    let p = item.get("print_after_input")?;
                    let m = p.get("match_hex")?.as_str()?.to_string();
                    Some((Some(m), p.get("bytes_hex")?.as_str()?.to_string()))
                }
            })
            .collect();
        Some((
            session,
            Program {
                prints,
                ..Program::default()
            },
        ))
    }

    /// Runs the prints that can run at step `i`.
    fn run(&mut self, i: usize) {
        while self.started && !self.unknown {
            let Some((m, _)) = self.prints.get(self.ran.len()) else {
                return;
            };
            if let Some(m) = m {
                let Some(at) = self
                    .log
                    .match_indices(m.as_str())
                    .map(|(at, _)| at)
                    .find(|at| at % 2 == 0)
                else {
                    return;
                };
                self.log.drain(..at + m.len());
            }
            self.ran.push(i);
        }
    }

    fn start(&mut self, i: usize) {
        self.started = true;
        self.run(i);
    }

    /// A host input at step `i`, with its bytes when the lint knows them.
    fn input(&mut self, hex: Option<&str>, i: usize) {
        if self.ran.len() == self.prints.len() {
            return;
        }
        match hex {
            Some(hex) => self.log.push_str(hex),
            None => self.unknown = true,
        }
        self.run(i);
    }

    /// The step of the sentinel of `title`: the step at which the program's last print ran, if the sequence of that title
    /// ends it. Another title after it does not do: the worker can show the title before it applies the bytes after it.
    fn sentinel(&self, title: &str) -> Option<usize> {
        let (_, last) = self.prints.last()?;
        if self.ran.len() == self.prints.len() && ends_with_title(last, title) {
            self.ran.last().copied()
        } else {
            None
        }
    }
}

/// The bytes of a host input when the transcript gives them as hex: a `WriteInput` or a `route_send` of a `bytes` payload.
fn input_hex(step: &Value) -> Option<&str> {
    step.pointer("/begin/op/WriteInput/payload/bytes/bytes/$bytes_hex")
        .or_else(|| {
            step.get("route_send")
                .filter(|s| s.get("frame").and_then(Value::as_str) == Some("bytes"))
                .and_then(|s| s.pointer("/bytes/$bytes_hex"))
        })
        .and_then(Value::as_str)
}

/// The bytes of a hex string, two digits each.
fn hex_bytes(hex: &str) -> impl Iterator<Item = &str> {
    (0..hex.len() / 2).map(move |i| &hex[2 * i..2 * i + 2])
}

/// The hex of the OSC 2 title that `await_title` waits for: `ESC ] 2 ;`, the title, then `ST` or `BEL`.
fn sentinel_hex(title: &str) -> String {
    let mut hex = String::from("1b5d323b");
    for b in title.bytes() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// Whether `hex` sets the window title to exactly `title`: `ESC ] 0 ;` or `ESC ] 2 ;` (both set the title), the title, then
/// `ST` or `BEL`. The match is on whole bytes, so neither a longer title nor a byte split across two hex digits matches.
fn sets_title(hex: &str, title: &str) -> bool {
    let body: String = title.bytes().map(|b| format!("{b:02x}")).collect();
    ["1b5d303b", "1b5d323b"].iter().any(|osc| {
        ["07", "1b5c"].iter().any(|end| {
            let sentinel = format!("{osc}{body}{end}");
            hex.match_indices(&sentinel).any(|(at, _)| at % 2 == 0)
        })
    })
}

/// Whether the last bytes of `hex` are the sequence that sets the title to exactly `title` (see `sets_title`).
fn ends_with_title(hex: &str, title: &str) -> bool {
    let body: String = title.bytes().map(|b| format!("{b:02x}")).collect();
    hex.len().is_multiple_of(2)
        && ["1b5d303b", "1b5d323b"].iter().any(|osc| {
            ["07", "1b5c"]
                .iter()
                .any(|end| hex.ends_with(&format!("{osc}{body}{end}")))
        })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    id: String,
    step: usize,
    rule: &'static str,
    what: String,
}

/// The `min_bytes` of an `await_pty` (R-56 (3)(1), the steward's settlement of #39 R1-2): a number in the transcript (1 when
/// absent, as in the driver); a variable that an earlier `let` bound, with its class; or anything else, which fences nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fence {
    Literal(u64),
    Bound(Num),
    Invalid,
}

/// The class of a `let` value. `Fixed`: the transcript alone fixes it (signed, as the driver computes it). `Run`: a numeric
/// operation on a value bound at run time, which cannot be negative (a length, or a sum of lengths and non-negative fixed
/// values). `Other`: anything else: a non-number (`eq`, `gt`, `contains`, `str`, ...), an operation whose sign the run decides
/// (`sub` of a run-time value), or an operation the lint does not model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Num {
    Fixed(i128),
    Run,
    Other,
}

impl Fence {
    /// Whether the fence covers a log of `len` bytes: a known value at least `len`. For a capture (`len` unknown), the steward's
    /// classes 1 and 2: a bound value fixed at 1 or more, or a run-time numeric value from a bound `let`. A number written in
    /// the transcript is no fence for a capture.
    fn covers(self, len: Option<u64>) -> bool {
        match (self, len) {
            (Fence::Literal(n), Some(len)) => n >= len,
            (Fence::Bound(Num::Fixed(n)), Some(len)) => n >= i128::from(len),
            (Fence::Bound(Num::Fixed(n)), None) => n >= 1,
            (Fence::Bound(Num::Run), None) => true,
            _ => false,
        }
    }
}

/// The string that the driver's `hex_of` reads: the `$bytes_hex` string of an object, or the value itself if it is a string. It
/// can be a variable reference, which the driver substitutes first. Other fields of an object are not read.
fn hex_source(v: &Value) -> Option<&str> {
    v.get("$bytes_hex")
        .and_then(Value::as_str)
        .or_else(|| v.as_str())
}

/// A variable's name as the driver keys it: without its leading `$`.
fn var_name(var: &str) -> String {
    var.trim_start_matches('$').to_string()
}

/// The variables that a step binds by a form other than `let`.
enum Rebound {
    /// `bind`, the values of `bind_fields` (keyed without their `$`), and an `attach_route`'s literal route name (verbatim; `r1`
    /// when `route` is absent or not a string, as the driver's `attach` reads it).
    Names(Vec<String>),
    /// An `attach_route` whose route is a variable reference: the driver binds the route's id under the variable's run-time
    /// value, which the lint does not know, so every variable loses its class.
    All,
}

fn rebound(step: &Value) -> Rebound {
    let mut out: Vec<String> = step
        .get("bind")
        .and_then(Value::as_str)
        .map(var_name)
        .into_iter()
        .collect();
    if let Some(fields) = step.get("bind_fields").and_then(Value::as_object) {
        out.extend(fields.values().filter_map(Value::as_str).map(var_name));
    }
    if let Some(attach) = step.get("attach_route") {
        match attach.get("route").and_then(Value::as_str).unwrap_or("r1") {
            route if is_variable(route) => return Rebound::All,
            route => out.push(route.to_string()),
        }
    }
    Rebound::Names(out)
}

/// A `WriteInput` that a `begin` bound: its session, and whether its payload is raw `bytes` (written with no encoding, so its
/// payload count and its PTY count are equal).
struct Write {
    session: String,
    raw: bool,
}

/// The `WriteInput` that a `begin` step binds, under its variable's name.
fn write_input(step: &Value) -> Option<(String, Write)> {
    let op = step.pointer("/begin/op/WriteInput")?;
    let var = step.get("bind").and_then(Value::as_str)?;
    let session = op.get("session").and_then(Value::as_str)?;
    let raw = op
        .get("payload")
        .and_then(Value::as_object)
        .is_some_and(|p| p.len() == 1 && p.contains_key("bytes"));
    Some((
        var_name(var),
        Write {
            session: session.to_string(),
            raw,
        },
    ))
}

/// The written counts that a `pump_until` or `pump_collect` matcher binds, each with the session of its write (steward ruling R-74,
/// the lint case). A count is a variable at `pty_bytes_written`, or at `payload_bytes_written` when the write's payload is raw
/// `bytes`, in the `Completed` of a `WriteInput` that an earlier `begin` bound. It is a `u64`, never negative, so an `await_pty` of
/// the same session for it fences a capture: the held write reads its log after `await_pty` for the kernel's count `$p`.
fn written_counts(step: &Value, writes: &HashMap<String, Write>) -> Vec<(String, String)> {
    fn counts(v: &Value, raw: bool, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, v) in map {
                    match v.as_str() {
                        Some(var)
                            if is_variable(var)
                                && (k == "pty_bytes_written"
                                    || (raw && k == "payload_bytes_written")) =>
                        {
                            out.push(var_name(var));
                        }
                        _ => counts(v, raw, out),
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|v| counts(v, raw, out)),
            _ => {}
        }
    }
    fn completions<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
        match v {
            Value::Object(map) => {
                if let Some(done) = map.get("Completed") {
                    out.push(done);
                }
                map.values().for_each(|v| completions(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| completions(v, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    for pump in ["pump_until", "pump_collect"] {
        let mut done = vec![];
        if let Some(matcher) = step.get(pump) {
            completions(matcher, &mut done);
        }
        for done in done {
            let Some(write) = done
                .get("op")
                .and_then(Value::as_str)
                .filter(|op| is_variable(op))
                .and_then(|op| writes.get(&var_name(op)))
            else {
                continue;
            };
            let mut vars = vec![];
            if let Some(result) = done.get("result") {
                counts(result, write.raw, &mut vars);
            }
            out.extend(vars.into_iter().map(|v| (v, write.session.clone())));
        }
    }
    out
}

/// R-83 (B): the `written_bytes` that a route matcher binds from an `input_done` or `input_refused` (`expect`, `expect_eventually`,
/// `expect_set`, on a route attached to a session), with that session. The clauses define it as the bytes that the op wrote to the
/// PTY: DP-5 for a paste, a `bytes`, a `text` or a key, and DP-5b for a file op (the encoded path paste, never the file's content).
/// So it is a written count of that session, the route form of R-74's matcher-bound count.
fn route_written_counts(step: &Value, routes: &Routes) -> Vec<(String, String)> {
    fn counts(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                if matches!(
                    map.get("frame").and_then(Value::as_str),
                    Some("input_done" | "input_refused")
                ) {
                    if let Some(var) = map
                        .get("written_bytes")
                        .and_then(Value::as_str)
                        .filter(|v| is_variable(v))
                    {
                        out.push(var_name(var));
                    }
                }
                map.values().for_each(|v| counts(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| counts(v, out)),
            _ => {}
        }
    }
    let Some(session) = step
        .get("on")
        .and_then(Value::as_str)
        .and_then(|r| routes.get(r))
    else {
        return vec![];
    };
    let mut vars = vec![];
    for form in ["expect", "expect_eventually", "expect_set"] {
        if let Some(matcher) = step.get(form) {
            counts(matcher, &mut vars);
        }
    }
    vars.into_iter().map(|v| (v, session.clone())).collect()
}

/// What the lint knows of each `let` variable's current value. A binding by any other form forgets the variable.
#[derive(Default)]
struct Lets {
    /// The numeric class of each `let` value.
    class: HashMap<String, Num>,
    /// The session of each value that is a written count of that session's write (R-74), or a `let` sum built from one.
    written: HashMap<String, String>,
    /// The `let` values that the transcript fixes: a computed `Fixed` value, or a value built only from literals and such values.
    fixed: BTreeSet<String>,
    /// The `let` values that the driver's `hex_of` never reads: a number, a boolean or a list, so an item of `sum_hex_bytes` that names
    /// one gives 0 (#41 R2-1).
    unread: BTreeSet<String>,
}

/// The `let` operations whose value is a number, a boolean or a list: never a string or an object, so `hex_of` reads nothing.
const UNREAD_OPS: [&str; 11] = [
    "record_payloads",
    "add",
    "sub",
    "gt",
    "lt",
    "eq",
    "contains",
    "hex_len",
    "str_len",
    "utf8_len",
    "sum_hex_bytes",
];

impl Lets {
    /// A `let` binds `var` to `expr`, computed from the earlier bindings; the new facts replace the old ones (#41 R1-2).
    fn bind(&mut self, var: &str, expr: &Value) {
        let name = var_name(var);
        let value = fold(expr, self);
        // A computed `Fixed` value is fixed, whatever field the driver ignored to compute it (#41 R2-1).
        let fixed = matches!(value, Num::Fixed(_)) || self.fixed(expr);
        set(&mut self.fixed, &name, fixed);
        let op = expr.as_object().and_then(|m| m.keys().next());
        set(
            &mut self.unread,
            &name,
            op.is_some_and(|op| UNREAD_OPS.contains(&op.as_str())),
        );
        // A value computed from written counts is a count of their session; written counts of two sessions fence neither.
        let mut sessions: BTreeSet<String> = BTreeSet::new();
        self.sessions_in(expr, &mut sessions);
        let mut sessions = sessions.into_iter();
        let value = match (sessions.next(), sessions.next()) {
            (Some(session), None) => {
                self.written.insert(name.clone(), session);
                value
            }
            (None, _) => {
                self.written.remove(&name);
                value
            }
            (Some(_), Some(_)) => {
                self.written.remove(&name);
                Num::Other
            }
        };
        self.class.insert(name, value);
    }

    /// The sessions of the written counts that a value names.
    fn sessions_in(&self, v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::String(var) if is_variable(var) => {
                out.extend(self.written.get(&var_name(var)).cloned());
            }
            Value::Object(m) => m.values().for_each(|x| self.sessions_in(x, out)),
            Value::Array(a) => a.iter().for_each(|x| self.sessions_in(x, out)),
            _ => {}
        }
    }

    /// A matcher binds `var` to a written count of `session`'s write (R-74). A variable that the lint already knows is not bound:
    /// the matcher compares it, and it keeps its class.
    fn written_count(&mut self, var: String, session: String) {
        if !self.class.contains_key(&var) {
            self.class.insert(var.clone(), Num::Run);
            self.written.insert(var, session);
        }
    }

    /// Another binding form replaces `var`: the lint knows nothing of its value.
    fn forget(&mut self, var: &str) {
        self.written.remove(var);
        self.class.remove(var);
        self.fixed.remove(var);
        self.unread.remove(var);
    }

    /// Whether a value depends on nothing bound at run time: every variable in it is a `let` variable that the transcript fixes.
    fn fixed(&self, v: &Value) -> bool {
        match v {
            Value::String(var) if is_variable(var) => self.fixed.contains(&var_name(var)),
            Value::Object(m) => m.values().all(|x| self.fixed(x)),
            Value::Array(a) => a.iter().all(|x| self.fixed(x)),
            _ => true,
        }
    }
}

fn set(names: &mut BTreeSet<String>, name: &str, member: bool) {
    if member {
        names.insert(name.to_string());
    } else {
        names.remove(name);
    }
}

/// A `Fixed` value that the driver can bind: an i64 or a u64 (`let_step` refuses any other `add` or `sub` result).
fn in_range(n: i128) -> Num {
    if i64::try_from(n).is_ok() || u64::try_from(n).is_ok() {
        Num::Fixed(n)
    } else {
        Num::Other
    }
}

/// The class of a `let` expression (one operation), from the classes of earlier `let` variables. A length of a value that the
/// transcript fixes but the lint does not compute (a `let` variable built from literals) is `Other`: unsupported, never a fence.
fn fold(expr: &Value, lets: &Lets) -> Num {
    let Some((op, args)) = expr
        .as_object()
        .filter(|m| m.len() == 1)
        .and_then(|m| m.iter().next())
    else {
        return Num::Other;
    };
    // The driver's arity: one argument for these operations, two for every other.
    let arity = if [
        "rtrim_spaces",
        "record_payloads",
        "sum_hex_bytes",
        "utf8_len",
    ]
    .contains(&op.as_str())
    {
        1
    } else {
        2
    };
    let Some(args) = args.as_array().filter(|a| a.len() == arity) else {
        return Num::Other;
    };
    // An operand as the driver's `number()` reads it (an i64 or a u64), or the class of a referenced `let` variable.
    let operand = |v: &Value| match v {
        Value::Number(n) => n
            .as_i64()
            .map(i128::from)
            .or_else(|| n.as_u64().map(i128::from))
            .map_or(Num::Other, Num::Fixed),
        Value::String(var) if is_variable(var) => lets
            .class
            .get(&var_name(var))
            .copied()
            .unwrap_or(Num::Other),
        _ => Num::Other,
    };
    let first = args.first();
    // The class of a length whose value the run fixes; `Other` when the transcript fixes it and the lint cannot compute it.
    let run_length = |a: &Value| {
        if lets.fixed(a) {
            Num::Other
        } else {
            Num::Run
        }
    };
    // The length of a hex value as `hex_of` reads it: a literal is fixed, a variable reference has its value's class, and only the
    // string that `hex_of` reads counts (#41 R1-1). `None`: `hex_of` reads nothing, also from a `let` number or boolean (R2-1).
    let hex_length = |a: &Value| {
        hex_source(a)
            .filter(|h| !(is_variable(h) && lets.unread.contains(&var_name(h))))
            .map(|h| {
                if is_variable(h) {
                    run_length(&json!(h))
                } else {
                    Num::Fixed((h.len() / 2) as i128)
                }
            })
    };
    match op.as_str() {
        // A length: fixed for a literal, else a run-time value that is never negative.
        // The driver refuses a value that `hex_of` does not read.
        "hex_len" => first.and_then(hex_length).unwrap_or(Num::Other),
        "str_len" | "utf8_len" => first.map_or(Num::Other, |a| match a.as_str() {
            Some(text) if !is_variable(text) => Num::Fixed(text.len() as i128),
            Some(_) => run_length(a),
            // The driver takes a string only.
            None => Num::Other,
        }),
        // Each item that `hex_of` reads gives its length, and every other item 0 (`hex_of(..).map_or(0, ..)`): a sum of fixed
        // lengths is fixed, a sum with a run-time length is a run-time value, and any length the lint does not compute is `Other`.
        "sum_hex_bytes" => first.map_or(Num::Other, |a| match a.as_array() {
            Some(items) => items
                .iter()
                .map(|h| hex_length(h).unwrap_or(Num::Fixed(0)))
                .fold(Num::Fixed(0), |sum, item| match (sum, item) {
                    (Num::Other, _) | (_, Num::Other) => Num::Other,
                    (Num::Fixed(a), Num::Fixed(b)) => Num::Fixed(a + b),
                    _ => Num::Run,
                }),
            None if is_variable(a.as_str().unwrap_or_default()) => run_length(a),
            None => Num::Other,
        }),
        "add" | "sub" if args.len() == 2 => {
            match (operand(&args[0]), operand(&args[1]), op.as_str()) {
                (Num::Fixed(a), Num::Fixed(b), "add") => in_range(a + b),
                (Num::Fixed(a), Num::Fixed(b), _) => in_range(a - b),
                // A sum of values that are never negative; a difference with a run-time operand has the run's sign.
                (a, b, "add")
                    if [a, b]
                        .iter()
                        .all(|x| matches!(x, Num::Run | Num::Fixed(0..))) =>
                {
                    Num::Run
                }
                _ => Num::Other,
            }
        }
        _ => Num::Other,
    }
}

/// The rule tags of the exception list.
const UNFENCED_CAPTURE: &str = "unfenced capture, R-56";
const UNFENCED_ROUTE_READ: &str = "unfenced route read, R-56";

/// The lengths of each op's runs whose bytes the transcript states (R-73 follow-up 2 (2), R-83): a `bytes` or `text` frame with an
/// op, and the body of a committed paste. An aborted paste wrote nothing. A run is known for the session of the route that sent
/// it, because a route read reads its own session's log. A paste is open on its route, so `open` is keyed by route and op, as
/// the driver keys it. In `done`, the ops of one session are keyed without their route, so a lint count stays a floor: the
/// shortest run of the op on that session.
#[derive(Default)]
struct KnownRuns {
    done: HashMap<String, HashMap<u64, Vec<u64>>>,
    open: HashMap<(String, u64), (String, u64)>,
}

impl KnownRuns {
    fn sent(&mut self, route: &str, session: &str, frame: botster_route_codec::prelude::ToWorker) {
        use botster_route_codec::prelude::*;
        let key = |op: Op| (route.to_string(), op.0);
        match frame {
            ToWorker::Bytes { op, bytes } if op.0 != 0 => {
                self.done(session, op.0, bytes.len() as u64)
            }
            ToWorker::Text { op, text } if op.0 != 0 => self.done(session, op.0, text.len() as u64),
            ToWorker::PasteChunk { op, bytes } => {
                if let Some((_, body)) = self.open.get_mut(&key(op)) {
                    *body += bytes.len() as u64;
                }
            }
            ToWorker::Input(Input::PasteBegin { op, .. }) => {
                self.open.insert(key(op), (session.to_string(), 0));
            }
            ToWorker::Input(Input::PasteCommit { op, .. }) => {
                if let Some((session, body)) = self.open.remove(&key(op)) {
                    self.done(&session, op.0, body);
                }
            }
            ToWorker::Input(Input::PasteAbort { op }) => {
                self.open.remove(&key(op));
            }
            _ => {}
        }
    }

    fn done(&mut self, session: &str, op: u64, len: u64) {
        self.done
            .entry(session.to_string())
            .or_default()
            .entry(op)
            .or_default()
            .push(len);
    }

    /// The known runs of the ops of `session`.
    fn of(&self, session: &str) -> HashMap<u64, Vec<u64>> {
        self.done.get(session).cloned().unwrap_or_default()
    }
}
const NO_PROGRAM_READY: &str = "no program_ready, R-60";
const NO_AWAIT_TITLE: &str = "no await_title, R-57";
const UNFENCED_ATTACH: &str = "no await_title before the attach, R-57";
const UNFENCED_READ: &str = "no output fence before the model read, R-80";

/// The program output controls of a session: the output that reaches the worker, then the model. `uncarriable_sequence`
/// writes the start of an OSC or DCS string with no terminator (R-88, the R-80 follow-up).
const PROGRAM_OUTPUT_OPS: [&str; 4] = [
    "pty_output",
    "pty_chunk",
    "program_write_once",
    "uncarriable_sequence",
];

/// A model read of a session (R-80): a `ReadScreen` or `CaptureSnapshot`, a `begin` whose operation has a `model_rev` guard,
/// a `terminal_state` call, an `attach_route` (its baseline) and a `queue_overflow` of a route of the session (its resync;
/// R-88, the R-80 follow-up). Its session and what it is. The read's own reply wait is not part of it, and is no fence.
fn model_read(step: &Value, routes: &Routes) -> Option<(String, &'static str)> {
    if let Some(op) = step.pointer("/begin/op").and_then(Value::as_object) {
        let (name, body) = op.iter().next()?;
        let session = body.get("session").or_else(|| body.get("id"))?.as_str()?;
        let what = match name.as_str() {
            "ReadScreen" => "ReadScreen",
            "CaptureSnapshot" => "CaptureSnapshot",
            _ if body.pointer("/guard/model_rev").is_some() => "a guarded begin",
            _ => return None,
        };
        return Some((session.to_string(), what));
    }
    if let Some(session) = step
        .pointer("/attach_route/session")
        .and_then(Value::as_str)
    {
        return Some((session.to_string(), "an attach (its baseline)"));
    }
    if control(step, "queue_overflow").is_some() {
        let session = routes.get(route_of(step))?;
        return Some((session.clone(), "a queue_overflow (its resync)"));
    }
    let call = step.get("call")?;
    if call.get("method").and_then(Value::as_str)? != "terminal_state" {
        return None;
    }
    let args = call.get("args")?;
    let session = args.get("id").or_else(|| args.get("session"))?.as_str()?;
    Some((session.to_string(), "terminal_state"))
}

/// Whether the last bytes of an output (hex) are the sequence that posts a derived event.
type LastBytesPost = fn(&str) -> bool;

/// The events that only the application of an output can post, so a wait that ends on one of them can fence that output
/// (R-80 addition (1)): each with the test that the output's last bytes are the sequence that posts it.
const DERIVED_EVENTS: [(&str, LastBytesPost); 3] = [
    ("ModesChanged", ends_with_a_mode_sequence),
    ("TitleChanged", ends_with_a_title_sequence),
    ("Bell", ends_with_a_bell),
];

/// Whether the last bytes are a mode sequence: `ESC =` or `ESC >`, a CSI whose parameter bytes run to a final `h` or `l`
/// (DEC and ANSI modes), or a CSI `=`, `>` or `<` ... `u` (the kitty keyboard flags: set, push, pop). A CSI `u` with no such
/// first byte is no mode change: `CSI u` restores the cursor (SCORC) and `CSI ? u` queries the flags (017f #54 F4).
fn ends_with_a_mode_sequence(hex: &str) -> bool {
    let bytes: Vec<&str> = hex_bytes(hex).collect();
    if bytes.ends_with(&["1b", "3d"]) || bytes.ends_with(&["1b", "3e"]) {
        return true;
    }
    let Some(csi) = bytes.windows(2).rposition(|w| w == ["1b", "5b"]) else {
        return false;
    };
    let tail = &bytes[csi + 2..];
    let Some((last, params)) = tail.split_last() else {
        return false;
    };
    let byte = |b: &str| u8::from_str_radix(b, 16).unwrap_or(0);
    let final_byte = match *last {
        "68" | "6c" => true,
        "75" => params
            .first()
            .is_some_and(|b| ["3d", "3e", "3c"].contains(b)),
        _ => false,
    };
    final_byte && params.iter().all(|b| (0x20..=0x3f).contains(&byte(b)))
}

/// Whether the last bytes are an OSC 0 or OSC 2 title, ended by `ST` or `BEL`, with no other `ESC` inside it.
fn ends_with_a_title_sequence(hex: &str) -> bool {
    let bytes: Vec<&str> = hex_bytes(hex).collect();
    let Some(osc) = bytes.windows(2).rposition(|w| w == ["1b", "5d"]) else {
        return false;
    };
    let tail = &bytes[osc + 2..];
    let body = if tail.ends_with(&["1b", "5c"]) {
        &tail[..tail.len() - 2]
    } else if tail.ends_with(&["07"]) {
        &tail[..tail.len() - 1]
    } else {
        return false;
    };
    (body.starts_with(&["30", "3b"]) || body.starts_with(&["32", "3b"]))
        && !body.contains(&"1b")
        && !body.contains(&"07")
}

/// Whether the last byte is a `BEL` that ends no OSC.
fn ends_with_a_bell(hex: &str) -> bool {
    let bytes: Vec<&str> = hex_bytes(hex).collect();
    if bytes.last() != Some(&"07") {
        return false;
    }
    let before = &bytes[..bytes.len() - 1];
    match before.windows(2).rposition(|w| w == ["1b", "5d"]) {
        // An OSC before it is ended earlier (by ST or BEL), so this BEL is its own.
        Some(osc) => {
            before[osc + 2..].contains(&"07")
                || before[osc + 2..].windows(2).any(|w| w == ["1b", "5c"])
        }
        None => true,
    }
}

/// The event of a plain `Activity {id: S, source: Output}` wait, in `output_fences`.
const ACTIVITY: &str = "Activity";

/// The output of a session that no output fence has met (R-80): the bytes of its last output step, and the count of every
/// output byte since the last fence, across steps. A control whose bytes the transcript does not give (`pty_chunk`,
/// `uncarriable_sequence`, a variable) has no known count.
#[derive(Debug, Clone)]
struct Unfenced {
    last_hex: String,
    bytes: Option<usize>,
}

impl Unfenced {
    /// The unfenced output after the output control `c`, given the unfenced output before it.
    fn after(before: Option<&Unfenced>, c: &Value) -> Unfenced {
        let hex = c.get("bytes_hex").and_then(Value::as_str);
        let n = hex.filter(|h| !is_variable(h)).map(|h| h.len() / 2);
        Unfenced {
            last_hex: hex.unwrap_or("").to_string(),
            bytes: match before {
                None => n,
                Some(b) => b.bytes.zip(n).map(|(a, n)| a + n),
            },
        }
    }
}

/// Whether an output fence covers a session's unfenced output (R-80; `output_fences` gives the fence's event).
/// - An `await_title` (no event) covers it: the worker applies bytes in order, so the title proves every byte before it.
/// - A plain `Activity` wait covers it only when exactly one output byte is unfenced, counted across steps. A real read
///   can split longer output, and the wait ends at the first `Activity` (R-80 condition 3, the steward's follow-up of
///   2026-10-11 on `ad_3`, in its strict form).
/// - A derived event covers it only when the last output step's last bytes post that event (addition (1) c).
fn output_fence_covers(event: Option<&str>, out: &Unfenced) -> bool {
    match event {
        None => true,
        Some(ACTIVITY) => out.bytes == Some(1),
        Some(name) => DERIVED_EVENTS
            .iter()
            .any(|(n, last)| *n == name && last(&out.last_hex)),
    }
}

/// An output fence of a session in a step (R-80): the session, and the event that the fence ends on (none for a title).
/// `output_fence_covers` decides which output it covers.
/// - `Activity {id: S, source: Output}` in the target of a `pump_until`, `pump_collect` or `await_*` wait: `ACTIVITY`.
/// - An `await_title` of S (an R-57 title sentinel; the R-57 rule checks the sentinel): no event.
/// - A derived event of S (`DERIVED_EVENTS`) that ends a `pump_until` (its `event`) or a `pump_collect` (one of its
///   `events`, which it cannot end before).
///
/// An `ignore` or `forbid` matcher never ends a wait, so it fences nothing. A wait on another session's event fences
/// nothing of S. Distinctness (R-61 (a)(2)) is stated per fence in the pull request.
fn output_fences(step: &Value) -> Vec<(String, Option<&'static str>)> {
    /// The sessions whose `Activity {source: Output}` every match of `v` requires. A `$one_of` requires only what each of
    /// its alternatives requires: `Activity {s1} or Bell {s1}` can end on the `Bell` alone (016c #54 R2-1). Any other `$`
    /// operator (`$not`, `$any`, ...) requires nothing.
    fn required(v: &Value) -> BTreeSet<String> {
        match v {
            Value::Object(m) => {
                let mut out = BTreeSet::new();
                if let Some(a) = m.get("Activity") {
                    if a.get("source").and_then(Value::as_str) == Some("Output") {
                        out.extend(a.get("id").and_then(Value::as_str).map(str::to_string));
                    }
                }
                if let Some(alternatives) = m.get("$one_of").and_then(Value::as_array) {
                    let mut each = alternatives.iter().map(required);
                    if let Some(first) = each.next() {
                        out.extend(each.fold(first, |all, alt| &all & &alt));
                    }
                }
                for (k, x) in m {
                    if k != "ignore" && k != "forbid" && !k.starts_with('$') {
                        out.extend(required(x));
                    }
                }
                out
            }
            Value::Array(a) => a.iter().flat_map(required).collect(),
            _ => BTreeSet::new(),
        }
    }
    fn activities(v: &Value, out: &mut Vec<(String, Option<&'static str>)>) {
        out.extend(required(v).into_iter().map(|s| (s, Some(ACTIVITY))));
    }
    fn derived(event: &Value, out: &mut Vec<(String, Option<&'static str>)>) {
        for (name, _) in DERIVED_EVENTS {
            if let Some(s) = event
                .pointer(&format!("/{name}/id"))
                .and_then(Value::as_str)
            {
                out.push((s.to_string(), Some(name)));
            }
        }
    }
    let mut out = vec![];
    let Some(obj) = step.as_object() else {
        return out;
    };
    for (kind, spec) in obj {
        if kind == "await_title" {
            out.extend(
                spec.get("session")
                    .and_then(Value::as_str)
                    .map(|s| (s.to_string(), None)),
            );
        } else if kind == "pump_until" || kind == "pump_collect" || kind.starts_with("await_") {
            activities(spec, &mut out);
        }
        if kind == "pump_until" {
            derived(&spec["event"], &mut out);
        }
        if kind == "pump_collect" {
            for event in spec
                .get("events")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                derived(event, &mut out);
            }
        }
    }
    out
}

fn lint(id: &str, steps: &[Value]) -> Vec<Finding> {
    let mut out = vec![];
    let mut find = |step: usize, rule: &'static str, what: String| {
        out.push(Finding {
            id: id.to_string(),
            step,
            rule,
            what,
        })
    };
    // R-61: hex digits compare without case, as the matcher does.
    let steps: Vec<Value> = steps.iter().map(lowercase_hex).collect();
    let mut routes = Routes::new();
    // The R-57 sweep: the handle that attached each route.
    let mut route_handles: HashMap<String, String> = HashMap::new();
    // R-56: the session's last `await_pty` since its last write.
    let mut fences: HashMap<String, Fence> = HashMap::new();
    // R-56 (3)(1): the variables that a `let` bound, with what the lint knows of their values.
    let mut lets = Lets::default();
    // The `WriteInput`s that `begin` steps bound, for their written counts (R-74).
    let mut writes: HashMap<String, Write> = HashMap::new();
    // R-57: the step of each session's last `pty_output`, and the last launch or adoption.
    let mut last_output: Vec<(usize, String, String)> = vec![];
    let mut last_baseline: Option<usize> = None;
    // The prints of each session's own program: the step of its last print is the step of a sentinel that it prints.
    let mut programs: HashMap<String, Program> = HashMap::new();
    // The R-57 sweep: each session's last `pty_output` that no read and no `await_title` has met yet.
    let mut unread: std::collections::BTreeMap<(String, String), usize> =
        std::collections::BTreeMap::new();
    // The attach rule: each route whose attach followed an unmet output of its session, with that output's step and the
    // attach's step.
    let mut attached: HashMap<String, (usize, usize)> = HashMap::new();
    // R-60: the sessions that an `await_control` of `program_ready` has fenced, the first host input of each session, and the
    // variables that a `tap_drain` bound to a session's bytes.
    let mut ready: BTreeSet<String> = BTreeSet::new();
    let mut first_input: HashMap<String, (usize, bool)> = HashMap::new();
    let mut tap: HashMap<String, String> = HashMap::new();
    // R-73 item 5: the bytes that each op's sent frames carry, which `written_by_op` asserts exactly.
    let mut sent: HashMap<u64, Vec<u8>> = HashMap::new();
    // R-83: the known runs of each op so far.
    let mut known_runs = KnownRuns::default();
    // R-73 follow-up, steward 2026-10-11 (017f F2 on #53): whether a sent body of each session holds `0d`.
    let mut sent_0d: BTreeSet<String> = BTreeSet::new();
    // R-62 Q3: the `pty_output` steps already reported for a `0a` that an `oracle_resume` reads.
    let mut resumed: BTreeSet<usize> = BTreeSet::new();
    // R-80: each session's program output that no output fence of the session has met, with its last step.
    let mut unfenced: HashMap<String, (usize, Unfenced)> = HashMap::new();
    for (i, unit) in units(&steps) {
        let step = &unit;
        if let Some(attach) = step.get("attach_route") {
            // `r1` when `route` is absent or not a string, as the driver's `attach` reads it (#41 R1-3).
            let r = attach.get("route").and_then(Value::as_str).unwrap_or("r1");
            if let Some(s) = attach.get("session").and_then(Value::as_str) {
                // R-61 (a)(1), the steward's ou_9 ruling: the attach takes its baseline at once, as a sync call of the
                // session does. An output of the session that no `await_title` and no wait has met, on any handle, can still
                // be inside the worker, and then the route receives it as live output.
                let output = unread
                    .iter()
                    .filter(|((_, session), _)| session == s)
                    .map(|(_, at)| *at)
                    .max();
                match output {
                    Some(at) => attached.insert(r.to_string(), (at, i)),
                    None => attached.remove(r),
                };
                routes.insert(r.to_string(), s.to_string());
                // The driver attaches the route on the step's handle (`handle`, else `h`).
                route_handles.insert(r.to_string(), defs::handle_of(attach));
            }
        }
        // R-60: the input log and the output that a step asserts, literal or computed, which need `program_ready` before the
        // session's first input.
        let mut asserted: BTreeSet<String> = BTreeSet::new();
        asserted.extend(defs::reads_input_log(step, &routes));
        asserted.extend(defs::output_assertions(step, &routes, &tap));
        for (_, hex) in defs::exact_outputs(step, &routes, &tap) {
            if hex_bytes(&hex).any(|b| b == "0a") {
                find(
                    i,
                    "R-60",
                    format!("an exact output assertion contains 0a: {hex}"),
                );
            }
        }
        for (_, hex) in defs::exact_inputs(step, &routes) {
            if hex_bytes(&hex).any(|b| b == "0d") {
                find(
                    i,
                    "R-61",
                    format!("an exact input assertion contains 0d: {hex}"),
                );
            }
        }
        if let Some((op, bytes)) = defs::sent_input(step) {
            sent.entry(op).or_default().extend(bytes);
        }
        if let Some((_, bytes)) = defs::sent_body(step) {
            if let Some(session) = routes.get(route_of(step)).filter(|_| bytes.contains(&0x0d)) {
                sent_0d.insert(session.clone());
            }
        }
        // R-73 follow-up, steward 2026-10-11 (017f F2 on #53): the accounting of `written_without_op` looks for an unplaced op's
        // bytes in the log, and on the real tier `ICRNL` turns `0d` into `0a`, so a sent body of the session must hold no `0d`
        // (R-61 (c): sent bytes stay findable in the log).
        if defs::asserts_without_op(step) {
            if let Some(session) = routes.get(route_of(step)).filter(|s| sent_0d.contains(*s)) {
                find(
                    i,
                    "R-61",
                    format!("written_without_op on {session}, whose sent bytes, text or paste body contain 0d (R-73 follow-up)"),
                );
            }
        }
        for op in defs::ops_by_relation(step) {
            if sent.get(&op).is_some_and(|bytes| bytes.contains(&0x0d)) {
                find(
                    i,
                    "R-61",
                    format!("written_by_op {op} asserts sent bytes that contain 0d (R-73)"),
                );
            }
        }
        // The attach rule: one finding per attach, at the output's step, as the sweep reports.
        if let Some(r) = defs::exact_output_route(step) {
            if let Some((at, attach)) = attached.remove(r) {
                let session = &routes[r];
                find(
                    at,
                    UNFENCED_ATTACH,
                    format!("the attach of {r} at step {attach} needs an await_title fence of {session} before it, because step {i} asserts its exact output (R-61 (a)(1))"),
                );
            }
        }
        for session in asserted {
            if let Some((at, fenced)) = first_input.get_mut(&session) {
                if !*fenced {
                    // One finding per session: at its first input.
                    *fenced = true;
                    find(
                        *at,
                        NO_PROGRAM_READY,
                        format!("the first input to {session} before its program_ready"),
                    );
                }
            }
        }
        // R-56: a read of the input log, before this step's own writes.
        if let Some(session) = pty_input(step) {
            let literal = step
                .pointer("/expect/bytes/$bytes_hex")
                .and_then(Value::as_str);
            let fence = fences.get(session).copied();
            let covers = |len| fence.is_some_and(|f| f.covers(len));
            match literal {
                Some("") => {}
                Some(hex) if covers(Some((hex.len() / 2) as u64)) => {}
                Some(hex) => find(
                    i,
                    "R-56",
                    format!(
                        "a literal log of {} bytes of {session} with the fence {fence:?}",
                        hex.len() / 2
                    ),
                ),
                // R-56 (3)(1): a capture's `min_bytes` comes from a bound number (`Fence::covers`).
                None if covers(None) => {}
                None => find(
                    i,
                    UNFENCED_CAPTURE,
                    format!("a capture of the log of {session} with the fence {fence:?}"),
                ),
            }
        }
        // R-56 on a route: a `route_expect_pty` reads the input log of the route's session. Each sequence needs a log of at least
        // its literal bytes and one byte for each relation part (a relation run is not empty, R-73), so the fence covers the
        // shortest one. A number that the lint knows (a literal, or a fixed `let`) must cover that floor; a run-time count (R-83
        // (B), (C)) is the reviewer's check, as for a capture. A floor of 0 (`var` parts only) still needs a fence of at least 1
        // byte, because a fence of 0 returns at once (017f F4): only an empty sequence is an absence check, and
        // `route_pty_least` gives none for it.
        if let (Some(frame), Some(session)) = (defs::sent_frame(step), routes.get(route_of(step))) {
            known_runs.sent(route_of(step), session, frame);
        }
        if let Some(session) = routes.get(route_of(step)) {
            if let Some(least) = defs::route_pty_least(step, &known_runs.of(session)) {
                let fence = fences.get(session).copied();
                if !fence
                    .is_some_and(|f| f.covers(Some(least.max(1))) || f == Fence::Bound(Num::Run))
                {
                    find(
                        i,
                        UNFENCED_ROUTE_READ,
                        format!("a route_expect_pty of at least {least} bytes of {session} with the fence {fence:?}"),
                    );
                }
            }
        }
        // A variable that any other binding form replaces (`bind`, `bind_fields`, an `attach_route` route) is no longer the
        // `let` value: its class is gone, so it fences nothing (#39 round 4). It is no longer a `WriteInput` op either: only
        // the step's own `WriteInput` binding installs one again (R-74 condition 2: the same session).
        let mut written = written_counts(step, &writes);
        written.extend(route_written_counts(step, &routes));
        match rebound(step) {
            Rebound::Names(vars) => {
                for var in vars {
                    lets.forget(&var);
                    writes.remove(&var);
                }
            }
            Rebound::All => {
                lets = Lets::default();
                writes.clear();
            }
        }
        for (var, session) in written {
            lets.written_count(var, session);
        }
        if let Some((var, write)) = write_input(step) {
            writes.insert(var, write);
        }
        if let Some(bound) = step.get("let").and_then(Value::as_object) {
            for (var, expr) in bound {
                writes.remove(&var_name(var));
                lets.bind(var, expr);
            }
        }
        if let Some(spec) = step.get("await_pty") {
            if let Some(session) = spec.get("session").and_then(Value::as_str) {
                let fence = match spec.get("min_bytes") {
                    None => Fence::Literal(1),
                    // A negative or fractional number is not an unsigned count; the driver reads its default.
                    Some(Value::Number(n)) => n.as_u64().map_or(Fence::Invalid, Fence::Literal),
                    // A written count fences only its own session's log (R-74).
                    Some(Value::String(var))
                        if is_variable(var)
                            && lets
                                .written
                                .get(&var_name(var))
                                .is_some_and(|of| of != session) =>
                    {
                        Fence::Invalid
                    }
                    Some(Value::String(var))
                        if is_variable(var) && lets.class.contains_key(&var_name(var)) =>
                    {
                        Fence::Bound(lets.class[&var_name(var)])
                    }
                    Some(_) => Fence::Invalid,
                };
                fences.insert(session.to_string(), fence);
            }
        }
        for session in defs::writes(step, &routes) {
            fences.remove(&session);
        }
        if let Some((session, program)) = Program::of(step) {
            programs.insert(session.to_string(), program);
        }
        if let Some(session) = step.pointer("/begin/op/Start/id").and_then(Value::as_str) {
            if let Some(program) = programs.get_mut(session) {
                program.start(i);
            }
        }
        for session in defs::host_inputs(step, &routes) {
            if let Some(program) = programs.get_mut(&session) {
                program.input(input_hex(step), i);
            }
            let fenced = ready.contains(&session);
            first_input.entry(session).or_insert((i, fenced));
        }
        if let Some(c) = step.pointer("/await_control/control") {
            if c.get("op").and_then(Value::as_str) == Some("program_ready")
                && step.pointer("/await_control/expect/ready") == Some(&json!(true))
            {
                ready.extend(c.get("session").and_then(Value::as_str).map(str::to_string));
            }
        }
        if let Some(bytes) = step.pointer("/bind_fields/bytes").and_then(Value::as_str) {
            if let Some(session) = step.pointer("/tap_drain/session").and_then(Value::as_str) {
                tap.insert(bytes.to_string(), session.to_string());
            }
        }
        // R-57: the scope of a title fence.
        let op = step.pointer("/begin/op");
        let launch = op
            .and_then(Value::as_object)
            .is_some_and(|o| o.contains_key("Start") || o.contains_key("Adopt"))
            || op.and_then(Value::as_str) == Some("AdoptAll")
            || step.get("open").is_some();
        if launch {
            last_baseline = Some(i);
        }
        // The R-57 sweep (R-61 (a)): the first read of a session after its last `pty_output`. Only the last output before a
        // read needs the fence, because one PTY keeps the order of its bytes.
        if let Some((scope, needs_fence)) = defs::output_read(step, &routes, &route_handles) {
            let read: Vec<(String, String)> = unread
                .keys()
                .filter(|(handle, session)| scope.reaches(handle, session))
                .cloned()
                .collect();
            for key in read {
                let at = unread.remove(&key).unwrap_or(i);
                let session = &key.1;
                if needs_fence {
                    find(
                        at,
                        NO_AWAIT_TITLE,
                        format!("the read at step {i} needs an await_title fence of {session} (R-61 (a))"),
                    );
                }
            }
        }
        // The output of a session reaches the Core of the handle that its control names; an `await_title` fences it there.
        if let Some(spec) = step.get("await_title") {
            if let Some(session) = spec.get("session").and_then(Value::as_str) {
                unread.remove(&(defs::handle_of(spec), session.to_string()));
            }
        }
        if let Some((session, hex)) = pty_output(step) {
            last_output.push((i, session.to_string(), hex.to_string()));
            let handle = defs::handle_of(&step["control"]);
            unread.insert((handle, session.to_string()), i);
        }
        // R-62 Q3: the oracle applies a session's `pty_output` bytes unchanged on both tiers, so none of them is a `0a`, which
        // `ONLCR` changes. Reported once, at the output's step.
        if step.pointer("/control/op").and_then(Value::as_str) == Some("oracle_resume") {
            let session = step.pointer("/control/session").and_then(Value::as_str);
            for (at, s, hex) in &last_output {
                if Some(s.as_str()) == session
                    && hex_bytes(hex).any(|b| b == "0a")
                    && resumed.insert(*at)
                {
                    find(
                        *at,
                        "R-60",
                        format!("a pty_output of {s} that the oracle_resume at step {i} reads contains 0a: {hex}"),
                    );
                }
            }
        }
        if let Some(spec) = step.get("await_title") {
            let session = spec.get("session").and_then(Value::as_str).unwrap_or("");
            let title = spec.get("title").and_then(Value::as_str).unwrap_or("");
            let at = last_output
                .iter()
                .rev()
                .find(|(_, s, hex)| s == session && sets_title(hex, title))
                .map(|(at, ..)| *at)
                .or_else(|| {
                    // A program's sentinel covers no pty_output of the session after it.
                    programs
                        .get(session)
                        .and_then(|p| p.sentinel(title))
                        .filter(|at| !last_output.iter().any(|(o, s, _)| s == session && o > at))
                });
            match at {
                None => find(
                    i,
                    "R-57",
                    format!(
                        "no pty_output of {session}, and no print of its program after all its pty_output, carries the title {title:?}"
                    ),
                ),
                Some(at) if last_baseline.is_some_and(|b| b > at && b < i) => find(
                    i,
                    "R-57",
                    format!(
                        "a launch or adoption at step {} falls between the sentinel and the wait",
                        last_baseline.unwrap()
                    ),
                ),
                Some(_) => {}
            }
        }
        // R-80: a model read of a session after an output of the session, on any handle, with no output fence of the session
        // between them. The worker can still hold that output, so the read can see the model before it. One finding per
        // read; a read is no fence, so a later read after the same output is a finding too.
        if let Some((session, what)) = model_read(step, &routes) {
            if let Some((at, _)) = unfenced.get(&session) {
                find(
                    i,
                    UNFENCED_READ,
                    format!("{what} of {session} at step {i} needs an output fence of {session} after its output at step {at} (R-80)"),
                );
            }
        }
        for (session, event) in output_fences(step) {
            let fences = unfenced
                .get(&session)
                .is_some_and(|(_, out)| output_fence_covers(event, out));
            if fences {
                unfenced.remove(&session);
            }
        }
        if let Some(c) = step.get("control") {
            let op = c.get("op").and_then(Value::as_str).unwrap_or("");
            if let Some(session) = c.get("session").and_then(Value::as_str) {
                if PROGRAM_OUTPUT_OPS.contains(&op) {
                    let out = Unfenced::after(unfenced.get(session).map(|(_, o)| o), c);
                    unfenced.insert(session.to_string(), (i, out));
                }
            }
        }
    }
    out
}

/// The entries of the exception list: `<id>  step <n>  <reason>`, or `<id>  step <n>  <reason>  <note>`, with fields
/// separated by two or more spaces. The note never decides a match. It is `debt: <owner and fence>` for a planned fix, or
/// `exception: <why no fence works>` (the steward, R-88 lint format, 2026-10-11).
fn parse_exceptions(text: &str) -> Result<BTreeSet<(String, usize, String)>, String> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|line| {
            let fields: Vec<&str> = line
                .split("  ")
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .collect();
            let (id, step, reason) = match fields[..] {
                [id, step, reason] => (id, step, reason),
                [id, step, reason, note] => {
                    if !(note.starts_with("debt: ") || note.starts_with("exception: ")) {
                        return Err(format!(
                            "an exception's note starts with `debt: ` or `exception: `: {line:?}"
                        ));
                    }
                    (id, step, reason)
                }
                _ => return Err(format!("an exception needs three or four fields: {line:?}")),
            };
            let step = step
                .strip_prefix("step ")
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| format!("an exception's second field is `step <n>`: {line:?}"))?;
            Ok((id.to_string(), step, reason.to_string()))
        })
        .collect()
}

fn exceptions() -> BTreeSet<(String, usize, String)> {
    parse_exceptions(include_str!(
        "../../../conformance/core-fence-exceptions.txt"
    ))
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn every_core_transcript_keeps_the_fence_and_line_discipline_rules() {
    let transcripts =
        botster_conformance::load_dir(&botster_core_conformance::CORE_TRANSCRIPTS).unwrap();
    let mut findings = vec![];
    for t in &transcripts {
        findings.extend(lint(&t.id, &t.steps));
        findings.extend(report::lint(&t.id, &t.steps));
    }
    let listed = exceptions();
    let excepted: BTreeSet<(String, usize, String)> = findings
        .iter()
        .filter(|f| {
            [
                UNFENCED_CAPTURE,
                UNFENCED_ROUTE_READ,
                NO_PROGRAM_READY,
                NO_AWAIT_TITLE,
                UNFENCED_READ,
                report::NO_REPORT_FENCE,
            ]
            .contains(&f.rule)
        })
        .map(|f| (f.id.clone(), f.step, f.rule.to_string()))
        .collect();
    let mut errors: Vec<String> = findings
        .iter()
        .filter(|f| !listed.contains(&(f.id.clone(), f.step, f.rule.to_string())))
        .map(|f| format!("{} step {} ({}): {}", f.id, f.step, f.rule, f.what))
        .collect();
    errors.extend(listed.difference(&excepted).map(|(id, step, reason)| {
        format!("{id}  step {step}  {reason}: no finding needs this exception; remove it")
    }));
    assert!(
        errors.is_empty(),
        "{} errors:\n{}",
        errors.len(),
        errors.join("\n")
    );
}

/// The rules of each finding of a step list.
fn rules(steps: Value) -> Vec<(usize, &'static str)> {
    lint("t", steps.as_array().unwrap())
        .into_iter()
        .map(|f| (f.step, f.rule))
        .collect()
}

fn write(session: &str) -> Value {
    json!({"begin": {"op": {"WriteInput": {"session": session, "payload": {"bytes": {"bytes": {"$bytes_hex": "61"}}}}}}})
}

fn read(session: &str, expect: impl Into<Value>) -> Value {
    let expect: Value = expect.into();
    json!({"control": {"op": "pty_input", "session": session}, "expect": {"bytes": expect}})
}

fn fence(session: &str, min: u64) -> Value {
    json!({"await_pty": {"session": session, "min_bytes": min}})
}

/// The `let` step that binds `$n` to a length that the run fixes (`hex_len` of a value bound at run time), for `bound_fence`.
/// A `let` is a step of its own.
fn let_n() -> Value {
    json!({"let": {"$n": {"hex_len": ["$reply", 0]}}})
}

/// A fence whose `min_bytes` is the bound value `$n` (a `let` sum), as a capture needs (R-56 (3)(1)). `let_n` comes before it.
fn bound_fence(session: &str) -> Value {
    json!({"await_pty": {"session": session, "min_bytes": "$n"}})
}

fn ready(session: &str) -> Value {
    json!({"await_control": {"control": {"op": "program_ready", "session": session}, "expect": {"ready": true}}})
}

#[test]
fn a_read_of_the_input_log_needs_a_fence_after_the_last_write() {
    let lit = |hex: &str| json!({"$bytes_hex": hex});
    // Fenced, with enough bytes: no finding.
    assert_eq!(
        rules(json!([
            ready("s1"),
            write("s1"),
            fence("s1", 2),
            read("s1", lit("6162"))
        ])),
        vec![]
    );
    // Too few bytes, and no fence: a literal finding.
    assert_eq!(
        rules(json!([
            ready("s1"),
            write("s1"),
            fence("s1", 1),
            read("s1", lit("6162"))
        ])),
        vec![(3, "R-56")]
    );
    assert_eq!(
        rules(json!([ready("s1"), write("s1"), read("s1", lit("61"))])),
        vec![(2, "R-56")]
    );
    // A write after the fence clears it; a fence of another session is no fence.
    assert_eq!(
        rules(json!([
            ready("s1"),
            write("s1"),
            fence("s1", 1),
            write("s1"),
            read("s1", lit("61"))
        ])),
        vec![(4, "R-56")]
    );
    assert_eq!(
        rules(json!([
            ready("s1"),
            write("s1"),
            fence("s2", 1),
            read("s1", "$log")
        ])),
        vec![(3, UNFENCED_CAPTURE)]
    );
    // An empty literal is an absence check, not this pattern; a capture fenced by a bound number passes.
    assert_eq!(rules(json!([read("s1", lit(""))])), vec![]);
    assert_eq!(
        rules(json!([
            let_n(),
            ready("s1"),
            write("s1"),
            bound_fence("s1"),
            read("s1", "$log")
        ])),
        vec![]
    );
    // A number in the transcript, 0 or more, does not fence a capture (R-56 (3)(1)): a bind or a pattern.
    let bind =
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$log"}});
    for min in [0, 1, 5] {
        for capture in [read("s1", "$log"), bind.clone()] {
            assert_eq!(
                rules(json!([ready("s1"), write("s1"), fence("s1", min), capture])),
                vec![(3, UNFENCED_CAPTURE)],
                "{min}"
            );
        }
    }
}

#[test]
fn route_input_and_a_terminal_query_are_writes() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let send = json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}});
    let query = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "1b5b63"}});
    let plain = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "41"}});
    let barrier = json!({"expect_quiet_until": {"$any": true}, "barrier": send});
    for writer in [send.clone(), query, barrier] {
        assert_eq!(
            rules(json!([
                let_n(),
                ready("s1"),
                attach,
                bound_fence("s1"),
                writer,
                read("s1", "$log")
            ])),
            vec![(5, UNFENCED_CAPTURE)]
        );
    }
    assert_eq!(
        rules(json!([
            let_n(),
            ready("s1"),
            attach,
            bound_fence("s1"),
            plain,
            read("s1", "$log")
        ])),
        vec![]
    );
    // An `attach_route` without a string `route` attaches `r1`, as the driver does, so a send on the default route writes to
    // its session (#41 R1-3).
    let raw = json!({"route_send_raw": {"bytes_hex": "61"}});
    for default in [
        json!({"attach_route": {"session": "s1", "client": "c1"}}),
        json!({"attach_route": {"route": 7, "session": "s1", "client": "c1"}}),
    ] {
        for writer in [send.clone(), raw.clone()] {
            assert_eq!(
                rules(json!([
                    let_n(),
                    ready("s1"),
                    default,
                    bound_fence("s1"),
                    writer,
                    read("s1", "$log")
                ])),
                vec![(5, UNFENCED_CAPTURE)],
                "{default} {writer}"
            );
        }
    }
    // A send on a route that is not attached writes to no session.
    assert_eq!(
        rules(json!([
            let_n(),
            bound_fence("s1"),
            send,
            read("s1", "$log")
        ])),
        vec![]
    );
}

#[test]
fn the_first_input_needs_program_ready_when_a_later_step_asserts_the_log() {
    let lit = json!({"$bytes_hex": "61"});
    assert_eq!(
        rules(json!([
            write("s1"),
            fence("s1", 1),
            read("s1", lit.clone())
        ])),
        vec![(0, NO_PROGRAM_READY)]
    );
    // One finding per session, at its first input; nothing when no later step asserts.
    assert_eq!(
        rules(json!([
            write("s1"),
            write("s1"),
            fence("s1", 2),
            read("s1", lit.clone()),
            read("s1", lit.clone())
        ])),
        vec![(0, NO_PROGRAM_READY)]
    );
    assert_eq!(rules(json!([write("s1")])), vec![]);
    // `program_ready` after the first input is too late; with `ready: false` it is no fence.
    assert_eq!(
        rules(json!([
            write("s1"),
            ready("s1"),
            fence("s1", 1),
            read("s1", lit.clone())
        ])),
        vec![(0, NO_PROGRAM_READY)]
    );
    let not_ready = json!({"await_control": {"control": {"op": "program_ready", "session": "s1"}, "expect": {"ready": false}}});
    assert_eq!(
        rules(json!([
            not_ready,
            write("s1"),
            fence("s1", 1),
            read("s1", lit)
        ])),
        vec![(1, NO_PROGRAM_READY)]
    );
}

#[test]
fn an_exact_assertion_keeps_no_byte_that_the_line_discipline_changes() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let output = |hex: &str| json!({"route_expect_output": {"bytes_hex": hex}});
    // A byte, not a pair of digits across two bytes: `a0a1` holds no `0a`.
    assert_eq!(rules(json!([attach, output("a0a1")])), vec![]);
    assert_eq!(rules(json!([attach, output("410a")])), vec![(1, "R-60")]);
    let parts = json!({"route_expect_output": {"parts": [{"fill": "$f"}, {"bytes_hex": "0a"}]}});
    let frame = json!({"expect_eventually": {"frame": "output", "payload": {"$bytes_hex": "0a"}}, "on": "r1"});
    let tap = json!([
        {"tap_drain": {"session": "s1"}, "bind_fields": {"bytes": "$b"}},
        {"check": {"eq": ["$b", {"$bytes_hex": "0a"}]}}
    ]);
    assert_eq!(rules(json!([attach, parts])), vec![(1, "R-60")]);
    assert_eq!(rules(json!([attach, frame])), vec![(1, "R-60")]);
    assert_eq!(rules(tap), vec![(1, "R-60")]);
    // A check of a variable that no `tap_drain` bound is not an output assertion.
    assert_eq!(
        rules(json!([{"check": {"eq": ["$x", {"$bytes_hex": "0a"}]}}])),
        vec![]
    );
    // The input direction: no `0d`; `0a` is allowed there.
    assert_eq!(
        rules(json!([read("s1", json!({"$bytes_hex": "0d"}))])),
        vec![(0, "R-61"), (0, "R-56")]
    );
    // A route read is also an R-56 read of the log (R-83): this one has no fence.
    assert_eq!(
        rules(json!([attach, {"route_expect_pty": [{"hex": "0d"}]}])),
        vec![(1, "R-61"), (1, "unfenced route read, R-56")]
    );
    assert_eq!(
        rules(json!([attach, {"route_expect_pty": [{"hex": "0a"}]}])),
        vec![(1, "unfenced route read, R-56")]
    );
}

#[test]
fn a_title_fence_has_its_sentinel_and_no_launch_or_adoption_before_it() {
    let sentinel = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": format!("07{}1b5c", sentinel_hex("t"))}});
    let wait = json!({"await_title": {"session": "s1", "title": "t"}});
    assert_eq!(rules(json!([sentinel, wait])), vec![]);
    for launch in [
        json!({"begin": {"op": {"Start": {"id": "s2"}}}}),
        json!({"begin": {"op": {"Adopt": {"id": "s1"}}}}),
        json!({"begin": {"op": "AdoptAll"}}),
        json!({"open": {"as": "h2"}}),
    ] {
        assert_eq!(rules(json!([sentinel, launch, wait])), vec![(2, "R-57")]);
        // Before the sentinel, a launch is outside the fence.
        assert_eq!(rules(json!([launch, sentinel, wait])), vec![]);
    }
    // No sentinel of this title, or one on another session.
    let other = json!({"control": {"op": "pty_output", "session": "s2", "bytes_hex": format!("{}07", sentinel_hex("t"))}});
    assert_eq!(rules(json!([other, wait])), vec![(1, "R-57")]);
    assert_eq!(rules(json!([wait])), vec![(0, "R-57")]);
    // OSC 0 sets the title too; either terminator ends it.
    let output =
        |hex: &str| json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": hex}});
    for hex in ["1b5d303b741b5c", "1b5d303b7407", "1b5d323b7407"] {
        assert_eq!(rules(json!([output(hex), wait])), vec![], "{hex}");
    }
    // A longer title, an unterminated title, another OSC, or a match off the byte grid is not the sentinel.
    for hex in [
        "1b5d323b747407",
        "1b5d323b74",
        "1b5d313b7407",
        "01b5d323b7407",
    ] {
        assert_eq!(
            rules(json!([output(hex), wait])),
            vec![(1, "R-57")],
            "{hex}"
        );
    }
}

/// A program can print its own sentinel (steward ruling R-88 (E), `a5_2` s79): the last `print` or `print_after_input` of the
/// session's program, if it ends with the title. Its step is the step at which that print runs: the session's `Start` for a
/// `print`, and the host input that completes its `match_hex` for a `print_after_input`. A launch after that step breaks the
/// fence; a pty_output of the session after that step is output that the sentinel does not cover; a program of another
/// session, or a print of another title, is no sentinel.
#[test]
fn a_program_that_prints_the_title_is_the_sentinel_at_the_input_that_triggers_it() {
    let program = |session: &str, items: Value| json!({"begin": {"op": {"Create": {"session": session, "request": {"program": items}}}}});
    let after =
        |m: &str, bytes: &str| json!({"print_after_input": {"match_hex": m, "bytes_hex": bytes}});
    let create =
        |session: &str, bytes: &str| program(session, json!([after("31", bytes), {"hold": {}}]));
    let start = |session: &str| json!({"begin": {"op": {"Start": {"id": session}}}});
    let input = |hex: &str| json!({"begin": {"op": {"WriteInput": {"session": "s1", "payload": {"bytes": {"bytes": {"$bytes_hex": hex}}}}}}});
    let printed = format!("62{}1b5c", sentinel_hex("t"));
    let wait = json!({"await_title": {"session": "s1", "title": "t"}});
    assert_eq!(
        rules(json!([
            create("s1", &printed),
            start("s1"),
            input("31"),
            wait
        ])),
        vec![]
    );
    // A `print` runs at the session's start; the input that completes a match can come in parts.
    let plain = program(
        "s1",
        json!([{"print": {"bytes_hex": printed}}, {"hold": {}}]),
    );
    assert_eq!(rules(json!([plain, start("s1"), wait])), vec![]);
    let split = program("s1", json!([after("3132", &printed), {"hold": {}}]));
    assert_eq!(
        rules(json!([split, start("s1"), input("31"), input("32"), wait])),
        vec![]
    );
    // A launch after the input that runs the print falls between the sentinel and the wait.
    assert_eq!(
        rules(json!([
            create("s1", &printed),
            start("s1"),
            input("31"),
            start("s2"),
            wait
        ])),
        vec![(4, "R-57")]
    );
    // An input that does not complete the match runs no print, and an input of unknown bytes places no print.
    assert_eq!(
        rules(json!([
            create("s1", &printed),
            start("s1"),
            input("32"),
            wait
        ])),
        vec![(3, "R-57")]
    );
    let key = json!({"begin": {"op": {"WriteInput": {"session": "s1", "payload": {"key": {"key": {"char": "1"}, "mods": []}}}}}});
    assert_eq!(
        rules(json!([create("s1", &printed), start("s1"), key, wait])),
        vec![(3, "R-57")]
    );
    // 016c on #75: the title that input 31 printed cannot fence a pty_output after it, even after a later, unrelated input.
    let out = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "6162"}});
    let found = rules(json!([
        create("s1", &printed),
        start("s1"),
        input("31"),
        wait,
        out,
        input("32"),
        wait
    ]));
    assert!(found.contains(&(6, "R-57")), "{found:?}");
    // The program of another session, or a print of another title, is no sentinel of s1.
    assert_eq!(
        rules(json!([create("s2", &printed), start("s2"), wait])),
        vec![(2, "R-57")]
    );
    let other = format!("62{}1b5c", sentinel_hex("u"));
    assert_eq!(
        rules(json!([
            create("s1", &other),
            start("s1"),
            input("31"),
            wait
        ])),
        vec![(3, "R-57")]
    );
    // The title must come after all the covered output, in the same stream (the steward on #75): a title with output after
    // it in the same print, or a title in an earlier print than the program's last, is no sentinel.
    let early = format!("{}1b5c62", sentinel_hex("t"));
    assert_eq!(
        rules(json!([
            create("s1", &early),
            start("s1"),
            input("31"),
            wait
        ])),
        vec![(3, "R-57")]
    );
    // 016c on #75: the title, more output, then another title ends the print. The other title ends the print, and the title
    // that the wait names does not.
    let then_other = format!("{}1b5c6162{}1b5c", sentinel_hex("t"), sentinel_hex("u"));
    assert_eq!(
        rules(json!([
            create("s1", &then_other),
            start("s1"),
            input("31"),
            wait
        ])),
        vec![(3, "R-57")]
    );
    let before_last = program(
        "s1",
        json!([after("31", &printed), after("32", "62"), {"hold": {}}]),
    );
    assert_eq!(
        rules(json!([before_last, start("s1"), input("31"), wait])),
        vec![(3, "R-57")]
    );
}

#[test]
fn the_output_that_an_oracle_resume_reads_has_no_0a() {
    // An `oracle_resume` reads every earlier `pty_output` of its session: none may hold a `0a` (R-62 Q3).
    let resume = |s: &str| json!({"control": {"op": "oracle_resume", "session": s, "capture": "$cap"}, "expect": {"equal": true}});
    let out = |s: &str, hex: &str| json!({"control": {"op": "pty_output", "session": s, "bytes_hex": hex}});
    // The R-60 findings only: the resume also reads the output without a title fence (the R-57 sweep's test).
    let rules = |steps: Value| -> Vec<(usize, &'static str)> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == "R-60")
            .collect()
    };
    assert_eq!(
        rules(json!([out("s1", "410a"), out("s1", "0a"), resume("s1")])),
        vec![(0, "R-60"), (1, "R-60")]
    );
    // Once per output, however many resumes read it.
    assert_eq!(
        rules(json!([out("s1", "0a"), resume("s1"), resume("s1")])),
        vec![(0, "R-60")]
    );
    // Not another session's output, not an output after the resume, not a `0a` across two bytes, not without a resume.
    assert_eq!(rules(json!([out("s2", "0a"), resume("s1")])), vec![]);
    assert_eq!(rules(json!([resume("s1"), out("s1", "0a")])), vec![]);
    assert_eq!(rules(json!([out("s1", "a0a1"), resume("s1")])), vec![]);
    assert_eq!(rules(json!([out("s1", "1b44")])), vec![]);
}

#[test]
fn a_computed_output_assertion_needs_program_ready() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1"}});
    let filled = json!({"route_expect_output": {"parts": [{"fill": "$f"}]}});
    let frame = json!({"expect": {"frame": "output", "payload": "$bytes"}, "on": "r1"});
    let any = json!({"expect": {"frame": "output", "payload": {"$any": true}}, "on": "r1"});
    for assertion in [filled, frame] {
        assert_eq!(
            rules(json!([attach, write("s1"), assertion])),
            vec![(1, NO_PROGRAM_READY)]
        );
        assert_eq!(
            rules(json!([attach, ready("s1"), write("s1"), assertion])),
            vec![]
        );
    }
    // A frame matched only by `$any` asserts no bytes.
    assert_eq!(rules(json!([attach, write("s1"), any])), vec![]);
}

#[test]
fn a_barrier_is_linted_as_its_own_step_before_the_wait() {
    // The barrier runs first, at the step's index: its read needs a fence, and its write clears one.
    let quiet = |barrier: Value| json!({"expect_quiet_until": {"$any": true}, "barrier": barrier, "until": {"$any": true}});
    let literal = read("s1", json!({"$bytes_hex": "61"}));
    assert_eq!(
        rules(json!([ready("s1"), write("s1"), quiet(literal.clone())])),
        vec![(2, "R-56")]
    );
    assert_eq!(
        rules(json!([
            ready("s1"),
            write("s1"),
            fence("s1", 1),
            quiet(literal)
        ])),
        vec![]
    );
    // The input rule applies inside a barrier too.
    let cr = read("s1", json!({"$bytes_hex": "0d"}));
    assert!(rules(json!([ready("s1"), quiet(cr)])).contains(&(1, "R-61")));
}

#[test]
fn hex_digits_compare_without_case() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1"}});
    assert_eq!(
        rules(json!([attach, {"route_expect_output": {"bytes_hex": "0A"}}])),
        vec![(1, "R-60")]
    );
    assert_eq!(
        rules(json!([read("s1", json!({"$bytes_hex": "0D"}))])),
        vec![(0, "R-61"), (0, "R-56")]
    );
    // A terminal query in upper case is a write; a title sentinel in upper case is a sentinel.
    let query = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "1B5B63"}});
    assert_eq!(
        rules(json!([
            let_n(),
            ready("s1"),
            bound_fence("s1"),
            query,
            read("s1", "$log")
        ])),
        vec![(4, UNFENCED_CAPTURE)]
    );
    let sentinel =
        json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "1B5D323B741B5C"}});
    assert_eq!(
        rules(json!([sentinel, {"await_title": {"session": "s1", "title": "t"}}])),
        vec![]
    );
}

#[test]
fn a_capture_needs_a_let_bound_threshold_that_is_not_zero() {
    let bind =
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$log"}});
    let wait = |min: Value| json!({"await_pty": {"session": "s1", "min_bytes": min}});
    // Not a bound number: the driver reads its default, and the lint keeps the finding.
    for min in [
        json!(null),
        json!(false),
        json!("not-a-variable"),
        json!("$unbound"),
        json!({"add": [1, 1]}),
    ] {
        assert_eq!(
            rules(json!([
                ready("s1"),
                write("s1"),
                wait(min.clone()),
                bind.clone()
            ])),
            vec![(3, UNFENCED_CAPTURE)],
            "{min}"
        );
    }
    // The steward's classes (#39 R1-2): a bound value fixed at 1 or more, or a run-time numeric value, fences a capture; a
    // fixed 0, a negative, a non-number, a value with the run's sign and an unmodelled operation do not. Each value is what the
    // driver's `let` computes, with its hex forms.
    for (expr, finding) in [
        (json!({"add": [0, 0]}), true),
        (json!({"sub": [3, 3]}), true),
        (json!({"sub": [0, 1]}), true),
        (json!({"eq": [0, 0]}), true),
        (json!({"gt": [2, 1]}), true),
        (json!({"hex_len": ["", 0]}), true),
        (json!({"hex_len": [{"$bytes_hex": ""}, 0]}), true),
        (json!({"str_len": ["", 0]}), true),
        (json!({"sum_hex_bytes": [["", ""]]}), true),
        (json!({"field": ["$rec", "size"]}), true),
        (json!({"sub": ["$reply", 1]}), true),
        (json!({"add": ["$unbound", 1]}), true),
        (json!({"add": [0, 1]}), false),
        (json!({"sub": [5, 1]}), false),
        (json!({"hex_len": ["616263", 0]}), false),
        (json!({"hex_len": [{"$bytes_hex": "61"}, 0]}), false),
        (json!({"utf8_len": ["é"]}), false),
        (
            json!({"sum_hex_bytes": [["61", {"$bytes_hex": "6263"}]]}),
            false,
        ),
        (json!({"hex_len": ["$reply", 0]}), false),
        (json!({"str_len": ["$text", 0]}), false),
    ] {
        let steps = json!([{"let": {"$n": expr}}, ready("s1"), write("s1"), wait(json!("$n")), bind.clone()]);
        let expected = if finding {
            vec![(4, UNFENCED_CAPTURE)]
        } else {
            vec![]
        };
        assert_eq!(rules(steps), expected, "{expr}");
    }
    // A chain: a run-time length plus a non-negative number is a run-time length; plus a negative one, its sign is the run's.
    // A length of a `let` value built from literals is fixed by the transcript; the lint does not compute it, so no fence.
    let empty = json!({"let": {"$e": {"hex_concat": ["", ""]}}});
    let steps = json!([empty, {"let": {"$n": {"hex_len": ["$e", 0]}}}, ready("s1"), write("s1"), wait(json!("$n")), bind.clone()]);
    assert_eq!(rules(steps), vec![(5, UNFENCED_CAPTURE)]);
    let len = json!({"let": {"$a": {"hex_len": ["$reply", 0]}}});
    for (expr, findings) in [
        (json!({"add": ["$a", 8]}), vec![]),
        (json!({"add": ["$a", "$a"]}), vec![]),
        (json!({"add": ["$a", -1]}), vec![(5, UNFENCED_CAPTURE)]),
    ] {
        let steps = json!([len, {"let": {"$n": expr}}, ready("s1"), write("s1"), wait(json!("$n")), bind.clone()]);
        assert_eq!(rules(steps), findings, "{expr}");
    }
    // A bound value also fences a literal log when the transcript fixes it at the log's length or more; a run-time value
    // cannot be compared with a literal length, so it does not.
    let lit = read("s1", json!({"$bytes_hex": "616263"}));
    for (expr, findings) in [
        (json!({"add": [3, 0]}), vec![]),
        (json!({"add": [2, 0]}), vec![(4, "R-56")]),
        (json!({"hex_len": ["$reply", 0]}), vec![(4, "R-56")]),
    ] {
        let steps = json!([{"let": {"$n": expr}}, ready("s1"), write("s1"), wait(json!("$n")), lit.clone()]);
        assert_eq!(rules(steps), findings, "{expr}");
    }
}

#[test]
fn a_variable_that_another_binding_replaces_loses_its_let_class() {
    let capture =
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$log"}});
    let wait = json!({"await_pty": {"session": "s1", "min_bytes": "$n"}});
    let n = json!({"let": {"$n": {"add": [1, 0]}}});
    // The reviewer's case: a control's `bind_fields` replaces `$n` with a boolean before the fence.
    let ready_bound = json!({"await_control": {"control": {"op": "program_ready", "session": "s1"}, "expect": {"ready": true}}, "bind_fields": {"ready": "$n"}});
    assert_eq!(
        rules(json!([n, ready_bound, write("s1"), wait, capture])),
        vec![(4, UNFENCED_CAPTURE)]
    );
    // `bind`, with or without the `$` (`route_send`'s too), and an `attach_route` named `n` replace it too.
    for rebind in [
        json!({"pump": {}, "bind": "$n"}),
        json!({"collect_events": {}, "bind": "n"}),
        json!({"route_send": {"op": "ping"}, "route": "r2", "bind": "$n"}),
        json!({"control": {"op": "worker_alive", "session": "s1"}, "bind_fields": {"alive": "n"}}),
        json!({"attach_route": {"route": "n", "session": "s2"}}),
    ] {
        assert_eq!(
            rules(json!([n, rebind, ready("s1"), write("s1"), wait, capture])),
            vec![(5, UNFENCED_CAPTURE)],
            "{rebind}"
        );
    }
    // An `attach_route` without a string `route` binds `r1`, as the driver's `attach` reads it: `r1` loses its class, `n` keeps it.
    let wait_r1 = json!({"await_pty": {"session": "s1", "min_bytes": "$r1"}});
    let r1 = json!({"let": {"$r1": {"add": [1, 0]}}});
    for attach in [
        json!({"attach_route": {"session": "s2"}}),
        json!({"attach_route": {"route": 7, "session": "s2"}}),
    ] {
        assert_eq!(
            rules(json!([
                r1,
                attach,
                ready("s1"),
                write("s1"),
                wait_r1,
                capture
            ])),
            vec![(5, UNFENCED_CAPTURE)],
            "{attach}"
        );
        assert_eq!(
            rules(json!([n, attach, ready("s1"), write("s1"), wait, capture])),
            vec![],
            "{attach}"
        );
    }
    // A variable route binds the route's id under the variable's run-time value, which the lint does not know: every
    // variable loses its class, and a `let` after it gives one again.
    let routed = json!({"attach_route": {"route": "$name", "session": "s2"}});
    assert_eq!(
        rules(json!([n, routed, ready("s1"), write("s1"), wait, capture])),
        vec![(5, UNFENCED_CAPTURE)]
    );
    assert_eq!(
        rules(json!([routed, n, ready("s1"), write("s1"), wait, capture])),
        vec![]
    );
    // A `let` after the other binding gives the variable a class again; a binding of another variable changes nothing.
    let other = json!({"pump": {}, "bind": "$m"});
    assert_eq!(
        rules(json!([
            json!({"pump": {}, "bind": "$n"}),
            n,
            ready("s1"),
            write("s1"),
            wait,
            capture
        ])),
        vec![]
    );
    assert_eq!(
        rules(json!([n, other, ready("s1"), write("s1"), wait, capture])),
        vec![]
    );
}

/// R-74 (the lint case): the held write reads its log after `await_pty` for the kernel's count, which the completion of its
/// `WriteInput` binds. The count is bound in an earlier step, by the write of the same session, at `pty_bytes_written`, or at
/// `payload_bytes_written` only for a raw `bytes` payload.
#[test]
fn a_written_count_that_a_pump_binds_is_a_run_time_threshold() {
    let capture =
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$log"}});
    let wait = |var: &str| json!({"await_pty": {"session": "s1", "min_bytes": var}});
    let raw = json!({"bytes": {"bytes": {"$bytes_hex": "61"}}});
    let begin = |session: &str, var: &str, payload: &Value| json!({"begin": {"op": {"WriteInput": {"session": session, "payload": payload}}}, "bind": var});
    let done = |op: &str, field: &str, var: &str| json!({"Completed": {"op": op, "result": {"ok": {"input": {"outcome": "Partial", field: var}}}}});
    let until =
        |op: &str, field: &str, var: &str| json!({"pump_until": {"event": done(op, field, var)}});
    for field in ["payload_bytes_written", "pty_bytes_written"] {
        for pump in [
            until("$w", field, "$p"),
            json!({"pump_collect": {"events": [done("$v", field, "$q"), done("$w", field, "$p")]}}),
        ] {
            let w = begin("s1", "$w", &raw);
            assert_eq!(
                rules(json!([ready("s1"), w, pump, wait("$p"), capture])),
                vec![],
                "{pump}"
            );
            // A sum with it is a count of the same session (the am_2 form: the count and the 1-byte write behind it).
            let n = json!({"let": {"$n": {"add": ["$p", 1]}}});
            assert_eq!(
                rules(json!([ready("s1"), w, pump, n, wait("$n"), capture])),
                vec![],
                "{pump}"
            );
        }
    }
    let w = begin("s1", "$w", &raw);
    // (1) Bound in an earlier step: an `await_pty` before the pump that binds the count fences nothing.
    assert_eq!(
        rules(json!([
            ready("s1"),
            w,
            wait("$p"),
            until("$w", "pty_bytes_written", "$p"),
            capture
        ])),
        vec![(4, UNFENCED_CAPTURE)]
    );
    // (2) The same session: the count of s2's write, a sum with it, or an op that is no `WriteInput`, fences no log of s1.
    let v = begin("s2", "$v", &raw);
    let start = json!({"begin": {"op": {"Start": {"id": "s1"}}}, "bind": "$s"});
    assert_eq!(
        rules(json!([
            ready("s1"),
            w,
            v,
            until("$v", "pty_bytes_written", "$p"),
            wait("$p"),
            capture
        ])),
        vec![(5, UNFENCED_CAPTURE)]
    );
    let both = json!({"pump_collect": {"events": [done("$w", "pty_bytes_written", "$p"), done("$v", "pty_bytes_written", "$q")]}});
    let sum = json!({"let": {"$n": {"add": ["$p", "$q"]}}});
    assert_eq!(
        rules(json!([ready("s1"), w, v, both, sum, wait("$n"), capture])),
        vec![(6, UNFENCED_CAPTURE)]
    );
    assert_eq!(
        rules(json!([
            ready("s1"),
            start,
            w,
            until("$s", "pty_bytes_written", "$p"),
            wait("$p"),
            capture
        ])),
        vec![(5, UNFENCED_CAPTURE)]
    );
    // (2) An op variable that any other form binds again is no longer its old `WriteInput` (#52 016c R1-1): `let $w = $v + 0`
    // names s2's write at run time, so a `Completed` for `$w` is no count of s1. Each replacement form drops it.
    for rebind in [
        json!({"let": {"$w": {"add": ["$v", 0]}}}),
        json!({"begin": {"op": {"Start": {"id": "s2"}}}, "bind": "$w"}),
        json!({"call": {"method": "list"}, "bind_fields": {"ids": "$w"}}),
        json!({"attach_route": {"route": "$r", "session": "s2"}}),
    ] {
        assert_eq!(
            rules(json!([
                ready("s1"),
                w,
                v,
                rebind,
                until("$w", "pty_bytes_written", "$p"),
                wait("$p"),
                capture
            ])),
            vec![(6, UNFENCED_CAPTURE)],
            "{rebind}"
        );
    }
    // Only a new `WriteInput` binding installs the variable again, with its own session.
    assert_eq!(
        rules(json!([
            ready("s1"),
            begin("s2", "$w", &raw),
            w,
            until("$w", "pty_bytes_written", "$p"),
            wait("$p"),
            capture
        ])),
        vec![]
    );
    assert_eq!(
        rules(json!([
            ready("s1"),
            w,
            begin("s2", "$w", &raw),
            until("$w", "pty_bytes_written", "$p"),
            wait("$p"),
            capture
        ])),
        vec![(5, UNFENCED_CAPTURE)]
    );
    // (3) An encoded payload: its payload count is not its PTY count, so only `pty_bytes_written` counts.
    for encoded in [
        json!({"text": {"text": "a"}}),
        json!({"paste": {"bytes": {"$bytes_hex": "61"}, "require_bracketed": false}}),
    ] {
        let e = begin("s1", "$w", &encoded);
        assert_eq!(
            rules(json!([
                ready("s1"),
                e,
                until("$w", "payload_bytes_written", "$p"),
                wait("$p"),
                capture
            ])),
            vec![(4, UNFENCED_CAPTURE)],
            "{encoded}"
        );
        assert_eq!(
            rules(json!([
                ready("s1"),
                e,
                until("$w", "pty_bytes_written", "$p"),
                wait("$p"),
                capture
            ])),
            vec![],
            "{encoded}"
        );
    }
    // Another field, or a binding outside a pump, is no written count.
    let called = json!({"call": {"method": "cancel", "args": {"op": "$w"}}, "expect": {"pty_bytes_written": "$p"}});
    for other in [until("$w", "at", "$p"), called] {
        assert_eq!(
            rules(json!([ready("s1"), w, other, wait("$p"), capture])),
            vec![(4, UNFENCED_CAPTURE)],
            "{other}"
        );
    }
    // A variable that a `let` already bound is compared, not bound: it keeps its class (here 0, which fences nothing).
    let zero = json!({"let": {"$p": {"add": [0, 0]}}});
    assert_eq!(
        rules(json!([
            zero,
            ready("s1"),
            w,
            until("$w", "payload_bytes_written", "$p"),
            wait("$p"),
            capture
        ])),
        vec![(5, UNFENCED_CAPTURE)]
    );
}

#[test]
fn a_threshold_reads_references_numbers_and_hex_lists_as_the_driver_does() {
    let capture =
        json!({"control": {"op": "pty_input", "session": "s1"}, "bind_fields": {"bytes": "$log"}});
    let fenced = |lets: Vec<Value>, min: Value| {
        let mut steps = lets;
        steps.extend([
            ready("s1"),
            write("s1"),
            json!({"await_pty": {"session": "s1", "min_bytes": min}}),
            capture.clone(),
        ]);
        rules(Value::Array(steps)).is_empty()
    };
    let n = |expr: Value| json!({"let": {"$n": expr}});
    // A reference is a `matcher::is_variable` string: a plain name is no `min_bytes` and no operand.
    assert!(fenced(vec![n(json!({"add": [1, 0]}))], json!("$n")));
    assert!(!fenced(vec![n(json!({"add": [1, 0]}))], json!("n")));
    let one = json!({"let": {"$one": {"add": [1, 0]}}});
    assert!(fenced(
        vec![one.clone(), n(json!({"add": ["$one", 0]}))],
        json!("$n")
    ));
    assert!(!fenced(
        vec![one, n(json!({"add": ["one", 0]}))],
        json!("$n")
    ));
    // The driver binds an `add` or `sub` result that fits i64 or u64, and reads each operand as an i64 or a u64.
    for (expr, fences) in [
        (json!({"add": [9_223_372_036_854_775_807_i64, 1]}), true),
        (json!({"add": [18_446_744_073_709_551_615_u64, 0]}), true),
        (
            json!({"sub": [18_446_744_073_709_551_615_u64, 18_446_744_073_709_551_614_u64]}),
            true,
        ),
        (json!({"add": [18_446_744_073_709_551_615_u64, 1]}), false),
        (json!({"sub": [0, 18_446_744_073_709_551_615_u64]}), false),
        (json!({"add": [1.5, 1]}), false),
    ] {
        assert_eq!(fenced(vec![n(expr.clone())], json!("$n")), fences, "{expr}");
    }
    // `sum_hex_bytes`: a literal item that `hex_of` does not read gives 0; a run-time item gives a run-time sum; an item from
    // a `let` built only from literals is not computed, so no fence.
    for (lets, fences) in [
        (
            vec![n(json!({"sum_hex_bytes": [["61", 5, {"x": 1}]]}))],
            true,
        ),
        (vec![n(json!({"sum_hex_bytes": [[5]]}))], false),
        (vec![n(json!({"sum_hex_bytes": [["61", "$v"]]}))], true),
        (vec![n(json!({"sum_hex_bytes": ["$list"]}))], true),
        (vec![n(json!({"sum_hex_bytes": ["61"]}))], false),
        (
            vec![
                json!({"let": {"$v": {"hex_concat": ["61", ""]}}}),
                n(json!({"sum_hex_bytes": [["61", "$v"]]})),
            ],
            false,
        ),
    ] {
        assert_eq!(fenced(lets.clone(), json!("$n")), fences, "{lets:?}");
    }
    // Only the string that `hex_of` reads counts: a variable in another field of an item or of the value is not read
    // (#41 R1-1).
    for (expr, fences) in [
        (json!({"sum_hex_bytes": [[{"ignored": "$v"}]]}), false),
        (json!({"sum_hex_bytes": [[{"ignored": "$v"}, "61"]]}), true),
        (
            json!({"sum_hex_bytes": [[{"$bytes_hex": "$v", "x": 1}]]}),
            true,
        ),
        (
            json!({"hex_len": [{"$bytes_hex": "61", "ignored": "$v"}, 0]}),
            true,
        ),
        (
            json!({"hex_len": [{"$bytes_hex": "", "ignored": "$v"}, 0]}),
            false,
        ),
    ] {
        assert_eq!(fenced(vec![n(expr.clone())], json!("$n")), fences, "{expr}");
    }
    // A computed `Fixed` value stays fixed in a chain, whatever field the driver ignored to compute it; and `hex_of` never
    // reads a `let` number, boolean or list, so a `sum_hex_bytes` item that names one gives 0, even when the run decides its
    // value (#41 R2-1). A `let` string that the run decides is still a run-time length.
    let bind_v =
        json!({"control": {"op": "worker_alive", "session": "s1"}, "bind_fields": {"alive": "$v"}});
    let z = |expr: Value| json!({"let": {"$z": expr}});
    for (lets, fences) in [
        (
            vec![
                bind_v.clone(),
                z(json!({"hex_len": [{"$bytes_hex": "", "ignored": "$v"}, 0]})),
                n(json!({"sum_hex_bytes": [["$z"]]})),
            ],
            false,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"hex_len": [{"$bytes_hex": "", "ignored": "$v"}, 0]})),
                json!({"let": {"$w": {"str": ["$z", "$z"]}}}),
                n(json!({"sum_hex_bytes": [["$w"]]})),
            ],
            false,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"eq": ["$v", 1]})),
                n(json!({"sum_hex_bytes": [["$z", {"$bytes_hex": "$z"}]]})),
            ],
            false,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"record_payloads": ["$v"]})),
                n(json!({"sum_hex_bytes": [["$z"]]})),
            ],
            false,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"hex_len": ["$v", 0]})),
                n(json!({"hex_len": ["$z", 0]})),
            ],
            false,
        ),
        // A binding by another form forgets that the value was a `let` number.
        (
            vec![
                bind_v.clone(),
                z(json!({"hex_len": ["$v", 0]})),
                json!({"control": {"op": "worker_alive", "session": "s1"}, "bind_fields": {"alive": "$z"}}),
                n(json!({"sum_hex_bytes": [["$z"]]})),
            ],
            true,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"concat": ["$v", ""]})),
                n(json!({"sum_hex_bytes": [["$z"]]})),
            ],
            true,
        ),
        (
            vec![
                bind_v.clone(),
                z(json!({"hex_len": ["$v", 0]})),
                n(json!({"add": ["$z", 1]})),
            ],
            true,
        ),
    ] {
        assert_eq!(fenced(lets.clone(), json!("$n")), fences, "{lets:?}");
    }
    // A later `let` replaces a variable's fixedness: a run-time value after a literal one is a run-time value, and a literal one
    // after a run-time one is fixed (#41 R1-2).
    let literal_v = json!({"let": {"$v": {"hex_concat": ["", ""]}}});
    let run_v = json!({"let": {"$v": {"hex_concat": ["$reply", ""]}}});
    let len = n(json!({"hex_len": ["$v", 0]}));
    assert!(fenced(
        vec![literal_v.clone(), run_v.clone(), len.clone()],
        json!("$n")
    ));
    assert!(!fenced(vec![run_v, literal_v, len], json!("$n")));
    // A plain string stays a literal input to a length; the driver's arity holds; a length of a non-string is refused.
    for (expr, fences) in [
        (json!({"hex_len": ["n1", 0]}), true),
        (json!({"str_len": ["n", 0]}), true),
        (json!({"hex_len": ["61"]}), false),
        (json!({"utf8_len": ["a", 0]}), false),
        (json!({"str_len": [5, 0]}), false),
        (json!({"hex_len": [{"x": "$v"}, 0]}), false),
        (json!({"hex_len": [{"$bytes_hex": "$v"}, 0]}), true),
    ] {
        assert_eq!(fenced(vec![n(expr.clone())], json!("$n")), fences, "{expr}");
    }
    // A fixed value from a list with a non-hex item fences a literal log of its length.
    let lit = read("s1", json!({"$bytes_hex": "61"}));
    let steps = json!([n(json!({"sum_hex_bytes": [["61", 5]]})), ready("s1"), write("s1"), {"await_pty": {"session": "s1", "min_bytes": "$n"}}, lit]);
    assert_eq!(rules(steps), vec![]);
}

#[test]
fn an_output_that_a_read_meets_before_a_wait_needs_a_title_fence() {
    // The sweep's findings only.
    let sweep = |steps: Value| -> Vec<usize> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == NO_AWAIT_TITLE)
            .map(|(at, _)| at)
            .collect()
    };
    let out = |s: &str| json!({"control": {"op": "pty_output", "session": s, "bytes_hex": "41"}});
    let attach = json!({"attach_route": {"route": "r1", "session": "s1"}});
    // A read that does not wait (shape 1), and a wait for an `EventsLost` marker that the output extends (shape 2).
    for read in [
        json!({"drain": {}}),
        json!({"forbid_events": {"events": [{"Bell": {"id": "s1"}}]}}),
        json!({"poll_events": {"max": 64}, "expect": []}),
        json!({"collect_events": {}, "bind": "$e"}),
        json!({"poll_all": {}}),
        json!({"poll_find": {"event": {"Bell": {"id": "s1"}}}}),
        json!({"expect_quiet_until": {"Bell": {"id": "s1"}}, "barrier": {"pump": {}}, "until": {"$any": true}}),
        json!({"expect": {"Bell": {"id": "s1"}}}),
        json!({"pump": {}, "expect": {"events_posted": 0}}),
        json!({"tap_drain": {"session": "s1"}, "bind": "$t"}),
        json!({"call": {"method": "terminal_state", "args": {"id": "s1"}}, "expect": {"$any": true}}),
        json!({"call": {"method": "list"}, "expect": {"$any": true}}),
        json!({"control": {"op": "oracle_state", "session": "s1"}, "bind": "$o"}),
        json!({"control": {"op": "oracle_screen", "session": "s1", "history": false}, "expect": {"$any": true}}),
        json!({"pump_until": {"event": {"EventsLost": {"kinds": ["Bell"]}}}}),
        json!({"expect_eventually": {"EventsLost": {"kinds": ["Bell"]}}}),
    ] {
        assert_eq!(
            sweep(json!([out("s1"), {"await_quiet": {}}, read])),
            vec![0],
            "{read}"
        );
    }
    // A wait for a new value, which only the output can produce, is its own fence.
    for wait in [
        json!({"pump_until": {"event": {"Bell": {"id": "s1"}}}}),
        json!({"pump_collect": {"events": [{"Bell": {"id": "s1"}}]}}),
        json!({"expect_eventually": {"Bell": {"id": "s1"}}}),
        json!({"route_expect_output": {"bytes_hex": "41"}}),
        json!({"expect": {"frame": "modes"}, "on": "r1"}),
        json!({"expect_set": [{"frame": "modes"}], "on": "r1"}),
        json!({"await_get": {"id": "s1", "expect": {"$any": true}}}),
        json!({"await_control": {"control": {"op": "oracle_state", "session": "s1"}, "expect": {"$any": true}}}),
        json!({"tap_drain": {"session": "s1", "accounted": 1}, "bind": "$t"}),
    ] {
        assert_eq!(
            sweep(json!([attach, out("s1"), wait, {"drain": {}}])),
            Vec::<usize>::new(),
            "{wait}"
        );
    }
    // Not a read: a control that observes nothing, the input side (R-56's), and the steps that read nothing. The next read
    // decides: here a wait, so no finding.
    for skip in [
        json!({"control": {"op": "pty_accept", "session": "s1", "bytes": 1}}),
        json!({"control": {"op": "control_queue", "session": "s1", "on": true}}),
        json!({"control": {"op": "pty_input", "session": "s1"}, "expect": {"bytes": {"$bytes_hex": ""}}}),
        json!({"await_quiet": {}}),
        json!({"pump": {}}),
        json!({"call": {"method": "features"}, "expect": {"$any": true}}),
    ] {
        assert_eq!(
            sweep(json!([out("s1"), skip, {"pump_until": {"event": {"$any": true}}}])),
            Vec::<usize>::new(),
            "{skip}"
        );
    }
    // An `await_title` of the session fences it.
    let sentinel = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": format!("41{}07", sentinel_hex("T1"))}});
    let title = json!({"await_title": {"session": "s1", "title": "T1"}});
    assert_eq!(
        sweep(json!([sentinel, title, {"drain": {}}])),
        Vec::<usize>::new()
    );
    // Only the last output before the read needs the fence; a read meets each output once.
    assert_eq!(sweep(json!([out("s1"), out("s1"), {"drain": {}}])), vec![1]);
    assert_eq!(
        sweep(json!([out("s1"), {"drain": {}}, {"drain": {}}])),
        vec![0]
    );
    assert_eq!(
        sweep(json!([out("s1"), {"pump_until": {"event": {"$any": true}}}, {"drain": {}}])),
        Vec::<usize>::new()
    );
    // A read of another session, or of a route that is not attached, does not read this output.
    let get =
        |s: &str| json!({"call": {"method": "get", "args": {"id": s}}, "expect": {"$any": true}});
    assert_eq!(sweep(json!([out("s1"), get("s2")])), Vec::<usize>::new());
    assert_eq!(sweep(json!([out("s1"), get("s2"), get("s1")])), vec![0]);
    let quiet_on = |r: &str| json!({"expect_quiet_until": {"frame": "output"}, "barrier": {"pump": {}}, "until": {"$any": true}, "on": r});
    assert_eq!(
        sweep(json!([out("s1"), quiet_on("r9")])),
        Vec::<usize>::new()
    );
    assert_eq!(sweep(json!([attach, out("s1"), quiet_on("r1")])), vec![1]);
    // #42 R1-1: only a matcher that can end the wait makes it shape 2; an `EventsLost` in `ignore` or `forbid` does not.
    for wait in [
        json!({"pump_collect": {"events": [{"Bell": {"id": "s1"}}], "ignore": [{"EventsLost": {}}], "forbid": [{"EventsLost": {}}]}}),
        json!({"expect_eventually": {"Bell": {"id": "s1"}}, "ignore": [{"EventsLost": {}}]}),
    ] {
        assert_eq!(
            sweep(json!([out("s1"), wait])),
            Vec::<usize>::new(),
            "{wait}"
        );
    }
    assert_eq!(
        sweep(
            json!([out("s1"), {"pump_collect": {"events": [{"EventsLost": {}}], "ignore": [{"Bell": {}}]}}])
        ),
        vec![0]
    );
    // #42 R1-2: an output reaches the Core of its control's handle. A wait or an `await_title` on another handle does not
    // meet it; a read on its own handle does. A route read reaches its session on the handle that attached the route.
    let out_on = |h: &str| json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "41", "handle": h}});
    let get_on = |h: &str| json!({"call": {"method": "get", "args": {"id": "s1"}, "handle": h}, "expect": {"$any": true}});
    let wait_on = |h: &str| json!({"pump_until": {"event": {"$any": true}, "handle": h}});
    assert_eq!(
        sweep(json!([out_on("h1"), wait_on("h2"), get_on("h1")])),
        vec![0]
    );
    assert_eq!(
        sweep(json!([out_on("h1"), wait_on("h1"), get_on("h1")])),
        Vec::<usize>::new()
    );
    assert_eq!(
        sweep(json!([out_on("h1"), get_on("h2")])),
        Vec::<usize>::new()
    );
    assert_eq!(
        sweep(json!([out_on("h2"), {"drain": {}}])),
        Vec::<usize>::new()
    );
    assert_eq!(
        sweep(json!([out_on("h2"), {"drain": {"handle": "h2"}}])),
        vec![0]
    );
    let events_on = |h: &str| json!({"expect": {"Bell": {"id": "s1"}}, "on": {"handle": h}});
    assert_eq!(
        sweep(json!([out_on("h2"), events_on("h")])),
        Vec::<usize>::new()
    );
    assert_eq!(sweep(json!([out_on("h2"), events_on("h2")])), vec![0]);
    let sentinel_on = json!({"control": {"op": "pty_output", "session": "s1", "handle": "h1", "bytes_hex": format!("41{}07", sentinel_hex("T1"))}});
    let title_on = |h: &str| json!({"await_title": {"session": "s1", "title": "T1", "handle": h}});
    assert_eq!(
        sweep(json!([sentinel_on, title_on("h2"), get_on("h1")])),
        vec![0]
    );
    assert_eq!(
        sweep(json!([sentinel_on, title_on("h1"), get_on("h1")])),
        Vec::<usize>::new()
    );
    assert_eq!(
        sweep(json!([attach, out_on("h2"), quiet_on("r1")])),
        Vec::<usize>::new()
    );
    assert_eq!(sweep(json!([attach, out_on("h"), quiet_on("r1")])), vec![1]);
    // #42 R2-1: equal session names on two handles (two data directories). A route of h2's `s1` does not read h1's `s1`
    // output, so a later read on h1 still needs the fence; a route of h1's `s1` meets it.
    let open = |h: &str, d: &str| json!({"open": {"as": h, "data_dir": d}});
    let attach_on =
        |r: &str, h: &str| json!({"attach_route": {"route": r, "session": "s1", "handle": h}});
    let state_on = |h: &str| json!({"call": {"method": "terminal_state", "args": {"id": "s1"}, "handle": h}, "expect": {"$any": true}});
    let route_read = |r: &str| json!({"route_expect_output": {"bytes_hex": "41"}, "route": r});
    assert_eq!(
        sweep(json!([
            open("h1", "d1"),
            open("h2", "d2"),
            attach_on("r2", "h2"),
            out_on("h1"),
            route_read("r2"),
            state_on("h1")
        ])),
        vec![3]
    );
    assert_eq!(
        sweep(json!([
            open("h1", "d1"),
            open("h2", "d2"),
            attach_on("r1", "h1"),
            out_on("h1"),
            route_read("r1"),
            state_on("h1")
        ])),
        Vec::<usize>::new()
    );
    // #42 R1-3: a sync call names its session by `id`, or by `session` when `id` is absent, as the driver reads it.
    for method in [
        "get",
        "terminal_state",
        "read_page",
        "snapshot_formats",
        "terminal_identity",
    ] {
        let by_session = json!({"call": {"method": method, "args": {"session": "s1"}}, "expect": {"$any": true}});
        assert_eq!(sweep(json!([out("s1"), by_session])), vec![0], "{method}");
        let other = json!({"call": {"method": method, "args": {"id": "s2", "session": "s1"}}, "expect": {"$any": true}});
        assert_eq!(
            sweep(json!([out("s1"), other])),
            Vec::<usize>::new(),
            "{method}"
        );
    }
    // #42 R1-4: a setter is no read, even when its result is checked or bound.
    for setter in [
        json!({"control": {"op": "pty_accept", "session": "s1", "bytes": 1}, "expect": {"$any": true}}),
        json!({"control": {"op": "pty_accept", "session": "s1", "bytes": 1}, "bind": "$a"}),
    ] {
        assert_eq!(
            sweep(json!([out("s1"), setter, {"pump_until": {"event": {"$any": true}}}])),
            Vec::<usize>::new(),
            "{setter}"
        );
    }
    // A barrier's output is its own unit before its step, so the step reads it.
    let barrier = json!({"expect_quiet_until": {"Bell": {"id": "s1"}}, "barrier": out("s1"), "until": {"$any": true}});
    assert_eq!(sweep(json!([barrier])), vec![0]);
}

// R-73 item 5 with R-61 (c): `written_by_op` asserts the bytes that the op's frames sent, so they hold no `0d`, typed or raw. A
// key's bytes are no exact assertion, and another op's `0d` does not count. A paste body counts: the lint may flag a `0d` in
// any paste body that a relation part covers (R-73 follow-up, steward 2026-10-11 (017f F2 on #53)).
#[test]
fn the_sent_bytes_that_written_by_op_asserts_hold_no_0d() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let chunk = |op: &str, hex: &str| json!({"route_send": {"frame": "paste_chunk", "op": op, "bytes": {"$bytes_hex": hex}}, "route": "r1"});
    let by = |op: &str| json!({"route_expect_pty": [{"written_by_op": op}]});
    let r61 = |steps: Value| {
        rules(steps)
            .into_iter()
            .filter(|(_, rule)| *rule == "R-61")
            .collect::<Vec<_>>()
    };
    assert_eq!(
        r61(json!([attach, chunk("1", "410d"), by("1")])),
        vec![(2, "R-61")]
    );
    assert_eq!(r61(json!([attach, chunk("1", "410a"), by("1")])), vec![]);
    assert_eq!(
        r61(json!([attach, chunk("2", "0d"), chunk("1", "41"), by("1")])),
        vec![]
    );
    let text = json!({"route_send": {"frame": "text", "op": "3", "text": "a\r"}, "route": "r1"});
    assert_eq!(r61(json!([attach, text, by("3")])), vec![(2, "R-61")]);
    // `bytes` op 9, `0d`, as a raw frame: type 0x83, the op in 8 bytes, the payload.
    let raw = json!({"route_send_raw": {"bytes_hex": "8300000000000000090d"}, "route": "r1"});
    assert_eq!(r61(json!([attach, raw, by("9")])), vec![(2, "R-61")]);
}

// R-73 follow-up, steward 2026-10-11 (017f F2 on #53), extended: a `written_without_op` on a session flags a `0d` in any sent
// `bytes`, `text` or paste body of that session before it, with or without an op. Another session's `0d`, or a later one, does
// not count.
#[test]
fn a_without_op_part_flags_a_0d_in_any_sent_body_of_its_session() {
    let attach =
        |r: &str, s: &str| json!({"attach_route": {"route": r, "session": s, "client": "c1"}});
    let send = |r: &str, frame: Value| json!({"route_send": frame, "route": r});
    let bytes =
        |op: &str, hex: &str| json!({"frame": "bytes", "op": op, "bytes": {"$bytes_hex": hex}});
    let without = |r: &str| json!({"route_expect_pty": [{"written_without_op": true}], "route": r});
    let r61 = |steps: Value| {
        rules(steps)
            .into_iter()
            .filter(|(_, rule)| *rule == "R-61")
            .collect::<Vec<_>>()
    };
    let at = |i: usize| vec![(i, "R-61")];
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            send("r1", bytes("0", "410d")),
            without("r1")
        ])),
        at(2)
    );
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            send("r1", bytes("4", "0d")),
            without("r1")
        ])),
        at(2)
    );
    let text = json!({"frame": "text", "op": "0", "text": "a\r"});
    assert_eq!(
        r61(json!([attach("r1", "s1"), send("r1", text), without("r1")])),
        at(2)
    );
    let chunk = json!({"frame": "paste_chunk", "op": "2", "bytes": {"$bytes_hex": "0d"}});
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            send("r1", chunk),
            without("r1")
        ])),
        at(2)
    );
    // `bytes` op 0, `0d`, as a raw frame: type 0x83, the op in 8 bytes, the payload.
    let raw = json!({"route_send_raw": {"bytes_hex": "8300000000000000000d"}, "route": "r1"});
    assert_eq!(r61(json!([attach("r1", "s1"), raw, without("r1")])), at(2));
    // Another route of the same session counts; another session, a later send, or a body without `0d` does not.
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            attach("r2", "s1"),
            send("r2", bytes("0", "0d")),
            without("r1")
        ])),
        at(3)
    );
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            attach("r2", "s2"),
            send("r2", bytes("0", "0d")),
            without("r1")
        ])),
        vec![]
    );
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            without("r1"),
            send("r1", bytes("0", "0d"))
        ])),
        vec![]
    );
    assert_eq!(
        r61(json!([
            attach("r1", "s1"),
            send("r1", bytes("0", "410a")),
            without("r1")
        ])),
        vec![]
    );
}

#[test]
fn an_attach_after_an_unmet_output_needs_a_title_fence_when_its_exact_output_is_asserted() {
    // The attach rule's findings only.
    let attach_rule = |steps: Value| -> Vec<usize> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == UNFENCED_ATTACH)
            .map(|(at, _)| at)
            .collect()
    };
    let out_on = |s: &str, h: &str| json!({"control": {"op": "pty_output", "session": s, "bytes_hex": "41", "handle": h}});
    let out = |s: &str| out_on(s, "h");
    let attach_on =
        |r: &str, h: &str| json!({"attach_route": {"route": r, "session": "s1", "handle": h}});
    let attach = |r: &str| attach_on(r, "h");
    let exact = |r: &str| json!({"route_expect_output": {"bytes_hex": "42"}, "route": r});
    let quiet = json!({"await_quiet": {}});
    // ou_9's shape: the output, `await_quiet` (no fence), the attach, then an exact assertion of the route's live output.
    assert_eq!(
        attach_rule(json!([out("s1"), quiet, attach("r1"), exact("r1")])),
        vec![0]
    );
    // Each exact form of R-61 (c) on the route counts; a payload with no literal bytes does not.
    for read in [
        json!({"route_expect_output": {"parts": [{"bytes_hex": "42"}]}, "route": "r1"}),
        json!({"expect": {"frame": "output", "payload": {"$bytes_hex": "42"}}, "on": "r1"}),
        json!({"expect_eventually": {"frame": "output_deflate", "payload": {"$bytes_hex": "42"}}, "on": "r1"}),
        // #49 R1-1: a part with literal bytes counts beside a variable part.
        json!({"route_expect_output": {"parts": [{"bytes_hex": "$b"}, {"bytes_hex": "42"}]}, "route": "r1"}),
    ] {
        assert_eq!(
            attach_rule(json!([out("s1"), attach("r1"), read])),
            vec![0],
            "{read}"
        );
    }
    for read in [
        json!({"expect": {"frame": "output", "payload": {"$any": true}}, "on": "r1"}),
        json!({"expect": {"frame": "live"}, "on": "r1"}),
        exact("r2"),
        // #49 R1-1: a variable reference is bound at run time, so it is no literal byte, in each form.
        json!({"route_expect_output": {"bytes_hex": "$b"}, "route": "r1"}),
        json!({"route_expect_output": {"parts": [{"bytes_hex": "$b"}, {"var": "$c"}]}, "route": "r1"}),
        json!({"expect": {"frame": "output", "payload": {"$bytes_hex": "$b"}}, "on": "r1"}),
        json!({"expect_eventually": {"frame": "output_deflate", "payload": {"$bytes_hex": "$b"}}, "on": "r1"}),
    ] {
        assert_eq!(
            attach_rule(json!([out("s1"), attach("r1"), read])),
            Vec::<usize>::new(),
            "{read}"
        );
    }
    // An `await_title` of the session before the attach fences it.
    let sentinel = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": format!("41{}07", sentinel_hex("T1"))}});
    let title = json!({"await_title": {"session": "s1", "title": "T1"}});
    assert_eq!(
        attach_rule(json!([sentinel, quiet, title, attach("r1"), exact("r1")])),
        Vec::<usize>::new()
    );
    // A wait that only the output can end also meets it (R-61 (a)): `dp_3_output_deflate_frames_share_no_state`'s shape.
    assert_eq!(
        attach_rule(json!([
            attach("r1"),
            out("s1"),
            exact("r1"),
            attach("r2"),
            exact("r2")
        ])),
        Vec::<usize>::new()
    );
    // The output of another session does not race this attach.
    assert_eq!(
        attach_rule(json!([out("s2"), attach("r1"), exact("r1")])),
        Vec::<usize>::new()
    );
    // An output on any handle races the attach: `ou_9_adopted_session_attaches_by_baseline` attaches on the adopting
    // handle. A wait on the adopting handle does not meet the output of the dropped one.
    let adopt = json!({"pump_until": {"event": {"$any": true}, "handle": "h2"}});
    assert_eq!(
        attach_rule(
            json!([out("s1"), {"drop": {"handle": "h"}}, {"open": {"as": "h2"}}, adopt, attach_on("r1", "h2"), exact("r1")])
        ),
        vec![0]
    );
    // One finding per attach, at the last unmet output; a later attach of the route after the fence is clean.
    assert_eq!(
        attach_rule(json!([
            out("s1"),
            out("s1"),
            attach("r1"),
            exact("r1"),
            exact("r1")
        ])),
        vec![1]
    );
    assert_eq!(
        attach_rule(json!([
            out_on("s1", "h3"),
            out("s1"),
            attach("r1"),
            exact("r1")
        ])),
        vec![1]
    );
    assert_eq!(
        attach_rule(json!([
            out("s1"),
            attach("r1"),
            sentinel,
            title,
            attach("r1"),
            exact("r1")
        ])),
        Vec::<usize>::new()
    );
}

#[test]
fn a_model_read_after_an_output_of_its_session_needs_an_output_fence() {
    // The R-80 rule's findings only.
    let reads = |steps: Value| -> Vec<usize> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == UNFENCED_READ)
            .map(|(at, _)| at)
            .collect()
    };
    let out_op =
        |op: &str, s: &str| json!({"control": {"op": op, "session": s, "bytes_hex": "42"}});
    let out = |s: &str| out_op("pty_output", s);
    let activity = |s: &str| json!({"pump_until": {"event": {"Activity": {"id": s, "source": "Output", "at": 5001}}}});
    let state = |var: &str| json!({"call": {"method": "terminal_state", "args": {"id": "s1"}}, "expect": {"model_rev": var}});
    let screen = json!({"begin": {"op": {"ReadScreen": {"session": "s1", "history": false}}}, "bind": "$rs"});
    let reply = json!({"pump_until": {"event": {"Completed": {"op": "$rs", "result": {"ok": {"screen": {"model_rev": "$rev2"}}}}}}});
    let capture = json!({"begin": {"op": {"CaptureSnapshot": {"session": "s1", "owner": "c1"}}}, "bind": "$k"});
    let guarded = |var: &str| json!({"begin": {"op": {"WriteInput": {"session": "s1", "payload": {"bytes": {"bytes": {"$bytes_hex": "61"}}}, "guard": {"model_rev": var}}}}});
    let quiet = json!({"await_quiet": {}});
    // Today's st_6: an output, await_quiet (no fence), then ReadScreen and its own reply wait (no fence either).
    assert_eq!(reads(json!([out("s1"), quiet, screen, reply])), vec![2]);
    // Each model read, after each program output control.
    for read in [screen.clone(), capture.clone(), state("$m"), guarded("$m")] {
        for output in [
            out("s1"),
            out_op("pty_chunk", "s1"),
            out_op("program_write_once", "s1"),
        ] {
            assert_eq!(reads(json!([output, read])), vec![1], "{output} {read}");
        }
    }
    // R-80's output fences: the session's Activity of Output, in a pump_until, a pump_collect or an await_*, and an
    // await_title of the session.
    let sentinel = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": format!("42{}07", sentinel_hex("T1"))}});
    let title = json!({"await_title": {"session": "s1", "title": "T1"}});
    for (output, fence) in [
        (out("s1"), activity("s1")),
        (
            out("s1"),
            json!({"pump_collect": {"events": [{"Activity": {"id": "s1", "source": "Output"}}, {"Bell": {"id": "s1"}}]}}),
        ),
        (sentinel, title),
        // Every alternative of the `$one_of` requires an Activity of s1.
        (
            out("s1"),
            json!({"pump_until": {"event": {"$one_of": [{"Activity": {"id": "s1", "source": "Output", "at": 5000}}, {"Activity": {"id": "s1", "source": "Output", "at": 5001}}]}}}),
        ),
    ] {
        assert_eq!(
            reads(json!([output, fence, screen])),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    // Not fences: another session's Activity (016c #54 R1-2), another event of the session, an Activity in an `ignore`
    // matcher, await_quiet, drain, pump and the clock.
    for not_fence in [
        activity("s2"),
        json!({"pump_until": {"event": {"Bell": {"id": "s1"}}}}),
        // A `$one_of` that can end on an alternative with no Activity of s1 (016c #54 R2-1), and a `$not`.
        json!({"pump_until": {"event": {"$one_of": [{"Activity": {"id": "s1", "source": "Output"}}, {"Bell": {"id": "s1"}}]}}}),
        json!({"pump_until": {"event": {"$one_of": [{"Activity": {"id": "s1", "source": "Output"}}, {"Activity": {"id": "s2", "source": "Output"}}]}}}),
        json!({"pump_until": {"event": {"$not": {"Activity": {"id": "s1", "source": "Output"}}}}}),
        json!({"pump_until": {"event": {"Completed": {"op": "$w"}}, "ignore": [{"Activity": {"id": "s1", "source": "Output"}}]}}),
        quiet.clone(),
        json!({"drain": {}}),
        json!({"pump": {}}),
        json!({"advance_clock": {"unix": 5001}}),
    ] {
        assert_eq!(
            reads(json!([out("s1"), not_fence, state("$m")])),
            vec![2],
            "{not_fence}"
        );
    }
    // R-80 addition (1): a derived event of the session fences the output only when it ends the wait (a pump_until's
    // event, a pump_collect's member) and the output's last bytes are the sequence that posts it.
    let out_hex =
        |hex: &str| json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": hex}});
    let until = |event: Value| json!({"pump_until": {"event": event}});
    let modes = json!({"ModesChanged": {"id": "s1"}});
    let titled = json!({"TitleChanged": {"id": "s1"}});
    let bell = json!({"Bell": {"id": "s1"}});
    let osc2 = format!("42{}1b5c", sentinel_hex("T1"));
    let osc0_bel = "421b5d303b543107";
    for (hex, fence) in [
        ("421b5b3f3130303468", until(modes.clone())),
        ("421b5b3d33313b3175", until(modes.clone())),
        // The kitty flags push (`CSI > 1 u`) and pop (`CSI < u`) (017f #54 F8).
        ("421b5b3e3175", until(modes.clone())),
        ("421b5b3c75", until(modes.clone())),
        ("421b3d", until(modes.clone())),
        (osc2.as_str(), until(titled.clone())),
        (osc0_bel, until(titled.clone())),
        ("4207", until(bell.clone())),
        (
            "1b5b3f3130303468",
            json!({"pump_collect": {"events": [modes.clone(), {"FocusChanged": {"id": "s1"}}]}}),
        ),
    ] {
        assert_eq!(
            reads(json!([out_hex(hex), fence, state("$m")])),
            Vec::<usize>::new(),
            "{hex} {fence}"
        );
    }
    for (hex, not_fence) in [
        // More bytes follow the sequence that posts the event.
        ("1b5b3f313030346842", until(modes.clone())),
        (&format!("{osc2}42"), until(titled.clone())),
        ("074242", until(bell.clone())),
        // The BEL ends an OSC, so it rings no bell; an OSC 1 sets no title.
        (osc0_bel, until(bell.clone())),
        ("421b5d313b543107", until(titled.clone())),
        // A CSI `u` that changes no mode: SCORC (`CSI u`) and the kitty flags query (`CSI ? u`) (017f #54 F4).
        ("421b5b75", until(modes.clone())),
        ("421b5b3f75", until(modes.clone())),
        // Another derived event than the one that the last bytes post.
        ("421b5b3f3130303468", until(titled.clone())),
        // The event of another session, and the event in an `ignore` or `forbid` matcher (it never ends the wait).
        (
            "421b5b3f3130303468",
            until(json!({"ModesChanged": {"id": "s2"}})),
        ),
        (
            "421b5b3f3130303468",
            json!({"pump_until": {"event": {"Completed": {"op": "$w"}}, "ignore": [modes.clone()]}}),
        ),
        (
            "421b5b3f3130303468",
            json!({"pump_collect": {"events": [{"Completed": {"op": "$w"}}], "forbid": [modes.clone()]}}),
        ),
    ] {
        assert_eq!(
            reads(json!([out_hex(hex), not_fence, state("$m")])),
            vec![2],
            "{hex} {not_fence}"
        );
    }
    // A derived fence clears only the output whose last bytes post it: here the mode output is fenced, then a later
    // output that posts no mode is not.
    assert_eq!(
        reads(json!([
            out_hex("1b5b3f3130303468"),
            out("s1"),
            until(modes.clone()),
            state("$m")
        ])),
        vec![3]
    );
    // A read is no fence, so each later read after the same output is a finding (017f #54 F2: a comparison of an
    // already-bound model_rev, and a model_rev bound by a reply wait, leave the guard after an unfenced output; 016c
    // #54 R1-1: the comparison at the first read is no new binding, and the guarded begin is still flagged).
    assert_eq!(
        reads(json!([state("$m"), out("s1"), state("$m"), guarded("$m")])),
        vec![2, 3]
    );
    assert_eq!(
        reads(json!([screen, reply, out("s1"), guarded("$rev2")])),
        vec![3]
    );
    assert_eq!(
        reads(json!([
            out("s1"),
            activity("s1"),
            state("$m"),
            out("s1"),
            guarded("$m")
        ])),
        vec![4]
    );
    // Not flagged: a read before any output, a read of another session, and an output of another session.
    assert_eq!(
        reads(json!([state("$m"), out("s2"), guarded("$m")])),
        Vec::<usize>::new()
    );
    assert_eq!(
        reads(json!([out("s1"), {"call": {"method": "terminal_state", "args": {"id": "s2"}}}])),
        Vec::<usize>::new()
    );
}

// R-88, the R-80 follow-up: `uncarriable_sequence` is a program output of its session, and an `attach_route` (its baseline)
// and a `queue_overflow` on a route of the session (its resync) are model reads of the session.
#[test]
fn an_attach_and_a_resync_are_model_reads_and_an_uncarriable_sequence_is_an_output() {
    let reads = |steps: Value| -> Vec<usize> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == UNFENCED_READ)
            .map(|(at, _)| at)
            .collect()
    };
    let attach =
        |r: &str, s: &str| json!({"attach_route": {"route": r, "session": s, "client": "c1"}});
    let overflow = |r: &str| json!({"control": {"op": "queue_overflow"}, "route": r});
    let out = |s: &str| json!({"control": {"op": "pty_output", "session": s, "bytes_hex": "42"}});
    let uncarriable = |s: &str| json!({"control": {"op": "uncarriable_sequence", "session": s}});
    let activity =
        |s: &str| json!({"pump_until": {"event": {"Activity": {"id": s, "source": "Output"}}}});
    let quiet = json!({"await_quiet": {}});
    let capture = json!({"begin": {"op": {"CaptureSnapshot": {"session": "s1", "owner": "c1"}}}, "bind": "$k"});
    // The attach's baseline, and the resync of a route of the session, after an unfenced output (a8_2 s8, a8_2 s14).
    assert_eq!(
        reads(json!([out("s1"), quiet, attach("r1", "s1")])),
        vec![2]
    );
    assert_eq!(
        reads(json!([
            attach("r1", "s1"),
            out("s1"),
            quiet,
            overflow("r1")
        ])),
        vec![3]
    );
    // An `uncarriable_sequence` is an output: a capture after it needs a fence (a8_2 s12).
    assert_eq!(reads(json!([uncarriable("s1"), quiet, capture])), vec![2]);
    // The session's output fence clears the one-byte output. A plain `Activity` wait does not cover an uncarriable
    // sequence, which is longer than one byte (the R-80 follow-up of 2026-10-11).
    assert_eq!(
        reads(json!([
            out("s1"),
            activity("s1"),
            attach("r1", "s1"),
            uncarriable("s1"),
            activity("s1"),
            overflow("r1")
        ])),
        vec![5]
    );
    // Another session's output, a resync of a route of another session, and a route that is not attached read nothing
    // of the output.
    assert_eq!(
        reads(json!([out("s2"), attach("r1", "s1")])),
        Vec::<usize>::new()
    );
    assert_eq!(
        reads(json!([
            attach("r1", "s1"),
            attach("r2", "s2"),
            out("s1"),
            overflow("r2")
        ])),
        Vec::<usize>::new()
    );
    assert_eq!(
        reads(json!([out("s1"), overflow("r9")])),
        Vec::<usize>::new()
    );
}

// The R-80 follow-up of 2026-10-11 (`ad_3`): a real read can split an output step, and a plain `Activity {S, Output}` wait
// ends at the first `Activity`. So the wait covers only a one-byte step. A title sentinel at the end of the step covers it
// at any length, and a derived event covers it when the step's last bytes post that event.
#[test]
fn a_plain_activity_wait_covers_only_a_one_byte_output() {
    let reads = |steps: Value| -> Vec<usize> {
        rules(steps)
            .into_iter()
            .filter(|(_, r)| *r == UNFENCED_READ)
            .map(|(at, _)| at)
            .collect()
    };
    let out =
        |hex: &str| json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": hex}});
    let activity = json!({"pump_until": {"event": {"Activity": {"id": "s1", "source": "Output"}}}});
    let screen = json!({"begin": {"op": {"ReadScreen": {"session": "s1", "history": false}}}, "bind": "$rs"});
    assert_eq!(
        reads(json!([out("42"), activity, screen])),
        Vec::<usize>::new()
    );
    assert_eq!(reads(json!([out("4243"), activity, screen])), vec![2]);
    let titled = format!("4243{}1b5c", sentinel_hex("t"));
    let title = json!({"await_title": {"session": "s1", "title": "t"}});
    assert_eq!(
        reads(json!([out(&titled), title, screen])),
        Vec::<usize>::new()
    );
    let bell = json!({"pump_until": {"event": {"Bell": {"id": "s1"}}}});
    assert_eq!(
        reads(json!([out("424307"), bell, screen])),
        Vec::<usize>::new()
    );
    // The count runs across steps since the last fence (the strict form): two one-byte steps are two bytes, and an
    // output whose bytes the transcript does not give has no known count.
    assert_eq!(
        reads(json!([out("42"), out("43"), activity, screen])),
        vec![3]
    );
    let uncarriable = json!({"control": {"op": "uncarriable_sequence", "session": "s1"}});
    assert_eq!(
        reads(json!([uncarriable, out("43"), activity, screen])),
        vec![3]
    );
    assert_eq!(
        reads(json!([out("42"), activity, out("43"), activity, screen])),
        Vec::<usize>::new()
    );
    // A derived event tests the last step's bytes only.
    assert_eq!(
        reads(json!([out("07"), uncarriable, bell, screen])),
        vec![3]
    );
}

// The exception list's note (the steward, R-88 lint format): an optional fourth field that starts with `debt: ` or
// `exception: `. The note never decides a match.
#[test]
fn an_exception_has_three_fields_or_a_debt_or_exception_note() {
    let one = |line: &str| parse_exceptions(line);
    let entry = BTreeSet::from([("conf::x".to_string(), 3, "R".to_string())]);
    assert_eq!(one("conf::x  step 3  R"), Ok(entry.clone()));
    assert_eq!(
        one("conf::x  step 3  R  debt: owner RC, pump_until Activity{s1, Output} after s2"),
        Ok(entry.clone())
    );
    assert_eq!(
        one("conf::x  step 3  R  exception: an unterminated OSC string"),
        Ok(entry)
    );
    for bad in [
        "conf::x  step 3  R  a note with no kind",
        "conf::x  step 3  R  debt: a  exception: b",
        "conf::x  step 3",
        "conf::x  3  R",
    ] {
        assert!(one(bad).is_err(), "{bad}");
    }
    assert_eq!(
        parse_exceptions("# a comment\n\nconf::x  step 3  R\n").map(|s| s.len()),
        Ok(1)
    );
}

// R-83 (B): a `written_bytes` that a route matcher binds from an `input_done` or `input_refused` on a route of the session is a
// run-time count of that session, so an `await_pty` of it fences a later route read. The same name on another session's route,
// a matcher with no attached route, or another frame's field fences nothing.
#[test]
fn a_written_count_that_a_route_matcher_binds_fences_a_route_read() {
    let attach =
        |r: &str, s: &str| json!({"attach_route": {"route": r, "session": s, "client": "c1"}});
    let done = |on: &str, frame: &str| json!({"expect_eventually": {"frame": frame, "op": "1", "written_bytes": "$n"}, "on": on});
    let fence = |s: &str| json!({"await_pty": {"session": s, "min_bytes": "$n"}});
    let read =
        json!({"route_expect_pty": [{"written_by_op": "1", "len_var": "$n"}], "route": "r1"});
    let unfenced = |steps: Value| {
        rules(steps)
            .into_iter()
            .filter(|(_, rule)| *rule == "unfenced route read, R-56")
            .collect::<Vec<_>>()
    };
    for frame in ["input_done", "input_refused"] {
        assert_eq!(
            unfenced(json!([
                attach("r1", "s1"),
                done("r1", frame),
                fence("s1"),
                read
            ])),
            vec![],
            "{frame}"
        );
    }
    assert_eq!(
        unfenced(json!([
            attach("r1", "s1"),
            attach("r2", "s2"),
            done("r2", "input_done"),
            fence("s1"),
            read
        ])),
        vec![(4, "unfenced route read, R-56")]
    );
    assert_eq!(
        unfenced(json!([
            attach("r1", "s1"),
            done("r9", "input_done"),
            fence("s1"),
            read
        ])),
        vec![(3, "unfenced route read, R-56")]
    );
    assert_eq!(
        unfenced(json!([
            attach("r1", "s1"),
            done("r1", "output"),
            fence("s1"),
            read
        ])),
        vec![(3, "unfenced route read, R-56")]
    );
}

// R-83: the floor of a route read. A fence that the lint knows (a literal, or a fixed `let`) must cover the floor; one byte less
// is a finding (017f F1, F2 on #57).
#[test]
fn a_known_fence_covers_the_floor_of_a_route_read() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let read = json!({"route_expect_pty": [{"hex": "616263"}], "route": "r1"});
    let literal = |n: u64| json!({"await_pty": {"session": "s1", "min_bytes": n}});
    let fixed = |a: u64, b: u64| json!({"let": {"$n": {"add": [a, b]}}});
    let by_let = json!({"await_pty": {"session": "s1", "min_bytes": "$n"}});
    assert_eq!(route_reads(json!([attach, literal(2), read])), vec![2]);
    assert_eq!(
        route_reads(json!([attach, literal(3), read])),
        Vec::<usize>::new()
    );
    assert_eq!(
        route_reads(json!([attach, fixed(1, 0), by_let, read])),
        vec![3]
    );
    assert_eq!(
        route_reads(json!([attach, fixed(2, 0), by_let, read])),
        vec![3]
    );
    assert_eq!(
        route_reads(json!([attach, fixed(2, 1), by_let, read])),
        Vec::<usize>::new()
    );
}

// R-83: the floor of each part. `hex` counts its bytes, a `var` 0, a `written_by_op` of a `bytes` op, a `text` op or a committed
// paste its op's body (R-73 follow-up 2 (2)), and an aborted paste or an op with no known bytes 1. `one_of` takes the least
// sequence; an empty sequence is an absence check (017f F2 on #57).
#[test]
fn the_floor_of_a_route_read_counts_each_part() {
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let fence = |n: u64| json!({"await_pty": {"session": "s1", "min_bytes": n}});
    let read = |parts: Value| json!({"route_expect_pty": parts, "route": "r1"});
    let by_op = |op: &str| read(json!([{"written_by_op": op}]));
    // The least fence that the lint accepts: None when the read needs no fence, else the n for which n is clean and n - 1 is a
    // finding.
    let floor = |sends: Vec<Value>, read: Value| -> Option<u64> {
        let with = |fenced: Option<u64>| {
            let mut steps = vec![attach.clone()];
            steps.extend(sends.iter().cloned());
            steps.extend(fenced.map(fence));
            steps.push(read.clone());
            route_reads(Value::Array(steps)).is_empty()
        };
        if with(None) {
            return None;
        }
        Some(
            (0..64)
                .find(|&n| with(Some(n)))
                .expect("a fence of 63 or fewer bytes"),
        )
    };
    let bytes =
        json!({"route_send": {"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "616263"}}});
    let text = json!({"route_send": {"frame": "text", "op": "2", "text": "abcd"}});
    let begin =
        |op: &str| json!({"route_send": {"frame": "input", "kind": "paste_begin", "op": op}});
    let chunk = |op: &str, hex: &str| json!({"route_send": {"frame": "paste_chunk", "op": op, "bytes": {"$bytes_hex": hex}}});
    let commit =
        |op: &str| json!({"route_send": {"frame": "input", "kind": "paste_commit", "op": op}});
    let abort =
        |op: &str| json!({"route_send": {"frame": "input", "kind": "paste_abort", "op": op}});

    assert_eq!(
        floor(vec![], read(json!([{"hex": "6162"}, {"var": "$x"}]))),
        Some(2)
    );
    assert_eq!(floor(vec![bytes], by_op("1")), Some(3));
    assert_eq!(floor(vec![text], by_op("2")), Some(4));
    assert_eq!(
        floor(
            vec![
                begin("3"),
                chunk("3", "6162"),
                chunk("3", "63"),
                commit("3")
            ],
            by_op("3")
        ),
        Some(3)
    );
    assert_eq!(
        floor(
            vec![begin("4"), chunk("4", "616263"), abort("4")],
            by_op("4")
        ),
        Some(1)
    );
    assert_eq!(floor(vec![], by_op("9")), Some(1));
    assert_eq!(
        floor(
            vec![],
            json!({"route_expect_pty": {"one_of": [[{"hex": "616263"}], [{"hex": "61"}], [{"hex": "6162"}]]}, "route": "r1"})
        ),
        Some(1)
    );
    // A `var` part counts 0, but a read of `var` parts only still needs a fence of the session (016c item 2 on #57). Only an
    // empty sequence, alone or in a `one_of`, needs none.
    assert_eq!(floor(vec![], read(json!([{"var": "$x"}]))), Some(1));
    assert_eq!(floor(vec![], read(json!([]))), None);
    assert_eq!(
        floor(
            vec![],
            json!({"route_expect_pty": {"one_of": [[{"hex": "61"}], []]}, "route": "r1"})
        ),
        None
    );
}

// R-83: a known run is a run of the read's session. An op of another session, or a second open paste of the op on another
// route, does not raise the floor (016c item 1 on #57).
#[test]
fn a_known_run_of_another_session_does_not_raise_the_floor() {
    let attach =
        |r: &str, s: &str| json!({"attach_route": {"route": r, "session": s, "client": "c1"}});
    let on = |r: &str, frame: Value| {
        let mut step = json!({"route_send": frame});
        step["route"] = json!(r);
        step
    };
    let paste = |r: &str, hex: &str| {
        vec![
            on(
                r,
                json!({"frame": "input", "kind": "paste_begin", "op": "1"}),
            ),
            on(
                r,
                json!({"frame": "paste_chunk", "op": "1", "bytes": {"$bytes_hex": hex}}),
            ),
            on(
                r,
                json!({"frame": "input", "kind": "paste_commit", "op": "1"}),
            ),
        ]
    };
    let tail = |n: u64| {
        vec![
            json!({"await_pty": {"session": "s1", "min_bytes": n}}),
            json!({"route_expect_pty": [{"written_by_op": "1"}], "route": "r1"}),
        ]
    };
    let mut both = vec![attach("r1", "s1"), attach("r2", "s2")];
    both.extend(paste("r1", "61"));
    both.extend(paste("r2", "6162636465"));
    let mut fenced = both.clone();
    fenced.extend(tail(1));
    assert_eq!(route_reads(Value::Array(fenced)), Vec::<usize>::new());
    // Only s2 has a known run of op 1; on s1, op 1 has no known bytes, so its floor is 1.
    let mut other = vec![attach("r1", "s1"), attach("r2", "s2")];
    other.extend(paste("r2", "6162636465"));
    other.extend(tail(1));
    assert_eq!(route_reads(Value::Array(other)), Vec::<usize>::new());
    let mut short = both;
    short.extend(tail(0));
    assert_eq!(route_reads(Value::Array(short)), vec![9]);
}

// R-83: a read of `var` parts only, such as an `oracle_encode` value, needs a fence (016c item 2 on #57).
#[test]
fn a_read_of_var_parts_only_needs_a_fence() {
    let steps = |fence: Vec<Value>| {
        let mut steps = vec![
            json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}}),
            json!({"control": {"op": "oracle_encode", "session": "s1", "kind": "key", "input": {"key": {"char": "e"}, "mods": [], "event": "press", "text": "e"}, "modes": {"kitty_flags": 0}}, "bind": "$e"}),
            json!({"route_send": {"frame": "input", "kind": "key", "key": {"char": "e"}, "mods": [], "event": "press", "text": "e"}}),
        ];
        steps.extend(fence);
        steps.push(json!({"route_expect_pty": [{"var": "$e"}], "route": "r1"}));
        Value::Array(steps)
    };
    let await_pty = |n: Value| json!({"await_pty": {"session": "s1", "min_bytes": n}});
    let fixed = |n: u64| json!({"let": {"$n": {"add": [n, 0]}}});
    assert_eq!(route_reads(steps(vec![])), vec![3]);
    // A fence of 0 bytes returns at once, so it fences nothing (017f F4 on #57): a literal, and a fixed `let`.
    assert_eq!(route_reads(steps(vec![await_pty(json!(0))])), vec![4]);
    assert_eq!(
        route_reads(steps(vec![fixed(0), await_pty(json!("$n"))])),
        vec![5]
    );
    assert_eq!(
        route_reads(steps(vec![await_pty(json!(1))])),
        Vec::<usize>::new()
    );
    assert_eq!(
        route_reads(steps(vec![fixed(1), await_pty(json!("$n"))])),
        Vec::<usize>::new()
    );
}

// R-83: a paste is open on its route. Two open pastes of one op on two routes of one session are two runs, so the floor of
// one is its own body (017f F3 on #57).
#[test]
fn an_open_paste_is_keyed_by_its_route() {
    let attach = |r: &str| json!({"attach_route": {"route": r, "session": "s1", "client": "c1"}});
    let on = |r: &str, frame: Value| {
        let mut step = json!({"route_send": frame});
        step["route"] = json!(r);
        step
    };
    let steps = |n: u64| {
        json!([
            attach("r1"),
            attach("r2"),
            on("r1", json!({"frame": "input", "kind": "paste_begin", "op": "1"})),
            on("r2", json!({"frame": "input", "kind": "paste_begin", "op": "1"})),
            on("r1", json!({"frame": "paste_chunk", "op": "1", "bytes": {"$bytes_hex": "61626364"}})),
            on("r2", json!({"frame": "paste_chunk", "op": "1", "bytes": {"$bytes_hex": "65"}})),
            on("r1", json!({"frame": "input", "kind": "paste_commit", "op": "1"})),
            {"await_pty": {"session": "s1", "min_bytes": n}},
            {"route_expect_pty": [{"written_by_op": "1"}], "route": "r1"}
        ])
    };
    assert_eq!(route_reads(steps(4)), Vec::<usize>::new());
    assert_eq!(route_reads(steps(3)), vec![8]);
}

/// The steps of the `unfenced route read, R-56` findings.
fn route_reads(steps: Value) -> Vec<usize> {
    rules(steps)
        .into_iter()
        .filter(|(_, rule)| *rule == UNFENCED_ROUTE_READ)
        .map(|(step, _)| step)
        .collect()
}

/// The steps of the R-88 findings of a step list.
fn r88(steps: Value) -> Vec<usize> {
    report::lint("t", steps.as_array().unwrap())
        .into_iter()
        .map(|f| f.step)
        .collect()
}

/// A list with `fence` inserted at `at`.
fn with(steps: &Value, at: usize, fence: Value) -> Value {
    let mut steps = steps.as_array().unwrap().clone();
    steps.insert(at, fence);
    Value::Array(steps)
}

fn start(session: &str, var: &str) -> Value {
    json!({"begin": {"op": {"Start": {"id": session}}}, "bind": var})
}

fn completed(var: &str) -> Value {
    json!({"pump_until": {"event": {"Completed": {"op": var, "result": {}}}}})
}

fn write_on(session: &str, handle: &str, var: &str, hex: &str) -> Value {
    json!({"begin": {"op": {"WriteInput": {"session": session, "payload": {"bytes": {"bytes": {"$bytes_hex": hex}}}}}, "handle": handle}, "bind": var})
}

fn poll() -> Value {
    json!({"poll_events": {"max": 64}, "expect": []})
}

fn pump() -> Value {
    json!({"pump": {}})
}

fn get(session: &str, state: Value) -> Value {
    json!({"call": {"method": "get", "args": {"id": session}}, "expect": {"id": session, "state": state}})
}

fn wait_for(event: Value) -> Value {
    json!({"pump_until": {"event": event}})
}

fn ctl_wait(control: Value, expect: Value) -> Value {
    json!({"await_control": {"control": control, "expect": expect}})
}

/// The R-88 race of `am_4` (P5's diagnosis): the `poll_events` read `$w2`'s report with no fence. #74's
/// `op_posted {for_op: $w2}` at step 11 is its fence. The later writes `$w1` and `$w3` begin after that fence, so their
/// match at step 25 is an exception line (queue order).
#[test]
fn the_report_rule_finds_am_4s_race_and_its_op_posted_fence() {
    let transcripts =
        botster_conformance::load_dir(&botster_core_conformance::CORE_TRANSCRIPTS).unwrap();
    let am_4 = transcripts
        .iter()
        .find(|t| t.id == "conf::am_4_capacity_returns_on_poll")
        .unwrap();
    let fence = json!({"await_control": {"control": {"op": "op_posted", "for_op": "$w2"}, "expect": {"posted": true}}, "because": "Core AM-4"});
    assert_eq!(am_4.steps[11], fence);
    // am_4's steps before `$w1` begins, then its poll: only `$w2`'s report can decide the read.
    let mut fenced = am_4.steps[..15].to_vec();
    fenced.push(am_4.steps[25].clone());
    assert_eq!(r88(Value::Array(fenced.clone())), Vec::<usize>::new());
    let mut unfenced = fenced;
    unfenced.remove(11);
    assert_eq!(r88(Value::Array(unfenced)), [14]);
}

#[test]
fn a_life_cause_needs_a_report_made_state_before_a_read() {
    let base = json!([start("s1", "$s"), pump(), get("s1", json!("Running"))]);
    assert_eq!(r88(base.clone()), [2]);
    // A read right after the begin, with no pump between them, sees no report yet (OR-1).
    assert_eq!(
        r88(json!([start("s1", "$s"), get("s1", json!("Running"))])),
        Vec::<usize>::new()
    );
    for fence in [
        json!({"await_get": {"id": "s1", "expect": {"state": "Running"}}}),
        wait_for(json!({"SessionState": {"id": "s1", "state": "Running"}})),
        completed("$s"),
        json!({"expect": {"Completed": {"op": "$s", "result": {}}}}),
        ctl_wait(
            json!({"op": "parked_work", "session": "s1"}),
            json!({"parked": true}),
        ),
    ] {
        assert_eq!(
            r88(with(&base, 2, fence.clone())),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    // A begin state, a state that proves nothing, another class's fence, or another operation is no fence.
    for fence in [
        json!({"await_get": {"id": "s1", "expect": {"state": "Starting"}}}),
        wait_for(json!({"SessionState": {"id": "s1", "state": "$any"}})),
        wait_for(json!({"SessionState": {"id": "s1", "state": "$st"}})),
        wait_for(json!({"SessionState": {"id": "s1"}})),
        wait_for(json!({"Activity": {"id": "s1", "source": "Output"}})),
        completed("$other"),
        wait_for(json!({"$one_of": [{"Completed": {"op": "$s"}}, {"Bell": {"id": "s1"}}]})),
        ctl_wait(
            json!({"op": "parked_work", "session": "s1"}),
            json!({"parked": false}),
        ),
    ] {
        assert_eq!(r88(with(&base, 2, fence.clone())), [3], "{fence}");
    }
    // A get that accepts both the begin state and the report state does not depend on the report.
    let either = json!([
        start("s1", "$s"),
        pump(),
        get("s1", json!({"$one_of": ["Starting", "Running"]}))
    ]);
    assert_eq!(r88(either), Vec::<usize>::new());
}

#[test]
fn an_input_cause_is_fenced_by_its_own_operation() {
    let base = json!([
        start("s1", "$s"),
        completed("$s"),
        write_on("s1", "h", "$w", "61"),
        pump(),
        poll()
    ]);
    assert_eq!(r88(base.clone()), [4]);
    for fence in [
        completed("$w"),
        json!({"expect": {"Completed": {"op": "$w"}}}),
        ctl_wait(
            json!({"op": "op_posted", "for_op": "$w"}),
            json!({"posted": true}),
        ),
    ] {
        assert_eq!(
            r88(with(&base, 4, fence.clone())),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    for fence in [
        completed("$s"),
        ctl_wait(
            json!({"op": "op_posted", "for_op": "$w"}),
            json!({"posted": false}),
        ),
        ctl_wait(
            json!({"op": "op_posted", "op_var": "$w"}),
            json!({"posted": true}),
        ),
        ctl_wait(
            json!({"op": "route_reports", "session": "s1", "route": "r1"}),
            json!({"activity_before_close": true}),
        ),
    ] {
        assert_eq!(r88(with(&base, 4, fence.clone())), [5], "{fence}");
    }
    // A variable that a later begin binds again is another operation: h2's Completed{$w} does not clear h's $w.
    let rebound = json!([
        start("s1", "$s"),
        completed("$s"),
        write_on("s1", "h", "$w", "61"),
        {"begin": {"op": {"Start": {"id": "s2"}}, "handle": "h2"}, "bind": "$s2"},
        {"pump_until": {"event": {"Completed": {"op": "$s2"}}, "handle": "h2"}},
        write_on("s2", "h2", "$w", "62"),
        {"pump_until": {"event": {"Completed": {"op": "$w"}}, "handle": "h2"}},
        poll(),
        {"poll_events": {"max": 64, "handle": "h2"}, "expect": []},
    ]);
    assert_eq!(r88(rebound.clone()), [7]);
    // An operation's Completed fences only on the handle that began it: a wait on h does not clear h2's $w.
    let mut on_h = rebound.as_array().unwrap().clone();
    on_h[6] = completed("$w");
    assert_eq!(r88(Value::Array(on_h)), [7, 8]);
}

#[test]
fn every_forwarded_op_is_an_input_cause_once_the_session_has_a_worker() {
    let resize = json!({"begin": {"op": {"Resize": {"session": "s1", "size": {"rows": 30, "cols": 100}}}}, "bind": "$rz"});
    assert_eq!(
        r88(json!([
            start("s1", "$s"),
            completed("$s"),
            resize,
            pump(),
            poll()
        ])),
        [4]
    );
    // With no worker, Core answers the setter itself.
    assert_eq!(r88(json!([resize, pump(), poll()])), Vec::<usize>::new());
    // A fatal Signal also causes an exit report; its Completed does not prove that report.
    let signal = json!({"begin": {"op": {"Signal": {"id": "s1", "sig": "kill"}}}, "bind": "$sig"});
    let base = json!([
        start("s1", "$s"),
        completed("$s"),
        signal,
        completed("$sig"),
        get("s1", json!("Exited"))
    ]);
    assert_eq!(r88(base.clone()), [4]);
    let exited = json!({"await_get": {"id": "s1", "expect": {"state": {"Exited": "Killed"}}}});
    assert_eq!(r88(with(&base, 4, exited)), Vec::<usize>::new());
}

#[test]
fn a_remove_and_a_detach_are_causes() {
    let remove = json!({"begin": {"op": {"Remove": {"id": "s1"}}}, "bind": "$rm"});
    let gone =
        json!({"call": {"method": "get", "args": {"id": "s1"}}, "expect_error": "UnknownSession"});
    let base = json!([start("s1", "$s"), completed("$s"), remove, pump(), gone]);
    assert_eq!(r88(base.clone()), [4]);
    let fence = json!({"await_get": {"id": "s1", "expect_error": "UnknownSession"}});
    assert_eq!(r88(with(&base, 4, fence)), Vec::<usize>::new());
    let attach = json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1"}});
    let detach = json!({"begin": {"op": {"Detach": {"route": "$r1", "reason": "Detached"}}}, "bind": "$det"});
    let base = json!([attach, detach, pump(), poll()]);
    assert_eq!(r88(base.clone()), [3]);
    for fence in [
        wait_for(json!({"RouteClosed": {"route": "$r1"}})),
        ctl_wait(
            json!({"op": "route_reports", "session": "s1", "route": "$r1"}),
            json!({"closed": true}),
        ),
    ] {
        assert_eq!(
            r88(with(&base, 3, fence.clone())),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    let other = ctl_wait(
        json!({"op": "route_reports", "session": "s1", "route": "$r2"}),
        json!({"closed": true}),
    );
    assert_eq!(r88(with(&base, 3, other)), [4]);
}

#[test]
fn an_output_cause_takes_r_80s_output_fences() {
    let output =
        |hex: &str| json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": hex}});
    let base = json!([output("61"), pump(), poll()]);
    assert_eq!(r88(base.clone()), [2]);
    for fence in [
        wait_for(json!({"Activity": {"id": "s1", "source": "Output"}})),
        json!({"await_title": {"session": "s1", "title": "x"}}),
        wait_for(json!({"EventsLost": {"id": "s1", "kinds": ["Activity"]}})),
    ] {
        assert_eq!(
            r88(with(&base, 2, fence.clone())),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    // A plain Activity wait fences only a one-byte output (the R-80 follow-up of 2026-10-11).
    let activity = wait_for(json!({"Activity": {"id": "s1", "source": "Output"}}));
    assert_eq!(
        r88(with(
            &json!([output("6162"), pump(), poll()]),
            2,
            activity.clone()
        )),
        [3]
    );
    // ... counted across steps since the last fence.
    let two = json!([output("61"), output("62"), pump(), poll()]);
    assert_eq!(r88(with(&two, 3, activity.clone())), [4]);
    let fenced = json!([output("61"), pump(), activity, output("62"), pump(), poll()]);
    assert_eq!(r88(fenced), [5]);
    // A derived event fences only an output whose last bytes post it (R-80 addition (1)).
    let bell = wait_for(json!({"Bell": {"id": "s1"}}));
    assert_eq!(r88(with(&base, 2, bell.clone())), [3]);
    assert_eq!(
        r88(with(&json!([output("07"), pump(), poll()]), 2, bell)),
        Vec::<usize>::new()
    );
    // A named-event read is scoped to its events' classes and sessions.
    let forbid = |events: Value| json!({"forbid_events": {"events": events}});
    assert_eq!(
        r88(json!([
            output("61"),
            pump(),
            forbid(json!([{"Bell": {"id": "s1"}}]))
        ])),
        [2]
    );
    assert_eq!(
        r88(json!([
            output("61"),
            pump(),
            forbid(json!([{"Bell": {"id": "s2"}}]))
        ])),
        Vec::<usize>::new()
    );
    assert_eq!(
        r88(json!([
            output("61"),
            pump(),
            forbid(json!([{"FocusChanged": {"id": "s1"}}]))
        ])),
        Vec::<usize>::new()
    );
}

#[test]
fn a_client_cause_is_fenced_per_route_and_the_sticky_flag_proves_only_the_first_send() {
    let attach = |r: &str| json!({"attach_route": {"route": r, "session": "s1", "client": "c1"}});
    let send = |r: &str| json!({"route_send": {"frame": "input", "bytes_hex": "61"}, "route": r});
    let activity = |r: &str| {
        ctl_wait(
            json!({"op": "route_reports", "session": "s1", "route": r}),
            json!({"activity_before_close": true, "closed": false}),
        )
    };
    let base = json!([attach("r1"), attach("r2"), send("r2"), pump(), poll()]);
    assert_eq!(r88(base.clone()), [4]);
    // RC's #69 form: r2's route_reports fences r2's send; r1's does not.
    assert_eq!(r88(with(&base, 4, activity("$r2"))), Vec::<usize>::new());
    assert_eq!(r88(with(&base, 4, activity("$r1"))), [5]);
    assert_eq!(
        r88(with(
            &base,
            4,
            wait_for(json!({"Activity": {"id": "s1", "source": {"Client": "$r2"}}}))
        )),
        Vec::<usize>::new()
    );
    assert_eq!(
        r88(with(
            &base,
            4,
            wait_for(json!({"Activity": {"id": "s1", "source": {"Client": "$r1"}}}))
        )),
        [5]
    );
    // The flag stays true after the first send, so it does not prove a later send on the same route.
    let again = json!([
        attach("r1"),
        send("r1"),
        pump(),
        activity("r1"),
        send("r1"),
        pump(),
        activity("r1"),
        poll()
    ]);
    assert_eq!(r88(again), [7]);
}

#[test]
fn route_frame_absences_drains_barriers_and_refused_begins_are_no_r_88_pairs() {
    let output = json!({"control": {"op": "pty_output", "session": "s1", "bytes_hex": "61"}});
    let quiet_route =
        json!({"expect_quiet_until": {"output": {}}, "on": "r1", "until": {"Bell": {"id": "s1"}}});
    assert_eq!(
        r88(json!([output, pump(), quiet_route, {"drain": {}}])),
        Vec::<usize>::new()
    );
    // The read's own barrier is not its cause.
    let barrier = json!({
        "expect_quiet_until": {"SessionState": {"id": "s1"}},
        "barrier": {"begin": {"op": {"Stop": {"id": "s1"}}}, "bind": "$st"},
        "until": {"Completed": {"op": "$st", "result": {}}},
    });
    assert_eq!(
        r88(json!([start("s1", "$s"), completed("$s"), barrier])),
        Vec::<usize>::new()
    );
    // A refused begin sends nothing to a worker.
    let refused =
        json!({"begin": {"op": {"Start": {"id": "s9"}}}, "expect_error": "UnknownSession"});
    assert_eq!(
        r88(json!([refused, pump(), get("s9", json!("Running"))])),
        Vec::<usize>::new()
    );
    // A Stop of an ended session is no cause.
    let stop = json!({"begin": {"op": {"Stop": {"id": "s1"}}}, "bind": "$st"});
    let exited = json!({"await_get": {"id": "s1", "expect": {"state": {"Exited": "Killed"}}}});
    assert_eq!(
        r88(json!([
            start("s1", "$s"),
            completed("$s"),
            stop,
            exited,
            stop,
            pump(),
            get("s1", json!("Exited"))
        ])),
        Vec::<usize>::new()
    );
}

#[test]
fn a_cancel_reads_its_own_operation_and_its_sessions_end() {
    let cancel = |expect: Value| json!({"call": {"method": "cancel", "args": {"op": "$w"}}, "expect": expect});
    let base = json!([
        start("s1", "$s"),
        completed("$s"),
        write_on("s1", "h", "$w", "61"),
        pump(),
        cancel(json!("Admitted"))
    ]);
    assert_eq!(r88(base.clone()), [4]);
    assert_eq!(r88(with(&base, 4, completed("$w"))), Vec::<usize>::new());
    // A cancel that accepts both answers does not depend on the report.
    let both = cancel(json!({"$one_of": ["Admitted", "TooLate"]}));
    assert_eq!(
        r88(json!([
            start("s1", "$s"),
            completed("$s"),
            write_on("s1", "h", "$w", "61"),
            pump(),
            both
        ])),
        Vec::<usize>::new()
    );
    // Another operation's cause is not this cancel's.
    let resize = json!({"begin": {"op": {"Resize": {"session": "s1", "size": {"rows": 30, "cols": 100}}}}, "bind": "$rz"});
    let other = json!([
        start("s1", "$s"),
        completed("$s"),
        write_on("s1", "h", "$w", "61"),
        completed("$w"),
        resize,
        pump(),
        cancel(json!("TooLate"))
    ]);
    assert_eq!(r88(other), Vec::<usize>::new());
}

#[test]
fn a_held_write_suspends_its_writes_only_in_r_74s_form() {
    let blocked = |on: bool| json!({"control": {"op": "pty_blocked", "session": "s1", "on": on, "kernel_bytes": 1}});
    let fill = json!({"let": {"$fill": {"repeat": ["61", 40960]}}});
    let queued = |lower: u64| {
        ctl_wait(
            json!({"op": "pty_queued", "session": "s1"}),
            json!({"bytes": {"$between": [lower, 40960]}}),
        )
    };
    let cancel = |var: &str| json!({"call": {"method": "cancel", "args": {"op": var}}, "expect": "Admitted"});
    let held = |write: Value, queued: Value| {
        json!([
            start("s1", "$s"),
            completed("$s"),
            blocked(true),
            fill,
            write,
            queued,
            cancel("$w")
        ])
    };
    let filler = write_on("s1", "h", "$w", "$fill");
    assert_eq!(r88(held(filler.clone(), queued(1))), Vec::<usize>::new());
    // No positive range, no expectation, or a write that the kernel takes whole is no held write.
    assert_eq!(r88(held(filler.clone(), queued(0))), [6]);
    assert_eq!(
        r88(held(
            filler.clone(),
            json!({"await_control": {"control": {"op": "pty_queued", "session": "s1"}}})
        )),
        [6]
    );
    assert_eq!(r88(held(write_on("s1", "h", "$w", "61"), queued(1))), [6]);
    // A write larger than `kernel_bytes` but smaller than the ruled filler: the real tier ignores `kernel_bytes`.
    assert_eq!(r88(held(write_on("s1", "h", "$w", "6162"), queued(1))), [6]);
    // A write after the hold is not held; the held write comes back, unarmed, when the PTY unblocks.
    let mut later = held(filler.clone(), queued(1)).as_array().unwrap().clone();
    later.truncate(6);
    later.extend([write_on("s1", "h", "$w2", "62"), pump(), cancel("$w2")]);
    assert_eq!(r88(Value::Array(later)), [8]);
    let mut released = held(filler, queued(1)).as_array().unwrap().clone();
    released.extend([blocked(false), cancel("$w"), pump(), cancel("$w")]);
    assert_eq!(r88(Value::Array(released)), [10]);
}

#[test]
fn holds_suspend_causes_and_clear_nothing() {
    let start_held = ctl_wait(
        json!({"op": "start_held", "session": "s1"}),
        json!({"held": true}),
    );
    let release = json!({"control": {"op": "release_start", "session": "s1"}});
    let running = get("s1", json!("Running"));
    let steps = json!([
        start("s1", "$s"),
        start_held,
        pump(),
        running,
        release,
        running,
        pump(),
        running
    ]);
    assert_eq!(r88(steps), [7]);
    let stop = json!({"begin": {"op": {"Stop": {"id": "s1"}}}, "bind": "$st"});
    let held = ctl_wait(
        json!({"op": "held_reports", "session": "s1"}),
        json!({"count": {"$between": [1, 1000]}}),
    );
    let exited = get("s1", json!("Exited"));
    let release = json!({"control": {"op": "release_worker_report", "session": "s1"}});
    let steps = json!([
        start("s1", "$s"),
        completed("$s"),
        stop,
        held,
        pump(),
        exited,
        release,
        pump(),
        exited
    ]);
    assert_eq!(r88(steps), [8]);
    let hold = json!({"control": {"op": "scheduler_hold", "for_op": "$w"}});
    let release = json!({"control": {"op": "scheduler_release", "for_op": "$w"}});
    let steps = json!([
        start("s1", "$s"),
        completed("$s"),
        hold,
        write_on("s1", "h", "$w", "61"),
        pump(),
        poll(),
        release,
        pump(),
        poll()
    ]);
    assert_eq!(r88(steps), [8]);
}

#[test]
fn a_checked_control_is_a_read() {
    let row = |check: &str| json!({"control": {"op": "registry_row", "session": "s1"}, check: {"state": "Running"}});
    for check in ["expect", "bind", "bind_fields"] {
        assert_eq!(
            r88(json!([start("s1", "$s"), pump(), row(check)])),
            [2],
            "{check}"
        );
    }
    let unchecked = json!({"control": {"op": "registry_row", "session": "s1"}});
    assert_eq!(
        r88(json!([start("s1", "$s"), pump(), unchecked])),
        Vec::<usize>::new()
    );
}

#[test]
fn an_op_posted_fence_is_on_its_own_handle() {
    let base = json!([
        start("s1", "$s"),
        completed("$s"),
        write_on("s1", "h", "$w", "61"),
        pump(),
        poll()
    ]);
    let on = |handle: &str| json!({"await_control": {"handle": handle, "control": {"op": "op_posted", "for_op": "$w"}, "expect": {"posted": true}}});
    assert_eq!(r88(with(&base, 4, on("h"))), Vec::<usize>::new());
    assert_eq!(r88(with(&base, 4, on("h2"))), [5]);
}

#[test]
fn a_state_that_is_not_terminal_proves_only_a_start() {
    let running = json!({"await_get": {"id": "s1", "expect": {"state": "Running"}}});
    let signal = json!({"begin": {"op": {"Signal": {"id": "s1", "sig": "kill"}}}, "bind": "$sig"});
    let remove = json!({"begin": {"op": {"Remove": {"id": "s1"}}}, "bind": "$rm"});
    for cause in [
        json!({"control": {"op": "lose_worker", "session": "s1"}}),
        json!({"control": {"op": "process_end_worker", "session": "s1"}}),
        json!({"control": {"op": "break_control", "session": "s1"}}),
        signal,
        remove,
    ] {
        let steps = json!([
            start("s1", "$s"),
            completed("$s"),
            cause.clone(),
            pump(),
            running.clone(),
            poll()
        ]);
        assert_eq!(r88(steps), [5], "{cause}");
    }
}

#[test]
fn a_completed_ends_the_session_only_when_every_alternative_does() {
    let ended = |result: Value| {
        json!([
            start("s1", "$s"),
            completed("$s"),
            write_on("s1", "h", "$w", "61"),
            {"control": {"op": "lose_worker", "session": "s1"}},
            pump(),
            {"pump_until": {"event": {"Completed": {"op": "$w", "result": result}}}},
            poll()
        ])
    };
    let session_ended = json!({"NotWritten": "SessionEnded"});
    let outcome = |o: Value| json!({"ok": {"input": {"outcome": o}}});
    assert_eq!(
        r88(ended(outcome(session_ended.clone()))),
        Vec::<usize>::new()
    );
    assert_eq!(
        r88(ended(outcome(
            json!({"$one_of": [session_ended.clone(), session_ended.clone()]})
        ))),
        Vec::<usize>::new()
    );
    assert_eq!(
        r88(ended(outcome(
            json!({"$one_of": [session_ended.clone(), {"Written": {}}]})
        ))),
        [6]
    );
    assert_eq!(
        r88(ended(
            json!({"$one_of": [outcome(session_ended.clone()), {"err": "SessionEnded"}]})
        )),
        [6]
    );
    // The two names apart are no `NotWritten(SessionEnded)`.
    assert_eq!(
        r88(ended(outcome(
            json!({"NotWritten": "Refused", "SessionEnded": true})
        ))),
        [6]
    );
}

#[test]
fn a_parked_frame_clears_only_the_earliest_armed_life_cause() {
    let parked = ctl_wait(
        json!({"op": "parked_work", "session": "s1"}),
        json!({"parked": true}),
    );
    let lose = json!({"control": {"op": "lose_worker", "session": "s1"}});
    let two = json!([start("s1", "$s"), pump(), lose, pump(), parked, poll()]);
    assert_eq!(r88(two), [5]);
    // Two reads with no step between them that takes events can see the same frame: the second clears nothing.
    let twice = json!([
        start("s1", "$s"),
        pump(),
        lose,
        pump(),
        parked,
        parked,
        poll()
    ]);
    assert_eq!(r88(twice), [6]);
    // Only a step that takes a mandatory event of the session can free the room, so only then can a later parked frame
    // be another one (the steward's round-4 ruling, EV-5b/d). A class K event, a drain or a poll_all frees none. A poll
    // between the reads is a read itself (step 5).
    let between = |step: Value| {
        json!([
            start("s1", "$s"),
            pump(),
            lose,
            pump(),
            parked,
            step,
            parked,
            poll()
        ])
    };
    for (step, expected) in [
        (
            json!({"expect_eventually": {"TitleChanged": {"id": "s2"}}}),
            vec![7],
        ),
        (wait_for(json!({"TitleChanged": {"id": "s1"}})), vec![7]),
        (json!({"drain": {}}), vec![7]),
        (json!({"poll_all": {}}), vec![5, 7]),
        (
            wait_for(json!({"SessionState": {"id": "s2", "state": "$any"}})),
            vec![7],
        ),
        (
            wait_for(json!({"$one_of": [{"SessionState": {"id": "s1"}}, {"Bell": {"id": "s1"}}]})),
            vec![7],
        ),
        // A mandatory event of s1 ends the witness.
        (
            wait_for(json!({"SessionState": {"id": "s1", "state": "$any"}})),
            vec![],
        ),
        (
            json!({"poll_events": {"max": 1}, "expect": [{"SessionState": {"id": "s1"}}]}),
            vec![5],
        ),
        (
            json!({"poll_find": {"event": {"Completed": {"op": "$s"}}}}),
            vec![5],
        ),
    ] {
        assert_eq!(r88(between(step.clone())), expected, "{step}");
    }
    // Another session's parked frame is no second read of this one.
    let other = ctl_wait(
        json!({"op": "parked_work", "session": "s2"}),
        json!({"parked": true}),
    );
    let s2 = json!([
        start("s1", "$s"),
        pump(),
        lose,
        pump(),
        parked,
        other,
        poll()
    ]);
    assert_eq!(r88(s2), [6]);
    // A cause that no pump has armed sent no frame yet, so the parked frame is the armed one's.
    let unarmed = json!([start("s1", "$s"), pump(), lose, parked, pump(), poll()]);
    assert_eq!(r88(unarmed), [5]);
}

/// The steps of a `parked_work` witness on h with `steps` between its two reads: the second read is step `5 + steps.len()`,
/// and the poll after it is the read that a fence must cover.
fn parked_twice(steps: Vec<Value>) -> Vec<Value> {
    let parked = ctl_wait(
        json!({"op": "parked_work", "session": "s1"}),
        json!({"parked": true}),
    );
    let lose = json!({"control": {"op": "lose_worker", "session": "s1"}});
    let mut all = vec![start("s1", "$s"), pump(), lose, pump(), parked.clone()];
    all.extend(steps);
    all.extend([parked, poll()]);
    all
}

/// 016c's round-4 F1: the witness of a `parked_work` read on h ends only on a mandatory event of the session on h, as
/// `event_fence` reads it. The operation `$new` is on h2, so its `Completed` on h ends no witness on h.
#[test]
fn a_completed_on_another_handle_ends_no_parked_witness() {
    let on_h2 = json!({"begin": {"op": {"Start": {"id": "s1"}}, "handle": "h2"}, "bind": "$new"});
    let completed_on_h = wait_for(json!({"Completed": {"op": "$new"}}));
    assert_eq!(r88(json!(parked_twice(vec![on_h2, completed_on_h]))), [8]);
}

/// The route sibling (017f, 016c; the steward's round-5 scope): a route event on h names the session that the route
/// variable is bound to on h at that step, never the latest attach on any handle.
#[test]
fn a_route_event_names_its_session_on_its_own_handle_for_a_parked_witness() {
    let attach = |session: &str, handle: &str| json!({"attach_route": {"route": "r1", "session": session, "client": "c1", "handle": handle}});
    // 016c's case: on h, `$r1` is s2's; a later attach of `$r1` to s1 on h2 does not make h's `RouteResumed` one of s1.
    let mut steps = vec![attach("s2", "h")];
    steps.extend(parked_twice(vec![
        attach("s1", "h2"),
        wait_for(json!({"RouteResumed": {"route": "$r1"}})),
    ]));
    assert_eq!(r88(json!(steps)), [9]);
    // `$r1` is attached on h2 only: its `RouteClosed` on h names no session on h.
    let closed = wait_for(json!({"RouteClosed": {"route": "$r1"}}));
    assert_eq!(
        r88(json!(parked_twice(vec![
            attach("s1", "h2"),
            closed.clone()
        ]))),
        [8]
    );
    // The same event on the route's own handle ends the witness.
    assert_eq!(
        r88(json!(parked_twice(vec![attach("s1", "h"), closed]))),
        Vec::<usize>::new()
    );
}

#[test]
fn a_route_fence_names_its_handle_and_its_binding() {
    let attach = |handle: &str| json!({"attach_route": {"route": "r1", "session": "s1", "client": "c1", "handle": handle}});
    let detach = json!({"begin": {"op": {"Detach": {"route": "$r1", "reason": "Detached"}}}, "bind": "$det"});
    let closed = |handle: &str| json!({"pump_until": {"event": {"RouteClosed": {"route": "$r1"}}, "handle": handle}});
    let reports = |handle: &str| json!({"await_control": {"handle": handle, "control": {"op": "route_reports", "session": "s1", "route": "$r1"}, "expect": {"closed": true}}});
    let base = json!([attach("h"), detach, attach("h2"), pump(), poll()]);
    assert_eq!(r88(base.clone()), [4]);
    for fence in [closed("h"), reports("h")] {
        assert_eq!(
            r88(with(&base, 4, fence.clone())),
            Vec::<usize>::new(),
            "{fence}"
        );
    }
    // h2's route r1 is another route: its close does not prove h's Detach.
    for fence in [closed("h2"), reports("h2")] {
        assert_eq!(r88(with(&base, 4, fence.clone())), [5], "{fence}");
    }
}
