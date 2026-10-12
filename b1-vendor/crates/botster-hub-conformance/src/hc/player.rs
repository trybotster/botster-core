//! The client-subject player (HC A5-2, A5-3, R-1.4): the runner plays the Hub for a transcript with `subject: client`.
//!
//! The player holds no Hub logic. It sends the server frames that the transcript scripts, in the delimiting of the control stream (HC TR-1),
//! and it reads what the client writes. The client under test sits behind [`ClientSubject`], which a Phase 1 client repo implements through
//! its own transport adapter. A Phase 0 test never gives the player a client of its own; the self-tests of this crate use a spy.
//!
//! Steps (each takes `"on": "<connection>"`, default `c1`):
//!
//! | Step | Form |
//! |---|---|
//! | `send` | a server frame; the engine checks it against `ServerFrame` first |
//! | `send_raw` | `{"send_raw": {"bytes_hex": ".."}}`: one frame whose JSON bytes are given (the player adds the delimiter): a malformed or cut frame (HC EN-6, A.1) |
//! | `send_wire` | `{"send_wire": {"bytes_hex": ".."}}`: wire bytes, one delivery, unchanged. Half a frame, then `disconnect`, is the transport lost mid-frame (HC TS-9) |
//! | `disconnect` | the transport drops (the R-1.4 fault "drop the transport mid-route") |
//! | `client_act` | `{"client_act": <action>}`: one stimulus to the client, such as `{"demand": {"requests": 3}}` or `{"reconnect": true}` (HC BD-3, DS-4) |
//! | `client_expect` | `{"observation": {<name>: <arg>}, "because": ".."}`: one typed observation of the client |
//! | `client_requests` | `{"client_requests": {"at_most": n, "verb"?: "..", "max_frame_bytes"?: n}}`: everything the client wrote is a request of that verb, at most `n`, none over the byte bound (HC BD-3 fixes maximums only: a client may serialize, queue or refuse) |
//! | `client_idle` | `{"client_idle": {}}`: the client has run to quiet and wrote nothing that the script did not match (HC A5-2: an unexpected client frame fails the test) |
//!
//! The observations: `generic_screen {kind}` and `notice_logged {kind}` hold when the client answers `true` (HC DS-4); `response_delivered {id}` and `welcome_accepted {}` hold when the client answers `true`: it
//! took the frame as a normal answer, with its unknown optional members ignored (HC EN-6). A client that cannot make an observation answers `Err`, and the step is `unsupported_control`.
//! The engine's order constructs (`expect_eventually`, `expect_set`, `expect_quiet_until`) match the frames that the client wrote.

use super::schemas::HcSchemas;
use super::subject::Pump;
use botster_conformance::matcher::Bindings;
use botster_conformance::{substitute, SchemaResolver};
use botster_conformance::{Deadline, Poll, StepDriver, StepError};
use botster_route_codec::prelude::hex_decode;
use serde_json::{json, Value};
use std::collections::VecDeque;

/// The most client steps that one wait may take before the run is inconclusive (design 6.1: a client that always reports progress never hangs the runner).
const MAX_PUMPS: usize = 10_000;

/// A client under test (HC A5-2: it is driven through its own transport adapter, never through an internal shortcut).
pub trait ClientSubject {
    /// True when the client can make the observation (`generic_screen`, `notice_logged`, `requests_outstanding_at_most`, ...).
    fn has_observation(&self, name: &str) -> bool;
    /// Applies the `client` part of the transcript `setup` (for example the hand-built kinds of HC DS-4).
    fn configure(&mut self, client: &Value) -> Result<(), String>;
    /// The player delivers bytes to the client over the transport edge. One call is one delivery.
    fn deliver(&mut self, bytes: &[u8]);
    /// The transport drops: the client sees the close signal (HC TB-0 item 4).
    fn close(&mut self);
    /// One stimulus to the client (`client_act`): a demand for requests, watches or large frames, or a reconnect.
    fn act(&mut self, action: &Value) -> Result<(), String>;
    /// The client's scheduler runs one step.
    fn pump(&mut self) -> Pump;
    /// The bytes that the client has written since the last call.
    fn take_written(&mut self) -> Vec<u8>;
    /// One typed observation of the client. The answer is a bool or a number, as the table above says.
    fn observe(&mut self, observation: &Value) -> Result<Value, String>;
}

fn failed(expected: Value, received: Value) -> StepError {
    StepError::Failed { expected, received }
}

