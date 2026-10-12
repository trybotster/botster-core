//! The HP step driver (design 5.5): `send_frame`, `send_frame_raw`, `send_wire`, `send_invalid`, `close_channel`, `advance_clock` and `control`, plus the order
//! constructs of the engine over the frames that the subject sends.
//!
//! | Step | Form |
//! |---|---|
//! | `send_frame` | `{"send_frame": {"lane": "control", "type": "Hello", "payload": {..}}, "conn"?: "main"}`: a valid frame, checked against its schema before it goes out. `"expect_error": {"kind": "backpressured"}` asserts that the write is refused (HP MD-3). |
//! | `send_invalid` | `{"send_invalid": {"lane", "type", "payload"}, "violates": "<Type>[@<rule>]"}`: a typed negative frame (design 5.4); it goes out as written |
//! | `send_frame_raw` | `{"send_frame_raw": {"lane": 3, "type": 32, "payload_hex": ".."}}` (or `"payload": <json>`, or `"payload_text": "..{{var}}.."`): a correct length field, any lane byte, any type byte, any payload bytes |
//! | `send_wire` | `{"send_wire": {"bytes_hex": ".."}}`: complete wire bytes (the length field included) in one write |
//! | `close_channel` | `{"close_channel": {}}`: the runner ends the connection |
//! | `advance_clock` | `{"advance_clock": {"ms": 5}}` |
//! | `control` | `{"control": {"op": "..", ..args}, "expect"?: <matcher>, "bind"?: {"var": "/json/pointer"}, "because"?: ..}` |
//! | `answer_host_call` | `{"answer_host_call": {"item": {"$item": ""}, "result": <PluginResult>}}`: the `ack` of an order step. When the item is a `HostCall` the Hub answers it with `result` (same epoch and `call_id`, effect `none`); any other item is left alone. A transcript uses it where the contract fixes the plugin's outcome and leaves open whether the host decides it locally or forwards the call and maps the Hub's refusal (steward ruling R-27). |
//!
//! Every step takes `"conn": "<name>"` (default `main`). The engine constructs take `"on": "<name>"`. A frame that the subject sends is `{"lane", "type", "payload"}`.
//! A frame that cannot be decoded is `{"wire_error": ".."}`; an unknown type is `{"unknown_type": n}`. The control `settle` makes the subject run until it has no more
//! work, and then a `{"settled": true}` item follows the frames of the connection that the control names (default `main`): it is the barrier of `expect_quiet_until`.
//! A closed connection gives `{"cause": ".."}` to `expect_closed`.

use crate::expand::expand;
use crate::subject::{Backpressured, HpSubject, Unsupported};
use crate::{lane_byte, lane_name};
use botster_conformance::{
    matcher::matches, substitute, Bindings, Deadline, MatchFailure, NoSchemas, Poll, StepDriver,
    StepError,
};
use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

pub struct HpDriver {
    subject: Box<dyn HpSubject>,
    fixtures: &'static include_dir::Dir<'static>,
    bufs: BTreeMap<String, Vec<u8>>,
    items: BTreeMap<String, VecDeque<Value>>,
    frame_bytes: u32,
}

fn failed(expected: Value, received: Value) -> StepError {
    StepError::Failed { expected, received }
}

fn bytes_of(hex: &str) -> Result<Vec<u8>, StepError> {
    if !hex.len().is_multiple_of(2) {
        return Err(failed(json!("an even number of hex digits"), json!(hex)));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| failed(json!("hex digits"), json!(hex)))
        })
        .collect()
}

impl HpDriver {
    pub fn new(subject: Box<dyn HpSubject>, fixtures: &'static include_dir::Dir<'static>) -> Self {
        HpDriver {
            subject,
            fixtures,
            bufs: BTreeMap::new(),
            items: BTreeMap::new(),
            frame_bytes: 65536,
        }
    }

    fn conn_of(step: &Value) -> String {
        step.get("conn")
            .and_then(Value::as_str)
            .unwrap_or("main")
            .to_string()
    }

    fn fixture(&self, name: &str) -> Option<Value> {
        let file = self.fixtures.get_file(format!("fixtures/{name}.json"))?;
        serde_json::from_str(file.contents_utf8()?).ok()
    }

    /// Reads every write that the subject has ready on `conn` and decodes it into items.
    fn drain(&mut self, conn: &str) {
        while let Some(bytes) = self.subject.read(conn) {
            self.bufs.entry(conn.to_string()).or_default().extend(bytes);
            self.decode(conn);
        }
    }

