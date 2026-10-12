//! The HC step driver: a [`StepDriver`] over a [`HubSubject`] (design 5.5, 6.4).
//!
//! Steps (each takes `"on": "<connection>"`, default `c1`, where it sends or waits):
//!
//! | Step | Form |
//! |---|---|
//! | `connect` | `{"connect": {"as": "c1", "binding"?, "uid"?, "credential"?, "refused"?: true}}` |
//! | `send`, `send_frame` | a valid client frame; the engine checks it against `ClientFrame` first |
//! | `send_invalid` | `{"send_invalid": <value>, "violates": "hc/ClientFrame[@rule]"}`; goes out as one control frame. `violates: "to_worker"` is a route input |
//! | `send_raw` | `{"send_raw": {"bytes_hex": ".."}}`: one control frame whose JSON bytes are the given bytes (the driver adds the delimiter) |
//! | `send_wire` | `{"send_wire": {"bytes_hex": ".."}}`: complete wire bytes, one write, unchanged |
//! | `disconnect` | `{"disconnect": {}}`: the client closes its end |
//! | `advance_clock` | `{"advance_clock": {"ms": n}}` |
//! | `control` | `{"control": {"op": "..", ..args}, "expect"?: <matcher>}`; a `conn` argument names a connection |
//! | `route_connect` | `{"route_connect": {"as": "r1", "route_id", "route_token", "session"?, "limits"?, "uid"?, "binding"?}}`: opens a new connection and does the TB-L2 handshake (control framing) |
//! | `route_*` | the steps of `RouteDriver` (design 5.5, codec suite), on a route made by `route_connect` |
//!
//! The engine constructs take `"on": "<connection or route>"`. A closed connection gives `{"eof": true}`.
//! An expectation carries `"because": "<contract> <clause id>"`, or a list of them.

use super::subject::*;
use crate::route::{ClientModel, Framing, RouteDriver, RouteRead, RouteSubject};
use botster_conformance::matcher::matches;
use botster_conformance::{substitute, Bindings, Deadline, Poll, StepDriver, StepError};
use botster_route_codec::prelude::{hex_decode, FrameBounds};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

/// The most bytes that one read asks for. A frame arrives in several reads, so a partial frame is exercised (HC TR-1).
const READ_MAX: usize = 256;

/// The most scheduler steps that one wait may take before the run is inconclusive (design 6.1).
const MAX_PUMPS: usize = 10_000;

fn failed(expected: Value, received: Value) -> StepError {
    StepError::Failed { expected, received }
}

fn bad(reason: impl Into<String>) -> StepError {
    failed(json!("a well-formed HC step"), json!(reason.into()))
}

struct Conn {
    handle: u32,
    buf: Vec<u8>,
    frames: VecDeque<Value>,
    eof: bool,
    eof_given: bool,
    max_frame: usize,
}

/// A route connection of the Hub as a [`RouteSubject`], so that the route steps of the codec suite run unchanged on a route that a Hub handed off.
struct RouteAdapter<S: HubSubject> {
    subject: Rc<RefCell<S>>,
    bounds: FrameBounds,
    /// handle to (session, route_id)
    ends: BTreeMap<u32, (String, String)>,
}