fn bad(reason: impl Into<String>) -> StepError {
    failed(
        json!("a well-formed client-subject step"),
        json!(reason.into()),
    )
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

/// The step driver of a `client` transcript.
pub struct ClientPlayer<C: ClientSubject> {
    client: C,
    buf: Vec<u8>,
    frames: VecDeque<(Value, usize)>,
    closed: bool,
    eof_given: bool,
}

impl<C: ClientSubject> ClientPlayer<C> {
    pub fn new(client: C) -> Self {
        ClientPlayer {
            client,
            buf: vec![],
            frames: VecDeque::new(),
            closed: false,
            eof_given: false,
        }
    }

    /// Parses the complete frames that the client wrote (HC TR-1).
    fn collect(&mut self) {
        let written = self.client.take_written();
        self.buf.extend(written);
        while self.buf.len() >= 4 {
            let len =
                u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
            if self.buf.len() < 4 + len {
                break;
            }
            let body: Vec<u8> = self.buf[4..4 + len].to_vec();
            self.buf.drain(..4 + len);
            let value = serde_json::from_slice(&body)
                .unwrap_or_else(|e| json!({"invalid_frame": e.to_string()}));
            self.frames.push_back((value, body.len()));
        }
    }

    /// Runs the client until it is quiet, collecting what it writes. Inconclusive at the deadline or after [`MAX_PUMPS`] steps.
    fn settle(&mut self, deadline: &Deadline) -> Result<(), StepError> {
        for _ in 0..MAX_PUMPS {
            if deadline.expired() {
                break;
            }
            self.collect();
            if self.client.pump() == Pump::Quiet {
                self.collect();
                return Ok(());
            }
        }
        Err(StepError::Inconclusive {
            limit: "the client did not come to quiet".into(),
        })
    }

    /// HC A5-2: a client frame that the script did not match fails the test.
    fn unmatched(&self) -> Result<(), StepError> {
        // Pending bytes are the start of a frame that the client did not finish: also not in the script.
        if !self.buf.is_empty() {
            return Err(failed(
                json!("no client output outside the script"),
                json!({ "unfinished_client_frame_bytes": self.buf.len() }),
            ));
        }
        match self.frames.front() {
            None => Ok(()),
            Some((frame, _)) => Err(failed(
                json!("no client frame outside the script"),
                json!({ "unexpected_client_frame": frame }),
            )),
        }
    }

    /// `client_requests`: the client runs to quiet; everything it wrote is a `req` (of `verb`, when given), at most `at_most` of them,
    /// and no frame is over `max_frame_bytes` (HC BD-3: a bound is a maximum; the script fixes no minimum, so a client may serialize,
    /// queue or refuse work).
    fn requests(&mut self, spec: &Value, deadline: &Deadline) -> Result<(), StepError> {
        self.settle(deadline)?;
        if !self.buf.is_empty() {
            return self.unmatched();
        }
        let most = spec
            .get("at_most")
            .and_then(Value::as_u64)
            .ok_or_else(|| bad("client_requests needs 'at_most'"))?;
        let max_bytes = spec.get("max_frame_bytes").and_then(Value::as_u64);
        let verb = spec.get("verb").and_then(Value::as_str);
        let taken: Vec<(Value, usize)> = self.frames.drain(..).collect();
        let schemas = HcSchemas::default();
        for (frame, size) in &taken {
            // Design 5.4: every received frame validates against its schema before it is matched.
            if let Err(e) = schemas.validate("hc/ClientFrame", frame) {
                return Err(failed(
                    json!("a client frame that validates against ClientFrame"),
                    json!({ "frame": frame, "error": e }),
                ));
            }
            let ok = frame["type"] == "req" && verb.is_none_or(|v| frame["verb"] == v);
            if !ok {
                return Err(failed(
                    json!({ "client_frames": "requests only", "verb": verb }),
                    json!({ "unexpected_client_frame": frame }),
                ));
            }
            if max_bytes.is_some_and(|m| *size as u64 > m) {
                return Err(failed(
                    json!({ "max_frame_bytes": max_bytes }),
                    json!({ "frame_bytes": size }),
                ));
            }
        }
        if taken.len() as u64 > most {
            return Err(failed(
                json!({ "requests_at_most": most }),
                json!({ "requests": taken.len() }),
            ));
        }
        Ok(())
    }

    fn observe(&mut self, spec: &Value, deadline: &Deadline) -> Result<(), StepError> {
        let observation = spec
            .get("observation")
            .and_then(Value::as_object)
            .ok_or_else(|| bad("client_expect needs an 'observation'"))?;
        let (name, arg) = observation
            .iter()
            .next()
            .ok_or_else(|| bad("an empty observation"))?;
        if !self.client.has_observation(name) {
            return Err(StepError::Unsupported { op: name.clone() });
        }
        // The client runs until it has nothing left to do, so the observation sees the settled state.
        self.settle(deadline)?;
        self.unmatched()?;
        let answer = self
            .client
            .observe(&json!({ name: arg }))
            .map_err(|_| StepError::Unsupported { op: name.clone() })?;
        let holds = match name.as_str() {
            "generic_screen" | "notice_logged" | "response_delivered" | "welcome_accepted" => {
                answer == json!(true)
            }
            other => {
                return Err(StepError::Unsupported {
                    op: other.to_string(),
                })
            }
        };
        if holds {
            Ok(())
        } else {
            Err(failed(json!({ name: arg }), json!({ "observed": answer })))
        }
    }
}

impl<C: ClientSubject> StepDriver for ClientPlayer<C> {
    fn has_feature(&self, _name: &str) -> bool {
        true
    }

    fn has_control(&self, op: &str) -> bool {
        self.client.has_observation(op)
    }

    fn setup(&mut self, setup: &Value) -> Result<(), StepError> {
        let client = setup.get("client").cloned().unwrap_or(Value::Null);
        self.client
            .configure(&client)
            .map_err(|e| bad(format!("setup: {e}")))
    }

    fn exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let step = &substitute(step, bindings).map_err(|v| bad(format!("unbound variable {v}")))?;
        let obj = step.as_object().ok_or_else(|| bad("a step is an object"))?;
        if self.closed && !obj.contains_key("client_expect") && !obj.contains_key("client_idle") {
            return Err(bad("the transport is closed"));
        }
        if let Some(frame) = obj.get("send") {
            self.client.deliver(&frame_bytes(
                &serde_json::to_vec(frame).map_err(|e| bad(e.to_string()))?,
            ));
            return Ok(());
        }
        if let Some(raw) = obj.get("send_raw") {
            self.client.deliver(&frame_bytes(&hex_of(raw)?));
            return Ok(());
        }
        if let Some(raw) = obj.get("send_wire") {
            self.client.deliver(&hex_of(raw)?);
            return Ok(());
        }
        if obj.contains_key("disconnect") {
            self.closed = true;
            self.client.close();
            return Ok(());
        }
        if let Some(action) = obj.get("client_act") {
            return self
                .client
                .act(action)
                .map_err(|e| bad(format!("client_act: {e}")));
        }
        if let Some(spec) = obj.get("client_requests") {
            return self.requests(spec, deadline);
        }
        if obj.contains_key("client_idle") {
            self.settle(deadline)?;
            return self.unmatched();
        }
        if let Some(spec) = obj.get("client_expect") {
            return self.observe(spec, deadline);
        }
        Err(bad(format!(
            "an unknown step: {}",
            obj.keys().next().map(String::as_str).unwrap_or("")
        )))
    }

    fn next(&mut self, _scope: &Value, deadline: &Deadline) -> Poll {
        if let Some((item, _)) = self.frames.pop_front() {
            return Poll::Item(item);
        }
        if deadline.expired() {
            return Poll::TimedOut;
        }
        // The client runs until it is quiet; each step may write a frame. A client that never comes to quiet ends the wait at the bound.
        for _ in 0..MAX_PUMPS {
            self.collect();
            if let Some((item, _)) = self.frames.pop_front() {
                return Poll::Item(item);
            }
            if deadline.expired() {
                return Poll::TimedOut;
            }
            if self.client.pump() == Pump::Quiet {
                return self.finish();
            }
        }
        Poll::TimedOut
    }
}

impl<C: ClientSubject> ClientPlayer<C> {
    /// The client is quiet: a frame it wrote in its last step, then the close once, then idle.
    fn finish(&mut self) -> Poll {
        self.collect();
        if let Some((item, _)) = self.frames.pop_front() {
            return Poll::Item(item);
        }
        if self.closed && !self.eof_given {
            self.eof_given = true;
            return Poll::Closed(json!({"eof": true}));
        }
        Poll::Idle
    }
}