    fn decode(&mut self, conn: &str) {
        loop {
            let buf = self.bufs.entry(conn.to_string()).or_default();
            // The runner decodes everything that arrives: it checks no epoch (HP R-6 is the subject's duty), so a wrong-epoch frame from the subject is an item.
            match decode(buf, self.frame_bytes, None) {
                Ok(Decoded::Incomplete { .. }) => return,
                Ok(Decoded::Frame { frame, lane, used }) => {
                    let payload: Value =
                        serde_json::from_slice(&buf[6..used]).unwrap_or(Value::Null);
                    buf.drain(..used);
                    if let Frame::Hello(h) = &frame {
                        self.frame_bytes = h.limits.frame_bytes as u32;
                    }
                    let item = json!({"lane": lane_name(lane), "type": frame.frame_type().spec().name, "payload": payload});
                    self.items
                        .entry(conn.to_string())
                        .or_default()
                        .push_back(item);
                }
                Ok(
                    Decoded::IgnoredAdvisory { ty, used }
                    | Decoded::UnknownStateChanging { ty, used }
                    | Decoded::Stale { ty, used, .. },
                ) => {
                    buf.drain(..used);
                    self.items
                        .entry(conn.to_string())
                        .or_default()
                        .push_back(json!({"unknown_type": ty}));
                }
                Err(e) => {
                    buf.clear();
                    self.items
                        .entry(conn.to_string())
                        .or_default()
                        .push_back(json!({"wire_error": format!("{e:?}")}));
                    return;
                }
            }
        }
    }

    fn write(&mut self, step: &Value, bytes: &[u8]) -> Result<(), StepError> {
        let result = self.subject.write(&Self::conn_of(step), bytes);
        let expected = step
            .get("expect_error")
            .and_then(|e| e.get("kind"))
            .and_then(Value::as_str);
        match (result, expected) {
            (Ok(()), None) => Ok(()),
            (Err(Backpressured), Some("backpressured")) => Ok(()),
            (Ok(()), Some(kind)) => Err(failed(
                json!({"write_refused": kind}),
                json!("the write was accepted"),
            )),
            (Err(Backpressured), _) => Err(failed(
                json!("the write is accepted"),
                json!({"write_refused": "backpressured"}),
            )),
        }
    }

    fn frame_bytes_of(frame: &Value) -> Result<Vec<u8>, StepError> {
        let lane = frame
            .get("lane")
            .and_then(Value::as_str)
            .and_then(lane_byte)
            .ok_or_else(|| failed(json!("a lane name"), frame.clone()))?;
        let ty = frame
            .get("type")
            .and_then(Value::as_str)
            .and_then(|n| FRAMES.iter().find(|f| f.name == n))
            .ok_or_else(|| failed(json!("a frame type name"), frame.clone()))?
            .byte;
        let payload = serde_json::to_vec(frame.get("payload").unwrap_or(&Value::Null))
            .expect("a value serializes");
        encode_raw(lane, ty, &payload, u32::MAX)
            .map_err(|e| failed(json!("an encodable frame"), json!(format!("{e:?}"))))
    }
}

impl StepDriver for HpDriver {
    fn is_fake(&self) -> bool {
        self.subject.is_fake()
    }

    fn has_feature(&self, name: &str) -> bool {
        self.subject.has_feature(name)
    }

    fn has_control(&self, op: &str) -> bool {
        self.subject.has_control(op)
    }

    fn setup(&mut self, setup: &Value) -> Result<(), StepError> {
        let mut setup = setup.clone();
        if let Some(name) = setup
            .get("fixture")
            .and_then(Value::as_str)
            .map(String::from)
        {
            // The fixture is a language-neutral description of the plugin that a host runs (design 5.1). The fake interprets the body; a real host has the fixture by name.
            let body = self.fixture(&name).ok_or_else(|| {
                failed(
                    json!(format!("fixture '{name}' in conformance/hp/fixtures/")),
                    json!("missing"),
                )
            })?;
            let templates = self.fixture("frames").unwrap_or(Value::Null);
            setup["fixture"] = expand(&body, &templates)
                .map_err(|e| failed(json!("a valid fixture"), json!(e)))?;
            setup["fixture_name"] = json!(name);
        }
        if let Some(n) = setup
            .get("limits")
            .and_then(|l| l.get("frame_bytes"))
            .and_then(Value::as_u64)
        {
            self.frame_bytes = n as u32;
        }
        self.subject
            .configure(&setup)
            .map_err(|e| failed(json!("a setup the subject accepts"), json!(e)))
    }