impl<S: HubSubject> RouteSubject for RouteAdapter<S> {
    fn has_feature(&self, name: &str) -> bool {
        self.subject.borrow().has_feature(name)
    }
    fn has_control(&self, op: &str) -> bool {
        self.subject.borrow().has_control(op) || self.subject.borrow().has_control("worker")
    }
    fn configure(&mut self, _worker: &Value) -> Result<(), String> {
        Ok(())
    }
    fn attach(&mut self, spec: &Value) -> Result<u32, String> {
        let handle = spec["handle"].as_u64().ok_or("needs a handle")? as u32;
        if let Some(l) = spec.get("limits") {
            self.bounds = FrameBounds {
                max_frame: l["max_frame_bytes"]
                    .as_u64()
                    .unwrap_or(self.bounds.max_frame),
                max_screen: l["max_screen_frame_bytes"]
                    .as_u64()
                    .unwrap_or(self.bounds.max_screen),
                max_history: l["max_history_page_bytes"]
                    .as_u64()
                    .unwrap_or(self.bounds.max_history),
            };
        }
        self.ends.insert(
            handle,
            (
                spec["session"].as_str().unwrap_or("").to_string(),
                spec["route_id"].as_str().unwrap_or("").to_string(),
            ),
        );
        Ok(handle)
    }
    fn framing(&self) -> Framing {
        Framing::Stream
    }
    fn bounds(&self) -> FrameBounds {
        self.bounds
    }
    fn max_chunk_bytes(&self) -> usize {
        0
    }
    fn write(&mut self, route: u32, bytes: &[u8]) {
        self.subject.borrow_mut().write(route, bytes);
    }
    fn read(&mut self, route: u32, max: usize, deadline: &Deadline) -> RouteRead {
        // An empty read runs the Hub's injected scheduler (HC A5-1, A5-4): a route frame may wait there. The wait is bounded.
        for _ in 0..MAX_PUMPS {
            let read = self.subject.borrow_mut().read(route, max, deadline);
            match read {
                HubRead::Bytes(b) => return RouteRead::Bytes(b),
                HubRead::Eof => return RouteRead::Eof { ended: None },
                HubRead::Empty => {
                    if self.subject.borrow_mut().pump() == Pump::Quiet {
                        return RouteRead::Empty;
                    }
                    if deadline.expired() {
                        return RouteRead::Empty;
                    }
                }
            }
        }
        RouteRead::Exhausted
    }
    fn control(&mut self, op: &str, args: &Value, route: Option<u32>) -> Result<Value, String> {
        // A step with no route in hand (`pty_input`) means the one route that the Hub handed off.
        let one = if self.ends.len() == 1 {
            self.ends.values().next().cloned()
        } else {
            None
        };
        let (session, route_id) = route
            .and_then(|r| self.ends.get(&r).cloned())
            .or(one)
            .unwrap_or_default();
        self.subject.borrow_mut().control(
            "worker",
            &json!({"session": session, "route_id": route_id, "inner": op, "args": args}),
        )
    }
    fn client_model(&self, _terminal_format: &str) -> Option<Box<dyn ClientModel>> {
        None
    }
}

/// Drives a Hub with the HC step vocabulary.
pub struct HubDriver<S: HubSubject> {
    subject: Rc<RefCell<S>>,
    conns: BTreeMap<String, Conn>,
    routes: RouteDriver<RouteAdapter<S>>,
    route_names: Vec<String>,
    fixture: fn(&str) -> Option<Value>,
}

fn name_of(step: &Value) -> String {
    step.get("on")
        .and_then(Value::as_str)
        .unwrap_or("c1")
        .to_string()
}

fn frame_bytes(json_bytes: &[u8]) -> Vec<u8> {
    let mut out = (json_bytes.len() as u32).to_be_bytes().to_vec();
    out.extend(json_bytes);
    out
}