    fn exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        _deadline: &Deadline,
    ) -> Result<(), StepError> {
        // Variables are substituted in every step, so a raw frame or a clock move can use a bound epoch or deadline. The `expect` of a control keeps its variables: it binds them.
        let mut step = step.clone();
        if let Some(obj) = step.as_object_mut() {
            let expect = obj.remove("expect");
            step = substitute(&Value::Object(obj.clone()), bindings)
                .map_err(|v| failed(json!("a bound variable"), json!(v)))?;
            if let (Some(e), Some(o)) = (expect, step.as_object_mut()) {
                o.insert("expect".into(), e);
            }
        }
        let step = &step;
        let Some(obj) = step.as_object() else {
            return Err(failed(json!("a step object"), step.clone()));
        };
        let conn = Self::conn_of(step);
        if let Some(frame) = obj.get("send_frame").or_else(|| obj.get("send_invalid")) {
            let bytes = Self::frame_bytes_of(frame)?;
            return self.write(step, &bytes);
        }
        if let Some(raw) = obj.get("send_frame_raw") {
            let (lane, ty) = (
                raw["lane"].as_u64().unwrap_or(0) as u8,
                raw["type"].as_u64().unwrap_or(0) as u8,
            );
            let payload = match (
                raw.get("payload"),
                raw.get("payload_text").and_then(Value::as_str),
            ) {
                (Some(v), _) => serde_json::to_vec(v).expect("a value serializes"),
                // Raw text with `{{var}}` places that take a bound value as JSON, so that a duplicate key or a bad number can sit next to a bound epoch.
                (None, Some(text)) => {
                    let mut text = text.to_string();
                    for (k, v) in bindings.iter() {
                        text = text.replace(&format!("{{{{{k}}}}}"), &v.to_string());
                    }
                    text.into_bytes()
                }
                (None, None) => bytes_of(raw["payload_hex"].as_str().unwrap_or(""))?,
            };
            let bytes = encode_raw(lane, ty, &payload, u32::MAX)
                .map_err(|e| failed(json!("an encodable frame"), json!(format!("{e:?}"))))?;
            return self.write(step, &bytes);
        }
        if let Some(wire) = obj.get("send_wire") {
            let bytes = bytes_of(wire["bytes_hex"].as_str().unwrap_or(""))?;
            return self.write(step, &bytes);
        }
        if let Some(spec) = obj.get("answer_host_call") {
            let item = &spec["item"];
            if item["type"] != "HostCall" {
                return Ok(());
            }
            let frame = json!({"lane": "call", "type": "HostResult", "payload": {
                "epoch": item["payload"]["epoch"], "call_id": item["payload"]["call_id"],
                "result": spec["result"], "effect": "none"}});
            let bytes = Self::frame_bytes_of(&frame)?;
            return self.write(step, &bytes);
        }
        if obj.contains_key("close_channel") {
            self.subject.close_channel(&conn);
            return Ok(());
        }
        if let Some(c) = obj.get("advance_clock") {
            self.subject.advance_clock(
                c.get("ms")
                    .and_then(Value::as_u64)
                    .or_else(|| c.as_u64())
                    .unwrap_or(0),
            );
            return Ok(());
        }
        if let Some(op) = obj.get("control") {
            let name = op
                .get("op")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let observed = self
                .subject
                .control(op)
                .map_err(|Unsupported(op)| StepError::Unsupported { op })?;
            if name == "settle" {
                // The subject ran until it had no more work: its frames come first, then the marker (design 5.3, the barrier).
                for c in self.subject.conns() {
                    self.drain(&c);
                }
                let marker_conn = op
                    .get("conn")
                    .and_then(Value::as_str)
                    .unwrap_or("main")
                    .to_string();
                self.items
                    .entry(marker_conn)
                    .or_default()
                    .push_back(json!({"settled": true}));
            }
            // `bind` maps a variable to a JSON pointer into what the control observed.
            if let Some(Value::Object(map)) = obj.get("bind") {
                for (var, pointer) in map {
                    let v = observed
                        .pointer(pointer.as_str().unwrap_or(""))
                        .cloned()
                        .ok_or_else(|| {
                            failed(json!(format!("a value at {pointer}")), observed.clone())
                        })?;
                    bindings.insert(var.clone(), v);
                }
            }
            if let Some(expect) = obj.get("expect") {
                return matches(expect, Some(&observed), bindings, &NoSchemas).map_err(
                    |f| match f {
                        MatchFailure::Mismatch { .. } => failed(expect.clone(), observed.clone()),
                        MatchFailure::BadMatcher { .. } => {
                            failed(json!("a well-formed matcher"), json!(f.describe()))
                        }
                    },
                );
            }
            return Ok(());
        }
        Err(failed(json!("an HP step (send_frame, send_invalid, send_frame_raw, send_wire, close_channel, advance_clock, control)"), step.clone()))
    }

    fn next(&mut self, scope: &Value, _deadline: &Deadline) -> Poll {
        let conn = scope.as_str().unwrap_or("main").to_string();
        loop {
            if let Some(item) = self.items.get_mut(&conn).and_then(VecDeque::pop_front) {
                return Poll::Item(item);
            }
            self.drain(&conn);
            if self.items.get(&conn).is_some_and(|q| !q.is_empty()) {
                continue;
            }
            return match self.subject.closed(&conn) {
                Some(cause) => Poll::Closed(json!({"cause": cause})),
                None => Poll::Idle,
            };
        }
    }
}