fn hex_of(spec: &Value) -> Result<Vec<u8>, StepError> {
    let text = spec
        .get("bytes_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("needs 'bytes_hex'"))?;
    hex_decode(text).map_err(bad)
}

impl<S: HubSubject> HubDriver<S> {
    /// `fixture` resolves a named fixture of `conformance/hc/fixtures` to its JSON.
    pub fn new(subject: S, fixture: fn(&str) -> Option<Value>) -> Self {
        let subject = Rc::new(RefCell::new(subject));
        let adapter = RouteAdapter {
            subject: subject.clone(),
            bounds: FrameBounds {
                max_frame: 4096,
                max_screen: 65536,
                max_history: 4096,
            },
            ends: BTreeMap::new(),
        };
        HubDriver {
            subject,
            conns: BTreeMap::new(),
            routes: RouteDriver::new(adapter),
            route_names: vec![],
            fixture,
        }
    }

    fn conn(&mut self, name: &str) -> Result<&mut Conn, StepError> {
        self.conns
            .get_mut(name)
            .ok_or_else(|| bad(format!("connection '{name}' is not open")))
    }

    fn handle_of(&self, name: &str) -> Result<u32, StepError> {
        self.conns
            .get(name)
            .map(|c| c.handle)
            .ok_or_else(|| bad(format!("connection '{name}' is not open")))
    }

    /// Reads once and parses the complete control frames (HC TR-1).
    fn pump(&mut self, name: &str, deadline: &Deadline) -> Result<bool, StepError> {
        let handle = self.handle_of(name)?;
        let read = self.subject.borrow_mut().read(handle, READ_MAX, deadline);
        let c = self.conn(name)?;
        match read {
            HubRead::Bytes(b) => {
                c.buf.extend(b);
                while c.buf.len() >= 4 {
                    let len = u32::from_be_bytes([c.buf[0], c.buf[1], c.buf[2], c.buf[3]]) as usize;
                    // HC EN-5: the Hub never sends a frame over `max_frame_bytes`.
                    if len > c.max_frame {
                        c.frames.push_back(json!({"invalid_frame": format!("a frame of {len} bytes is over max_frame_bytes")}));
                        c.buf.clear();
                        break;
                    }
                    if c.buf.len() < 4 + len {
                        break;
                    }
                    let body: Vec<u8> = c.buf[4..4 + len].to_vec();
                    c.buf.drain(..4 + len);
                    let v = serde_json::from_slice::<Value>(&body)
                        .unwrap_or_else(|e| json!({"invalid_frame": e.to_string()}));
                    if v["type"] == "welcome" {
                        if let Some(m) = v["limits"]["max_frame_bytes"].as_u64() {
                            c.max_frame = m as usize;
                        }
                    }
                    c.frames.push_back(v);
                }
                Ok(true)
            }
            // Nothing to read: the Hub's scheduler runs one step (HC A5-4), and the engine asks again while it makes progress.
            HubRead::Empty => Ok(self.subject.borrow_mut().pump() == Pump::Progress),
            HubRead::Eof => {
                c.eof = true;
                Ok(true)
            }
        }
    }

    fn next_item(&mut self, name: &str, deadline: &Deadline) -> Poll {
        let Ok(c) = self.conn(name) else {
            return Poll::Idle;
        };
        if let Some(item) = c.frames.pop_front() {
            return Poll::Item(item);
        }
        if c.eof {
            if !c.eof_given {
                c.eof_given = true;
                return Poll::Closed(json!({"eof": true}));
            }
            return Poll::Idle;
        }
        if deadline.expired() {
            return Poll::TimedOut;
        }
        match self.pump(name, deadline) {
            Ok(true) => Poll::Pending,
            Ok(false) => Poll::Idle,
            Err(_) => Poll::TimedOut,
        }
    }

    fn write_frame(&mut self, name: &str, json_bytes: &[u8]) -> Result<(), StepError> {
        let h = self.handle_of(name)?;
        self.subject.borrow_mut().write(h, &frame_bytes(json_bytes));
        Ok(())
    }

    /// Replaces a connection name in control arguments by its handle.
    fn map_conn(&self, args: &mut Value) -> Result<(), StepError> {
        if let Some(n) = args.get("conn").and_then(Value::as_str).map(str::to_string) {
            args["conn"] = json!(self.handle_of(&n)?);
        }
        Ok(())
    }

    fn exec_step(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let obj = step.as_object().ok_or_else(|| bad("a step is an object"))?;
        if let Some(spec) = obj.get("connect") {
            let name = spec
                .get("as")
                .and_then(Value::as_str)
                .unwrap_or("c1")
                .to_string();
            let cs = ConnectSpec {
                binding: spec
                    .get("binding")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                uid: spec.get("uid").and_then(Value::as_u64).map(|n| n as u32),
                credential: spec
                    .get("credential")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            };
            let handle = self.subject.borrow_mut().connect(&cs).map_err(bad)?;
            return match (handle, spec.get("refused") == Some(&json!(true))) {
                (Some(handle), false) => {
                    self.conns.insert(
                        name,
                        Conn {
                            handle,
                            buf: vec![],
                            frames: VecDeque::new(),
                            eof: false,
                            eof_given: false,
                            max_frame: 1 << 20,
                        },
                    );
                    Ok(())
                }
                (None, true) => Ok(()),
                (Some(_), true) => Err(failed(
                    json!("a refused connection"),
                    json!("the Hub accepted it"),
                )),
                (None, false) => Err(failed(
                    json!("an accepted connection"),
                    json!("nothing listens"),
                )),
            };
        }
        if let Some(frame) = obj
            .get("send")
            .or_else(|| obj.get("send_frame"))
            .or_else(|| obj.get("send_invalid"))
        {
            if obj
                .get("violates")
                .and_then(Value::as_str)
                .is_some_and(|v| v.starts_with("to_worker"))
            {
                return self.routes_exec(step, bindings, deadline);
            }
            let bytes = serde_json::to_vec(frame).map_err(|e| bad(e.to_string()))?;
            return self.write_frame(&name_of(step), &bytes);
        }
        if let Some(raw) = obj.get("send_raw") {
            return self.write_frame(&name_of(step), &hex_of(raw)?);
        }
        if let Some(raw) = obj.get("send_wire") {
            let h = self.handle_of(&name_of(step))?;
            self.subject.borrow_mut().write(h, &hex_of(raw)?);
            return Ok(());
        }
        if obj.contains_key("disconnect") {
            let h = self.handle_of(&name_of(step))?;
            self.subject.borrow_mut().close(h);
            return Ok(());
        }
        if let Some(spec) = obj.get("advance_clock") {
            let ms = spec
                .get("ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("advance_clock needs ms"))?;
            // A Hub that cannot inject its clock is `unsupported_control`, never a pass (design 6.2).
            return self.subject.borrow_mut().advance_clock(ms).map_err(|_| {
                StepError::Unsupported {
                    op: "advance_clock".into(),
                }
            });
        }
        if let Some(spec) = obj.get("route_connect") {
            return self.route_connect(spec, bindings);
        }
        if let Some(spec) = obj.get("control") {
            let mut spec =
                substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
            self.map_conn(&mut spec)?;
            let op = spec
                .get("op")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("control needs 'op'"))?
                .to_string();
            if !self.subject.borrow().has_control(&op) {
                return Err(StepError::Unsupported { op });
            }
            let result = self.subject.borrow_mut().control(&op, &spec).map_err(bad)?;
            if let Some(expect) = obj.get("expect") {
                if let Err(f) = matches(
                    expect,
                    Some(&result),
                    bindings,
                    &botster_conformance::NoSchemas,
                ) {
                    return Err(failed(
                        expect.clone(),
                        json!({"control": op, "result": result, "reason": f.describe()}),
                    ));
                }
            }
            return Ok(());
        }
        self.routes_exec(step, bindings, deadline)
    }

    fn routes_exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        self.routes.exec(step, bindings, deadline)
    }

    /// HC TB-L2: a new connection whose first frame is `{ "frame": "route_connect", route_id, route_token }` in control framing.
    fn route_connect(&mut self, spec: &Value, bindings: &mut Bindings) -> Result<(), StepError> {
        let spec = substitute(spec, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
        let name = spec
            .get("as")
            .and_then(Value::as_str)
            .unwrap_or("r1")
            .to_string();
        let cs = ConnectSpec {
            binding: spec
                .get("binding")
                .and_then(Value::as_str)
                .map(str::to_string),
            uid: spec.get("uid").and_then(Value::as_u64).map(|n| n as u32),
            credential: None,
        };
        let handle = self
            .subject
            .borrow_mut()
            .connect(&cs)
            .map_err(bad)?
            .ok_or_else(|| failed(json!("an accepted connection"), json!("nothing listens")))?;
        let hs = json!({"frame": "route_connect", "route_id": spec["route_id"], "route_token": spec["route_token"]});
        self.subject.borrow_mut().write(
            handle,
            &frame_bytes(&serde_json::to_vec(&hs).expect("serializes")),
        );
        if spec.get("handshake_only") == Some(&json!(true)) {
            // The connection stays a control-framed connection: the transcript reads the answer of the handshake (a typed `close`).
            self.conns.insert(
                name,
                Conn {
                    handle,
                    buf: vec![],
                    frames: VecDeque::new(),
                    eof: false,
                    eof_given: false,
                    max_frame: 1 << 20,
                },
            );
            return Ok(());
        }
        let attach = json!({"handle": handle, "session": spec.get("session"), "route_id": spec["route_id"], "limits": spec.get("limits")});
        let step = json!({"route": name, "route_attach": attach});
        self.route_names.push(name);
        let mut b = Bindings::new();
        self.routes.exec(&step, &mut b, &Deadline::none())
    }
}

impl<S: HubSubject> StepDriver for HubDriver<S> {
    fn has_feature(&self, name: &str) -> bool {
        self.subject.borrow().has_feature(name)
    }

    fn has_control(&self, op: &str) -> bool {
        self.subject.borrow().has_control(op)
    }

    fn setup(&mut self, setup: &Value) -> Result<(), StepError> {
        let mut hub = serde_json::Map::new();
        if let Some(name) = setup.get("fixture").and_then(Value::as_str) {
            let f = (self.fixture)(name).ok_or_else(|| bad(format!("no fixture '{name}'")))?;
            if let Some(m) = f.as_object() {
                hub.extend(m.clone());
            }
        }
        if let Some(m) = setup.get("hub").and_then(Value::as_object) {
            hub.extend(m.clone());
        }
        self.subject
            .borrow_mut()
            .configure(&Value::Object(hub))
            .map_err(|e| bad(format!("setup: {e}")))?;
        if setup.get("start") != Some(&json!(false)) {
            self.subject
                .borrow_mut()
                .start()
                .map_err(|e| bad(format!("start: {e}")))?;
        }
        Ok(())
    }

    fn exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        self.exec_step(step, bindings, deadline)
    }

    fn next(&mut self, scope: &Value, deadline: &Deadline) -> Poll {
        let name = scope.as_str().unwrap_or("c1").to_string();
        if self.route_names.contains(&name) {
            return self.routes.next(scope, deadline);
        }
        self.next_item(&name, deadline)
    }
}
