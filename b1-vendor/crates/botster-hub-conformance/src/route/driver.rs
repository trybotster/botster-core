//! The route step driver: a [`StepDriver`] over a [`RouteSubject`] (design 5.5).

use super::{ClientModel, Framing, RouteRead, RouteSubject};
use botster_conformance::matcher::matches;
use botster_conformance::{
    input_log_0d_defect, Bindings, Deadline, Direction, FrameSchema, NoSchemas, Poll,
    SchemaRegistry, SchemaResolver, StepDriver, StepError,
};
use botster_route_codec::prelude::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::sync::OnceLock;

const TO_CLIENT: &str = "route/to_client";
const TO_WORKER: &str = "route/to_worker";

/// The most bytes that one read asks for. A route frame arrives in several reads, so partial frames are exercised (Codec TS-3).
const READ_MAX: usize = 256;

/// The most steps of one internal loop (design 6.1: an execution limit, never a contract bound).
const MAX_STEPS: usize = 10_000;

fn registry() -> &'static SchemaRegistry {
    static REGISTRY: OnceLock<SchemaRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut r = SchemaRegistry::new();
        r.add(TO_CLIENT, &botster_route_codec::schema::to_client())
            .expect("to-client schema compiles");
        r.add(TO_WORKER, &botster_route_codec::schema::to_worker())
            .expect("to-worker schema compiles");
        r.set_frame_schema(Direction::Received, TO_CLIENT);
        r.set_frame_schema(Direction::Sent, TO_WORKER);
        r
    })
}

/// The schema resolver of the codec suite: frames that the transcript sends are `ToWorker` frames, and frames received are `ToClient` frames (design 5.4).
pub struct RouteSchemas;

impl SchemaResolver for RouteSchemas {
    fn validate(&self, schema_ref: &str, value: &Value) -> Result<(), String> {
        registry().validate(schema_ref, value)
    }
    fn violations(&self, schema_ref: &str, value: &Value) -> Result<Vec<String>, String> {
        // A transcript names the route schemas by their short names.
        let name = match schema_ref {
            "to_worker" | "to-worker" => TO_WORKER,
            "to_client" | "to-client" => TO_CLIENT,
            other => other,
        };
        registry().violations(name, value)
    }
    fn frame_schema(&self, direction: Direction, frame: &Value) -> FrameSchema {
        registry().frame_schema(direction, frame)
    }
}

enum Reader {
    Stream(StreamReader),
    Chunks(ChunkReader),
}

/// One item of a route's queue: a decoded frame that validated, kept typed, or the report of a frame that did not decode or
/// validate (`{"undecodable": ..}`, `{"invalid_frame": .., "error": ..}`). A frame's JSON form is built only where a step
/// needs it ([`frame_value`]): its payload's hex is large for a big frame.
enum Received {
    Frame(ToClient),
    Report(Value),
}

impl Received {
    fn into_value(self) -> Value {
        match self {
            Received::Frame(frame) => frame_value(&frame),
            Received::Report(value) => value,
        }
    }
}

/// The JSON form of a frame (design 5.4).
fn frame_value(frame: &ToClient) -> Value {
    serde_json::to_value(frame).expect("a frame serializes")
}

/// The frame with each payload cut to its first byte at most. The frozen schema states only the payload's form
/// (`{"$bytes_hex": "^([0-9a-f]{2})*$"}`, no length keyword; `the_payload_schema_states_only_the_hex_form` pins it), and the
/// encoder always gives that form, so the stand-in validates exactly when the frame does. Every other field is the frame's.
fn stand_in(frame: &ToClient) -> ToClient {
    let cut = |payload: &Payload| Payload(payload.0.slice(..payload.len().min(1)));
    match frame {
        ToClient::Screen { payload } => ToClient::Screen {
            payload: cut(payload),
        },
        ToClient::History { payload } => ToClient::History {
            payload: cut(payload),
        },
        ToClient::Output { payload } => ToClient::Output {
            payload: cut(payload),
        },
        ToClient::OutputDeflate { payload, .. } => ToClient::output_deflate(cut(payload)),
        other => other.clone(),
    }
}

/// Validates a received frame against a schema registry (design 5.4: the frozen schema) through its stand-in. On a failure the
/// error is the one of the whole frame, as a validation of the frame's own JSON form gives it.
fn validate_frame_in(registry: &SchemaRegistry, frame: &ToClient) -> Result<(), String> {
    let Err(cut) = registry.validate(TO_CLIENT, &frame_value(&stand_in(frame))) else {
        return Ok(());
    };
    Err(registry
        .validate(TO_CLIENT, &frame_value(frame))
        .err()
        .unwrap_or(cut))
}

/// The frame's `frame` tag.
fn frame_tag(frame: &ToClient) -> String {
    frame_value(&stand_in(frame))["frame"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The payload of a frame that has one, as the frame carries it.
fn payload_of(frame: &ToClient) -> Option<&Payload> {
    match frame {
        ToClient::Screen { payload }
        | ToClient::History { payload }
        | ToClient::Output { payload }
        | ToClient::OutputDeflate { payload, .. } => Some(payload),
        _ => None,
    }
}

/// A report item of the queue fails a step that reads it: it never validates (design 5.4).
fn report_failure(item: &Value) -> StepError {
    let error = registry()
        .validate(TO_CLIENT, item)
        .err()
        .unwrap_or_else(|| "a report of a received frame".to_string());
    failed(
        json!("a frame that validates"),
        json!({ "frame": item, "error": error }),
    )
}

struct RouteState {
    handle: u32,
    reader: Reader,
    frames: VecDeque<Received>,
    eof: Option<Value>,
    eof_given: bool,
    model: Option<Box<dyn ClientModel>>,
}

enum Pump {
    Progress,
    Nothing,
}

/// Drives a route endpoint with the codec step vocabulary.
pub struct RouteDriver<S: RouteSubject> {
    subject: S,
    routes: BTreeMap<String, RouteState>,
    /// Every frame that the driver sent, in order, for the relation parts of `route_expect_pty` (R-73).
    seen: Vec<Seen>,
}

fn failed(expected: Value, received: Value) -> StepError {
    StepError::Failed { expected, received }
}

fn bad(reason: impl Into<String>) -> StepError {
    failed(json!("a well-formed route step"), json!(reason.into()))
}

fn limit(which: &str) -> StepError {
    StepError::Inconclusive {
        limit: which.to_string(),
    }
}

fn route_name(step: &Value) -> String {
    step.get("route")
        .and_then(Value::as_str)
        .unwrap_or("r1")
        .to_string()
}

fn hex_of(spec: &Value) -> Result<Vec<u8>, StepError> {
    let text = spec
        .get("bytes_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("needs 'bytes_hex'"))?;
    hex_decode(text).map_err(bad)
}

/// The bytes that one `route_fill` writes (steward ruling R-47, item 3): byte k is 0x61 + (k mod 26), with k from 0 for
/// each fill. The pattern has no 0x0a byte, so the terminal's newline translation (R-48) cannot change it.
pub fn fill_pattern(bytes: usize) -> Vec<u8> {
    (0..bytes).map(|k| b'a' + (k % 26) as u8).collect()
}

/// One part of the expected output of `route_expect_output` with `parts`: where it starts in the joined bytes, and what it is
/// (a fill's name, or `bytes_hex`), for the report of a mismatch.
struct Part {
    start: usize,
    what: Value,
}

/// The expected bytes of `route_expect_output`: its `bytes_hex`, or its `parts` in order (R-47 item 3). A part is
/// `{"fill": "$name"}`, the pattern of the fill whose result `$name` holds (`{bytes: N}`, or the number N), or
/// `{"bytes_hex": ".."}`, with no other key. An unbound name, a fill that is not a name, or a part of another form (both keys
/// included) is a malformed step.
fn expected_output(spec: &Value, bindings: &Bindings) -> Result<(Vec<u8>, Vec<Part>), StepError> {
    let Some(parts) = spec.get("parts") else {
        return Ok((hex_of(spec)?, vec![]));
    };
    if spec.get("bytes_hex").is_some() {
        return Err(bad(
            "route_expect_output takes 'bytes_hex' or 'parts', not both",
        ));
    }
    let parts = parts.as_array().ok_or_else(|| bad("'parts' is a list"))?;
    let mut want = vec![];
    let mut layout = vec![];
    for part in parts {
        let start = want.len();
        // A part is exactly one of the two forms: a part with both keys, another key, or a fill that is not a name is
        // malformed, so no part of the expected value is ignored.
        let form = part
            .as_object()
            .filter(|keys| keys.len() == 1)
            .and_then(|keys| keys.keys().next());
        if form.map(String::as_str) == Some("fill") {
            let name = part["fill"]
                .as_str()
                .ok_or_else(|| bad(format!("a fill part names a variable: {part}")))?;
            let value = bindings
                .get(name.trim_start_matches('$'))
                .ok_or_else(|| bad(format!("unbound variable {name}")))?;
            let bytes = value
                .get("bytes")
                .unwrap_or(value)
                .as_u64()
                .ok_or_else(|| bad(format!("{name} holds no fill size: {value}")))?;
            let bytes =
                usize::try_from(bytes).map_err(|_| bad(format!("{name}: {bytes} bytes")))?;
            want.extend(fill_pattern(bytes));
        } else if form.map(String::as_str) == Some("bytes_hex") {
            want.extend(hex_of(part)?);
        } else {
            return Err(bad(format!(
                "a part is {{\"fill\": \"$name\"}} or {{\"bytes_hex\": ..}}, not {part}"
            )));
        }
        layout.push(Part {
            start,
            what: part.clone(),
        });
    }
    Ok((want, layout))
}

/// The report of output that differs from `want`, with `parts`: the first offset that differs, the part that holds it, the
/// offset in that part, and the expected and received byte there (or the end of the received bytes). The joined bytes of a
/// fill can be large, so the report never repeats them.
fn mismatch(want: &[u8], got: &[u8], layout: &[Part]) -> StepError {
    let at = want
        .iter()
        .zip(got)
        .position(|(a, b)| a != b)
        .unwrap_or(want.len().min(got.len()));
    let part = layout.iter().rposition(|p| p.start <= at);
    let byte = |b: &[u8]| b.get(at).map_or(json!(null), |b| json!(format!("{b:02x}")));
    failed(
        json!({
            "output_bytes": want.len(),
            "at": at,
            "part": part.map(|i| layout[i].what.clone()),
            "offset_in_part": part.map(|i| at - layout[i].start),
            "byte_hex": byte(want),
        }),
        json!({ "output_bytes": got.len(), "at": at, "byte_hex": byte(got) }),
    )
}

impl<S: RouteSubject> RouteDriver<S> {
    pub fn new(subject: S) -> Self {
        RouteDriver {
            subject,
            routes: BTreeMap::new(),
            seen: vec![],
        }
    }

    fn state(&mut self, name: &str) -> Result<&mut RouteState, StepError> {
        self.routes
            .get_mut(name)
            .ok_or_else(|| bad(format!("route '{name}' is not attached")))
    }

    /// Writes one client frame (`[type][body]`) with the binding's delimiting. On a chunked binding the frame is split to the route's chunk bound (Codec TR-3).
    fn send_frame(&mut self, name: &str, frame: &[u8]) -> Result<(), StepError> {
        let handle = self.state(name)?.handle;
        match self.subject.framing() {
            Framing::Stream => self.subject.write(handle, &stream_wrap(frame)),
            Framing::Chunked => {
                for message in chunk_wrap(frame, &[self.subject.max_chunk_bytes().max(1)]) {
                    self.subject.write(handle, &message);
                }
            }
        }
        Ok(())
    }

    /// Reads once from the subject and feeds the bytes into the route's reassembly.
    fn pump(&mut self, name: &str, max: usize, deadline: &Deadline) -> Result<Pump, StepError> {
        let bounds = self.subject.bounds();
        let handle = self.state(name)?.handle;
        let read = self.subject.read(handle, max, deadline);
        if deadline.expired() {
            return Err(limit("step_timeout"));
        }
        let mut decoded: Vec<Result<Decoded<ToClient>, DecodeError>> = vec![];
        let mut trailing: Option<DecodeError> = None;
        let route = self.state(name)?;
        match read {
            RouteRead::Bytes(bytes) => {
                let mut frames = vec![];
                match &mut route.reader {
                    Reader::Stream(r) => {
                        r.push(&bytes);
                        loop {
                            match r.next_frame(&bounds) {
                                Ok(Some(f)) => frames.push(f),
                                Ok(None) => break,
                                Err(e) => {
                                    trailing = Some(e);
                                    break;
                                }
                            }
                        }
                    }
                    Reader::Chunks(r) => match r.push_message(&bytes, &bounds) {
                        Ok(Some(f)) => frames.push(f),
                        Ok(None) => {}
                        Err(e) => trailing = Some(e),
                    },
                }
                decoded.extend(frames.iter().map(|f| ToClient::decode_shared(f, &bounds)));
            }
            RouteRead::Blocked | RouteRead::Empty => return Ok(Pump::Nothing),
            RouteRead::Exhausted => return Err(limit("scheduler_bound")),
            RouteRead::Eof { ended } => {
                // Codec TR-3: a frame that is incomplete when the channel closes is discarded.
                if let Reader::Chunks(r) = &mut route.reader {
                    r.discard_incomplete();
                }
                let mut v = json!({ "eof": true });
                if let Some(e) = ended {
                    v["ended"] = json!(e);
                }
                route.eof = Some(v);
                return Ok(Pump::Nothing);
            }
        }
        for result in decoded {
            match result {
                Ok(Decoded::Frame(frame)) => {
                    // Design 5.4: a received frame is validated before the client model sees it.
                    if let Err(e) = validate_frame_in(registry(), &frame) {
                        let value = frame_value(&frame);
                        self.state(name)?.frames.push_back(Received::Report(
                            json!({ "invalid_frame": value, "error": e }),
                        ));
                        continue;
                    }
                    // The client model is chosen from the format that `attached` negotiated (Codec TS-1, TS-4).
                    if let ToClient::Attached(a) = &frame {
                        let model = self.subject.client_model(&a.terminal_format);
                        self.state(name)?.model = model;
                    }
                    // R-73 follow-up 2 (1): a refusal that the driver observed changes the run of its op.
                    if let ToClient::InputRefused(refusal) = &frame {
                        self.seen.push(Seen::Refused {
                            route: name.to_string(),
                            refusal: refusal.clone(),
                        });
                    }
                    let route = self.state(name)?;
                    if let Some(m) = &mut route.model {
                        m.frame(&frame, bounds.max_frame);
                    }
                    route.frames.push_back(Received::Frame(frame));
                }
                Ok(_) => {}
                Err(e) => {
                    let route = self.state(name)?;
                    route
                        .frames
                        .push_back(Received::Report(json!({ "undecodable": e.to_string() })));
                }
            }
        }
        if let Some(e) = trailing {
            self.state(name)?
                .frames
                .push_back(Received::Report(json!({ "undecodable": e.to_string() })));
        }
        Ok(Pump::Progress)
    }

    /// One bounded step toward the next item: it returns an item or a close at once, and otherwise reads once (design 6.1).
    fn next_item(&mut self, name: &str, deadline: &Deadline) -> Poll {
        match self.next_received(name, deadline) {
            Ok(item) => Poll::Item(item.into_value()),
            Err(poll) => poll,
        }
    }

    /// [`Self::next_item`] with the item kept typed: `Ok` with the next item, else the poll to return.
    fn next_received(&mut self, name: &str, deadline: &Deadline) -> Result<Received, Poll> {
        let Ok(route) = self.state(name) else {
            return Err(Poll::Idle);
        };
        if let Some(item) = route.frames.pop_front() {
            return Ok(item);
        }
        if let Some(eof) = route.eof.clone() {
            if !route.eof_given {
                route.eof_given = true;
                return Err(Poll::Closed(eof));
            }
            return Err(Poll::Idle);
        }
        if deadline.expired() {
            return Err(Poll::TimedOut);
        }
        Err(match self.pump(name, READ_MAX, deadline) {
            Ok(Pump::Progress) => Poll::Pending,
            Ok(Pump::Nothing) => {
                let route = self.state(name).expect("route");
                if route.eof.is_some() {
                    Poll::Pending
                } else {
                    Poll::Idle
                }
            }
            Err(_) => Poll::TimedOut,
        })
    }

    /// Reads until the subject has nothing ready (bounded).
    fn drain(&mut self, name: &str, deadline: &Deadline) -> Result<(), StepError> {
        for _ in 0..MAX_STEPS {
            if matches!(self.pump(name, READ_MAX, deadline)?, Pump::Nothing) {
                // Design 5.4: a frame that does not decode or validate fails the step. Each report is one of those.
                let bad_frame = self.state(name)?.frames.iter().find_map(|item| match item {
                    Received::Report(f) => Some(f.clone()),
                    Received::Frame(_) => None,
                });
                return match bad_frame {
                    Some(f) => Err(failed(json!("a received frame that validates"), f)),
                    None => Ok(()),
                };
            }
        }
        Err(limit("max_polls"))
    }

    fn exec_step(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let name = route_name(step);
        let obj = step.as_object().ok_or_else(|| bad("a step is an object"))?;
        if let Some(spec) = obj.get("route_attach") {
            let handle = self.subject.attach(spec).map_err(bad)?;
            let reader = match self.subject.framing() {
                Framing::Stream => Reader::Stream(StreamReader::new()),
                Framing::Chunked => Reader::Chunks(ChunkReader::with_max_chunk(
                    self.subject.max_chunk_bytes() as u64,
                )),
            };
            self.routes.insert(
                name,
                RouteState {
                    handle,
                    reader,
                    frames: VecDeque::new(),
                    eof: None,
                    eof_given: false,
                    model: None,
                },
            );
            return Ok(());
        }
        if let Some(frame) = obj.get("route_send") {
            let typed: ToWorker = serde_json::from_value(frame.clone())
                .map_err(|e| bad(format!("route_send frame: {e}")))?;
            let encoded = typed.encode();
            // `"bind": "$size"` binds the size of the frame that was sent: its type byte plus its body (Codec TS-3), the binding's own
            // delimiting not counted. A reply's admission cost is that size (Core E4-1).
            if let Some(var) = obj.get("bind").and_then(Value::as_str) {
                bindings.insert(
                    var.trim_start_matches('$').to_string(),
                    json!(encoded.len()),
                );
            }
            self.send_frame(&name, &encoded)?;
            self.seen.push(Seen::Sent {
                route: name,
                frame: typed,
            });
            return Ok(());
        }
        if let Some(raw) = obj.get("route_send_raw") {
            // A raw frame that decodes is a frame of the transcript too: its op counts for `route_expect_pty` (R-73).
            let raw = hex_of(raw)?;
            self.send_frame(&name, &raw)?;
            if let Ok(Decoded::Frame(frame)) = ToWorker::decode(&raw, &self.subject.bounds()) {
                self.seen.push(Seen::Sent { route: name, frame });
            }
            return Ok(());
        }
        if let Some(raw) = obj.get("route_send_wire") {
            // Codec TS-3: the bytes go through unchanged, in one write.
            let handle = self.state(&name)?.handle;
            self.subject.write(handle, &hex_of(raw)?);
            return Ok(());
        }
        if let Some(invalid) = obj.get("send_invalid") {
            // A typed negative input (design 5.4): the engine proved that it fails its schema. It goes out as the JSON body of a 0x81 frame.
            let Value::Object(map) = invalid else {
                return Err(bad("send_invalid needs an object"));
            };
            if map.get("frame").and_then(Value::as_str) != Some("input") {
                return Err(StepError::Unsupported {
                    op: "send_invalid".into(),
                });
            }
            let mut body = map.clone();
            body.remove("frame");
            let mut frame = vec![TYPE_INPUT];
            frame.extend(serde_json::to_vec(&body).expect("a value serializes"));
            return self.send_frame(&name, &frame);
        }
        if let Some(n) = obj.get("route_read_bytes") {
            let n = n
                .get("bytes")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("route_read_bytes needs 'bytes'"))?;
            self.pump(&name, n as usize, deadline)?;
            return Ok(());
        }
        if let Some(matcher) = obj.get("route_expect") {
            return match self.next_until_received(&name, deadline)? {
                Some(item) => {
                    // A frame of the queue validated when it arrived; a report never validates.
                    let item = match item {
                        Received::Frame(frame) => frame_value(&frame),
                        Received::Report(item) => return Err(report_failure(&item)),
                    };
                    matches(matcher, Some(&item), bindings, &NoSchemas).map_err(|f| {
                        failed(
                            matcher.clone(),
                            json!({ "frame": item, "reason": f.describe() }),
                        )
                    })
                }
                None => Err(failed(matcher.clone(), json!("no frame"))),
            };
        }
        if let Some(spec) = obj.get("route_expect_output") {
            return self.expect_output(&name, spec, bindings, deadline);
        }
        if obj.contains_key("route_drain") {
            return self.drain(&name, deadline);
        }
        if let Some(parts) = obj.get("route_expect_pty") {
            return self.expect_pty(parts, bindings);
        }
        if obj.contains_key("route_expect_model_equal") {
            return self.expect_model_equal(&name, deadline);
        }
        if let Some(spec) = obj.get("route_check_path") {
            let spec = botster_conformance::substitute(spec, bindings)
                .map_err(|v| bad(format!("unbound variable {v}")))?;
            return check_path(&spec);
        }
        if let Some(control) = obj.get("control") {
            let op = control
                .get("op")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("control needs 'op'"))?;
            let handle = self.routes.get(&name).map(|r| r.handle);
            let result = self.subject.control(op, control, handle).map_err(|e| {
                if self.subject.has_control(op) {
                    bad(e)
                } else {
                    StepError::Unsupported { op: op.to_string() }
                }
            })?;
            if let Some(matcher) = obj.get("expect") {
                matches(matcher, Some(&result), bindings, &NoSchemas).map_err(|f| {
                    failed(
                        matcher.clone(),
                        json!({ "control": op, "result": result, "reason": f.describe() }),
                    )
                })?;
            }
            return Ok(());
        }
        Err(StepError::Unsupported {
            op: obj.keys().next().cloned().unwrap_or_default(),
        })
    }

    /// The next item, within the execution limits. `None` when the route is idle or closed.
    fn next_until_received(
        &mut self,
        name: &str,
        deadline: &Deadline,
    ) -> Result<Option<Received>, StepError> {
        for _ in 0..MAX_STEPS {
            match self.next_received(name, deadline) {
                Ok(item) => return Ok(Some(item)),
                Err(Poll::Pending) => {}
                Err(Poll::TimedOut) => return Err(limit("step_timeout")),
                Err(_) => return Ok(None),
            }
        }
        Err(limit("max_polls"))
    }

    /// The client model that loaded the baseline equals the worker's model (Codec TS-4). History is compared only when it is complete.
    fn expect_model_equal(&mut self, name: &str, deadline: &Deadline) -> Result<(), StepError> {
        self.drain(name, deadline)?;
        let route = self.state(name)?;
        // Every frame that the model saw must be valid (design 5.4): a frame of the queue validated when it arrived, and a
        // report never validates.
        if let Some(Received::Report(item)) = route
            .frames
            .iter()
            .find(|item| matches!(item, Received::Report(_)))
        {
            return Err(report_failure(item));
        }
        let client = route.model.as_ref().map(|m| m.canonical());
        let handle = route.handle;
        let mut worker = self
            .subject
            .control("model_state", &Value::Null, Some(handle))
            .map_err(bad)?;
        let Some(mut client) = client else {
            return Err(StepError::Unsupported {
                op: "client_model".into(),
            });
        };
        if client["history_complete"] != json!(true) {
            for v in [&mut client, &mut worker] {
                if let Some(o) = v.as_object_mut() {
                    o.remove("scrollback");
                }
            }
        }
        if let Some(o) = client.as_object_mut() {
            o.remove("history_complete");
        }
        if client == worker {
            Ok(())
        } else {
            Err(failed(worker, client))
        }
    }

    /// The bytes that reached the PTY match the parts, or one of the sequences of `{"one_of": [[parts], ..]}`.
    /// A part is `{"hex": ".."}`, `{"var": "$name"}` (the UTF-8 of a bound string), `{"written_without_op": true}` or
    /// `{"written_by_op": "<op>", "len_var"?: "$n", "contains"?: [{hex|var}, ..]}`. The last two name the bytes that the session wrote for
    /// an input by relation, never by a hand-written literal (BUILD.md, architecture rule 2), as runs of the one byte log that the
    /// transcript's own frames give (steward ruling R-73, [`runs`]): one input of that op, not empty, whose length equals the bound number
    /// and which contains the listed bytes in order. The parts take the log in order, and each input once. An `input_refused` that
    /// the driver observed before the step changes its op's run (R-73 follow-up 2 (1)). A `written_without_op` run holds no known
    /// bytes of an op's run that no part placed: a `bytes` or `text` input, or a paste's body (R-73 follow-up 2 (2)). The stated
    /// limit: key, mouse and focus bytes are known only to the encoder (R-7), so a `written_without_op` run may cover them.
    fn expect_pty(&mut self, parts: &Value, bindings: &Bindings) -> Result<(), StepError> {
        let log = self
            .subject
            .control("pty_input", &Value::Null, None)
            .map_err(bad)?;
        let sequences: Vec<&Value> = match parts.get("one_of").and_then(Value::as_array) {
            Some(many) => many.iter().collect(),
            None => vec![parts],
        };
        let runs = runs(&self.seen);
        let sequences = sequences
            .into_iter()
            .map(|seq| pty_parts(seq, bindings))
            .collect::<Result<Vec<_>, _>>()?;
        // R-85: every sequence is checked before any one matches, so the check does not depend on the log.
        for parts in &sequences {
            no_0d_expected(parts, &runs)?;
        }
        let mut last = String::new();
        for parts in &sequences {
            match pty_matches(parts, &log, &runs)? {
                Ok(()) => return Ok(()),
                Err(why) => last = why,
            }
        }
        Err(failed(parts.clone(), json!({ "pty": log, "reason": last })))
    }

    /// The `output` frames, inflated and joined, equal the expected bytes (Codec BP-2). With `"kind": "output_deflate"` or
    /// `"kind": "output"`, every output frame that carries those bytes is of that kind: a plain frame where compression was forced
    /// (Core A9-3), or a compressed one where none was negotiated, fails the step. Each compressed frame is inflated alone by the
    /// codec decoder (Codec A.4 0x0E: one payload, no shared state), so the check is independent of the worker's own compressor.
    fn expect_output(
        &mut self,
        name: &str,
        spec: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError> {
        let (want, layout) = expected_output(spec, bindings)?;
        // With `parts`, a report names the size and never repeats the bytes (a fill can be large).
        let expected = if layout.is_empty() {
            json!({ "output_hex": hex_encode(&want) })
        } else {
            json!({ "output_bytes": want.len() })
        };
        let ignore: Vec<Value> = spec
            .get("ignore")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let kind = spec.get("kind").and_then(Value::as_str);
        // `eligible_from` (with `kind: output_deflate`): the smallest payload that Core A9-3 lets the edge compress. A plain frame is
        // right below it and wrong at or above it, and a compressed frame is wrong below it. `min_deflate_frames`: at least that many
        // compressed frames were seen; with fewer, no eligible payload occurred and the step is inconclusive.
        let eligible_from = spec.get("eligible_from").and_then(Value::as_u64);
        let min_deflate = spec
            .get("min_deflate_frames")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut deflate_frames = 0u64;
        if let Some(k) = kind {
            if k != "output" && k != "output_deflate" {
                return Err(bad(format!(
                    "route_expect_output kind is output or output_deflate, not {k}"
                )));
            }
        }
        let mut got: Vec<u8> = vec![];
        while got.len() < want.len() {
            let Some(item) = self.next_until_received(name, deadline)? else {
                if !layout.is_empty() {
                    return Err(mismatch(&want, &got, &layout));
                }
                return Err(failed(
                    expected,
                    json!({ "so_far_hex": hex_encode(&got), "then": "no frame" }),
                ));
            };
            // A frame of the queue validated when it arrived (design 5.4); a report never validates. The payload is read
            // from the decoded frame, and its JSON form is built only for a report of the step.
            let frame = match item {
                Received::Frame(frame) => frame,
                Received::Report(item) => return Err(report_failure(&item)),
            };
            let have = frame_tag(&frame);
            if let Some(want_kind) = kind {
                let allowed = match (want_kind, eligible_from) {
                    // A plain frame below the eligibility floor is right; a compressed frame is checked below, after it is inflated.
                    ("output_deflate", Some(floor)) => {
                        have == "output_deflate"
                            || !have.starts_with("output")
                            || payload_of(&frame).map_or(0, |p| p.len() as u64) < floor
                    }
                    _ => !have.starts_with("output") || have == want_kind,
                };
                if !allowed {
                    return Err(failed(
                        json!({ "frame_kind": want_kind }),
                        frame_value(&frame),
                    ));
                }
            }
            match &frame {
                ToClient::Output { payload } => got.extend_from_slice(payload),
                ToClient::OutputDeflate { .. } => {
                    // The decoder inflated this stream when it decoded the frame (Codec A.4 0x0E); it is not inflated again.
                    let inflated = frame.inflated().ok_or_else(|| {
                        bad("an output_deflate frame that the decoder did not inflate".to_string())
                    })?;
                    if eligible_from.is_some_and(|floor| (inflated.len() as u64) < floor) {
                        return Err(failed(
                            json!({ "payload_at_least": eligible_from }),
                            frame_value(&frame),
                        ));
                    }
                    deflate_frames += 1;
                    got.extend_from_slice(inflated);
                }
                _ => {
                    let item = frame_value(&frame);
                    let skip = ignore.iter().any(|m| {
                        matches(m, Some(&item), &mut bindings.clone(), &NoSchemas).is_ok()
                    });
                    if !skip {
                        return Err(failed(expected, item));
                    }
                }
            }
        }
        if got != want {
            if !layout.is_empty() {
                return Err(mismatch(&want, &got, &layout));
            }
            return Err(failed(
                json!({ "output_hex": hex_encode(&want) }),
                json!({ "output_hex": hex_encode(&got) }),
            ));
        }
        if deflate_frames < min_deflate {
            // The bytes were delivered as expected (a mismatch failed above). The worker may partition the offered bytes into payloads that
            // are all below the eligibility floor, and the forced choice never acts on those: no eligible payload occurred, so the
            // premise of the step is open (design 6.1: inconclusive, never a contract failure). A plain frame at or above the floor is a
            // failure, and was reported above.
            return Err(StepError::Inconclusive {
                limit: "no_eligible_payload".into(),
            });
        }
        Ok(())
    }
}

fn literal(part: &Value, bindings: &Bindings) -> Result<Option<Vec<u8>>, StepError> {
    if let Some(h) = part.get("hex").and_then(Value::as_str) {
        return hex_decode(h).map(Some).map_err(bad);
    }
    if let Some(v) = part.get("var").and_then(Value::as_str) {
        let bound = bindings
            .get(v.trim_start_matches('$'))
            .and_then(Value::as_str)
            .ok_or_else(|| bad(format!("unbound string {v}")))?;
        return Ok(Some(bound.as_bytes().to_vec()));
    }
    Ok(None)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || hay.windows(needle.len()).any(|w| w == needle)
}

/// What the transcript's own frames say that one input of an op writes to the PTY (steward ruling R-73). One run is one admitted
/// transaction (Core AM-2: contiguous), never one PTY write.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Written {
    /// A `bytes` or `text` frame: exactly its bytes.
    Exact(Vec<u8>),
    /// A committed paste, a key, mouse or focus input, a terminal reply, a committed file, or a partial write of unknown bytes:
    /// bytes that an encoder owns (the paste encoder of Codec 5.1A, the terminal's key and mouse encoding, the worker's path
    /// paste), which the transcript cannot state (R-7, no hand-written encoder). The run is not empty; a part checks only its
    /// position, its length bound and its listed bytes (R-73 item 2 and its follow-up 1).
    Unknown {
        /// The bytes that the run holds verbatim: a paste's body (IN-8 writes the payload verbatim; R-73 follow-up 2 (2)).
        body: Option<Vec<u8>>,
        /// The run's length, when a refusal reported it exactly (R-73 follow-up 2 (1)).
        len: Option<usize>,
    },
}

impl Written {
    /// The bytes that a `written_without_op` span may not hold while no part placed this run (R-73 follow-up 2). Key, mouse and
    /// focus bytes are known only to the encoder (R-7), so a span can cover them: the stated limit.
    fn known(&self) -> Option<&[u8]> {
        match self {
            Written::Exact(bytes) => Some(bytes),
            Written::Unknown { body, .. } => body.as_deref(),
        }
    }
}

/// The input of one op that the transcript sent, as its frames make it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    route: String,
    op: Op,
    written: Written,
}

/// What the driver did or observed on a route, in order, for the relation parts of `route_expect_pty` (R-73).
enum Seen {
    /// A frame that the driver sent.
    Sent { route: String, frame: ToWorker },
    /// An `input_refused` that the driver received.
    Refused {
        route: String,
        refusal: InputRefused,
    },
}

/// The runs of the ops of the sent frames, in commit order. Op 0 is no op (Codec TS-5b), so its input is not a run. A paste or a
/// file that was aborted, or not yet committed, wrote nothing. A resize writes nothing to the PTY. An observed refusal changes the
/// run of its op (R-73 follow-up 2 (1)): with zero effect the op has no run; a partial write of exact length is a run of that
/// length; a `failed` or `session_ended` refusal without `written_bytes` leaves a run of unknown bytes.
fn runs(seen: &[Seen]) -> Vec<Run> {
    let mut out: Vec<Run> = vec![];
    // The pastes and the files that began and have not ended, by route and op, with a paste's body so far.
    let mut open: BTreeMap<(String, u64), Option<Vec<u8>>> = BTreeMap::new();
    for event in seen {
        let (route, frame) = match event {
            Seen::Sent { route, frame } => (route, frame),
            Seen::Refused { route, refusal } => {
                refuse(&mut out, &mut open, route, refusal);
                continue;
            }
        };
        let key = |op: Op| (route.clone(), op.0);
        let run = |op: Op, written: Written| Run {
            route: route.clone(),
            op,
            written,
        };
        let unknown = |op: Op| {
            run(
                op,
                Written::Unknown {
                    body: None,
                    len: None,
                },
            )
        };
        match frame {
            ToWorker::Bytes { op, bytes } if op.0 != 0 => {
                out.push(run(*op, Written::Exact(bytes.to_vec())))
            }
            ToWorker::Text { op, text } if op.0 != 0 => {
                out.push(run(*op, Written::Exact(text.as_bytes().to_vec())))
            }
            ToWorker::PasteChunk { op, bytes } => {
                if let Some(Some(body)) = open.get_mut(&key(*op)) {
                    body.extend_from_slice(bytes);
                }
            }
            ToWorker::Input(input) => match input {
                Input::PasteBegin { op, .. } => {
                    open.insert(key(*op), Some(vec![]));
                }
                Input::FileBegin { op, .. } => {
                    open.insert(key(*op), None);
                }
                Input::PasteCommit { op, .. } | Input::FileCommit { op } => {
                    if let Some(body) = open.remove(&key(*op)) {
                        out.push(run(
                            *op,
                            Written::Unknown {
                                body: body.filter(|b| !b.is_empty()),
                                len: None,
                            },
                        ));
                    }
                }
                Input::PasteAbort { op } | Input::FileAbort { op } => {
                    open.remove(&key(*op));
                }
                Input::Resize { .. } => {}
                Input::Key { op, .. }
                | Input::Mouse { op, .. }
                | Input::Focus { op, .. }
                | Input::ReplyTerminalQuery { op, .. } => {
                    out.extend(op.filter(|op| op.0 != 0).map(unknown));
                }
            },
            ToWorker::Bytes { .. } | ToWorker::Text { .. } | ToWorker::FileChunk { .. } => {}
        }
    }
    out
}

/// Applies an observed refusal to the latest run of its op on its route, or to its open paste or file (R-73 follow-up 2 (1)).
fn refuse(
    out: &mut Vec<Run>,
    open: &mut BTreeMap<(String, u64), Option<Vec<u8>>>,
    route: &str,
    refusal: &InputRefused,
) {
    let Some(op) = refusal.op.filter(|op| op.0 != 0) else {
        return;
    };
    let partial = matches!(
        refusal.reason,
        RefusalReason::Failed | RefusalReason::SessionEnded
    );
    // The length that the refused input wrote: `None` for zero effect, `Some(None)` for a write of unknown length.
    let wrote: Option<Option<usize>> = match refusal.written_bytes {
        Some(0) => None,
        Some(n) => Some(usize::try_from(n).ok()),
        None => partial.then_some(None),
    };
    let unknown = |len: Option<usize>| Written::Unknown { body: None, len };
    if open.remove(&(route.to_string(), op.0)).is_some() {
        if let Some(len) = wrote {
            out.push(Run {
                route: route.to_string(),
                op,
                written: unknown(len),
            });
        }
        return;
    }
    let Some(i) = out.iter().rposition(|r| r.route == route && r.op == op) else {
        return;
    };
    match wrote {
        None => {
            out.remove(i);
        }
        Some(len) => {
            let written = match (&out[i].written, len) {
                (Written::Exact(bytes), Some(n)) if n <= bytes.len() => {
                    Written::Exact(bytes[..n].to_vec())
                }
                (_, len) => unknown(len),
            };
            out[i].written = written;
        }
    }
}

/// One part of a sequence of `route_expect_pty`.
enum PtyPart {
    /// `{hex}` or `{var}`: exactly these bytes.
    Literal(Vec<u8>),
    /// `{written_by_op: X}`: the run of one input of op X (R-73).
    ByOp {
        op: Op,
        len: Option<usize>,
        contains: Vec<u8>,
    },
    /// `{written_without_op: true}`: a non-empty run that no op-carrying frame accounts for, such as key, mouse and focus input,
    /// or `bytes` and `text` with op 0 (R-73).
    WithoutOp {
        len: Option<usize>,
        contains: Vec<u8>,
    },
}

/// R-85 (R-61's input rule for computed bytes): the bytes that a sequence compares with the input log hold no `0d`, whatever their
/// source. These are a literal part (`hex`, or a bound `var`), the `contains` bytes of a relation part, and the known bytes of the
/// runs that a `written_by_op` part names: a `bytes` or `text` input, or a committed paste's body. A `written_without_op` part
/// compares the known bytes of every run of the route that no part placed, because it must not hold them (R-73 follow-up 2 (2)),
/// so with such a part every run's known bytes are checked. A key, mouse or focus run has no bytes that the transcript knows
/// (R-7), so it is not checked here; its `oracle_encode` value is a `var` or `hex` part.
fn no_0d_expected(parts: &[PtyPart], runs: &[Run]) -> Result<(), StepError> {
    let fail = |what: &str, bytes: &[u8]| -> Result<(), StepError> {
        match bytes.iter().position(|b| *b == 0x0d) {
            Some(at) => Err(input_log_0d_defect(what, &hex_encode(bytes), at)),
            None => Ok(()),
        }
    };
    for part in parts {
        match part {
            PtyPart::Literal(bytes) => fail("route_expect_pty literal part", bytes)?,
            PtyPart::ByOp { op, contains, .. } => {
                fail("route_expect_pty contains", contains)?;
                for run in runs.iter().filter(|run| run.op == *op) {
                    if let Some(known) = run.written.known() {
                        fail("route_expect_pty written_by_op run", known)?;
                    }
                }
            }
            PtyPart::WithoutOp { contains, .. } => {
                fail("route_expect_pty contains", contains)?;
                for known in runs.iter().filter_map(|run| run.written.known()) {
                    fail(
                        "route_expect_pty run that written_without_op excludes",
                        known,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn pty_part(part: &Value, bindings: &Bindings) -> Result<PtyPart, StepError> {
    if let Some(bytes) = literal(part, bindings)? {
        return Ok(PtyPart::Literal(bytes));
    }
    let len = match part.get("len_var").and_then(Value::as_str) {
        None => None,
        Some(n) => {
            let bound = bindings
                .get(n.trim_start_matches('$'))
                .and_then(Value::as_u64)
                .ok_or_else(|| bad(format!("len_var {n} is not a bound number")))?;
            Some(usize::try_from(bound).map_err(|_| bad(format!("len_var {n} is {bound}")))?)
        }
    };
    let mut contains = vec![];
    for p in part
        .get("contains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        contains.extend(literal(p, bindings)?.ok_or_else(|| bad("contains takes {hex} or {var}"))?);
    }
    if let Some(op) = part.get("written_by_op") {
        let op = op
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|op| *op != 0)
            .ok_or_else(|| {
                bad(format!(
                    "written_by_op names a decimal op that is not 0: {op}"
                ))
            })?;
        return Ok(PtyPart::ByOp {
            op: Op(op),
            len,
            contains,
        });
    }
    if part.get("written_without_op").is_some() {
        return Ok(PtyPart::WithoutOp { len, contains });
    }
    Err(bad(
        "a part is {hex}, {var}, {written_by_op} or {written_without_op}",
    ))
}

/// A run's bytes meet the part's bounds: not empty, the bound length, and the listed bytes in order.
fn run_fits(run: &[u8], len: Option<usize>, needle: &[u8]) -> bool {
    !run.is_empty() && len.is_none_or(|n| run.len() == n) && contains(run, needle)
}

/// The search for one placement of every part, each at the end of the one before it, that covers the whole log. A run of bytes
/// that the transcript cannot state can end anywhere, so the search tries each end. `left` bounds the work.
struct Search<'a> {
    log: &'a [u8],
    parts: &'a [PtyPart],
    runs: &'a [Run],
    used: Vec<bool>,
    /// Where each placed `written_without_op` run starts and ends.
    without: Vec<(usize, usize)>,
    /// The placements that the search may still try.
    left: usize,
    /// The furthest part that the search placed, and where the next one failed: the reason of a mismatch.
    furthest: (usize, usize),
}

/// Most placements that one sequence may try.
const MAX_PLACEMENTS: usize = 1_000_000;

impl Search<'_> {
    fn place(&mut self, part: usize, pos: usize) -> Result<bool, StepError> {
        self.left = self.left.checked_sub(1).ok_or_else(|| {
            bad(format!(
                "route_expect_pty: more than {MAX_PLACEMENTS} placements of the parts"
            ))
        })?;
        self.furthest = self.furthest.max((part, pos));
        let Some(current) = self.parts.get(part) else {
            return Ok(pos == self.log.len() && self.without_is_unaccounted());
        };
        let rest = &self.log[pos..];
        match current {
            PtyPart::Literal(want) => {
                Ok(rest.starts_with(want) && self.place(part + 1, pos + want.len())?)
            }
            PtyPart::WithoutOp { len, contains } => {
                for end in 1..=rest.len() {
                    if !run_fits(&rest[..end], *len, contains) {
                        continue;
                    }
                    self.without.push((pos, pos + end));
                    let placed = self.place(part + 1, pos + end)?;
                    self.without.pop();
                    if placed {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            PtyPart::ByOp { op, len, contains } => {
                for i in 0..self.runs.len() {
                    if self.used[i] || self.runs[i].op != *op {
                        continue;
                    }
                    self.used[i] = true;
                    let placed = self.place_run(i, part, pos, *len, contains)?;
                    self.used[i] = false;
                    if placed {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// R-73 item 2 and follow-up 2: a `written_without_op` run is one that no op-carrying frame accounts for. A placement fails
    /// when such a run holds the known bytes of an op's run that no part placed: a `bytes` or `text` input, or a paste's body.
    fn without_is_unaccounted(&self) -> bool {
        self.without.iter().all(|&(start, end)| {
            let run = &self.log[start..end];
            self.runs.iter().zip(&self.used).all(|(r, used)| {
                *used
                    || !r
                        .written
                        .known()
                        .is_some_and(|bytes| !bytes.is_empty() && contains(run, bytes))
            })
        })
    }

    /// Places the run `i` of the part `part` at `pos`.
    fn place_run(
        &mut self,
        i: usize,
        part: usize,
        pos: usize,
        len: Option<usize>,
        needle: &[u8],
    ) -> Result<bool, StepError> {
        let rest = &self.log[pos..];
        match &self.runs[i].written {
            Written::Exact(bytes) => Ok(rest.starts_with(bytes)
                && run_fits(bytes, len, needle)
                && self.place(part + 1, pos + bytes.len())?),
            Written::Unknown { len: exact, .. } => {
                for end in 1..=rest.len() {
                    if exact.is_none_or(|n| n == end)
                        && run_fits(&rest[..end], len, needle)
                        && self.place(part + 1, pos + end)?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

/// The parts of one sequence of `route_expect_pty`.
fn pty_parts(parts: &Value, bindings: &Bindings) -> Result<Vec<PtyPart>, StepError> {
    parts
        .as_array()
        .ok_or_else(|| bad("route_expect_pty takes an array"))?
        .iter()
        .map(|part| pty_part(part, bindings))
        .collect()
}

/// One sequence of parts against the byte log of the subject and the runs of the transcript's frames (R-73). The outer error is
/// a transcript error, the inner one a mismatch.
fn pty_matches(
    parts: &[PtyPart],
    log: &Value,
    runs: &[Run],
) -> Result<Result<(), String>, StepError> {
    let all = log["bytes"]["$bytes_hex"]
        .as_str()
        .ok_or_else(|| bad("pty_input gives no bytes"))
        .and_then(|h| hex_decode(h).map_err(bad))?;
    let mut search = Search {
        log: &all,
        parts,
        runs,
        used: vec![false; runs.len()],
        without: vec![],
        left: MAX_PLACEMENTS,
        furthest: (0, 0),
    };
    Ok(if search.place(0, 0)? {
        Ok(())
    } else {
        let (part, pos) = search.furthest;
        Err(if part == parts.len() {
            format!("{} more bytes were written", all.len() - pos)
        } else {
            format!("part {part} does not match the log at byte {pos}")
        })
    })
}

/// A path that the worker chose for an uploaded file satisfies the rules of Codec TS-5f: it is one component under the directory, made of
/// `A-Z a-z 0-9 . _ -`, not empty, within `max_component_bytes`, and never the path of another file (`differs_from`).
/// `contains` names text that the component must hold: the allowed characters of the supplied name, `file` for a name that reduces to
/// nothing, or the extension that fits.
fn check_path(spec: &Value) -> Result<(), StepError> {
    let path = spec
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("route_check_path needs a bound 'path'"))?;
    let under = spec.get("under").and_then(Value::as_str).unwrap_or("/");
    let limit = spec
        .get("max_component_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(255) as usize;
    let component = path.strip_prefix(&format!("{under}/")).unwrap_or("");
    let holds = spec
        .get("contains")
        .and_then(Value::as_str)
        .is_none_or(|text| component.contains(text));
    let distinct = spec
        .get("differs_from")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .all(|other| other.as_str() != Some(path));
    let ok = !component.is_empty()
        && !component.contains('/')
        && component.len() <= limit
        && component
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && holds
        && distinct;
    if ok {
        Ok(())
    } else {
        Err(failed(
            json!({ "under": under, "component": "A-Z a-z 0-9 . _ -, non-empty, at most max_component_bytes", "contains": spec.get("contains"), "differs_from": spec.get("differs_from") }),
            json!(path),
        ))
    }
}

impl<S: RouteSubject> StepDriver for RouteDriver<S> {
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
        let worker = setup.get("worker").cloned().unwrap_or(Value::Null);
        self.subject
            .configure(&worker)
            .map_err(|e| bad(format!("setup: {e}")))
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
        let name = scope.as_str().unwrap_or("r1").to_string();
        self.next_item(&name, deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD_FRAMES: [&str; 4] = ["screen", "history", "output", "output_deflate"];

    /// The stand-in is exact only while the frozen schema states the payload's form and nothing else: the payload property
    /// is a bare reference to `HexBytes`, `HexBytes` is the hex form with no length keyword, and only the frames that
    /// `stand_in` cuts carry one.
    #[test]
    fn the_payload_schema_states_only_the_hex_form() {
        let schema = botster_route_codec::schema::to_client();
        assert_eq!(
            schema["$defs"]["HexBytes"],
            json!({
                "type": "object",
                "properties": { "$bytes_hex": { "type": "string", "pattern": "^([0-9a-f]{2})*$" } },
                "required": ["$bytes_hex"],
                "additionalProperties": false
            })
        );
        let mut cut = vec![];
        for variant in schema["oneOf"].as_array().expect("variants") {
            let text = variant.to_string();
            if !text.contains("#/$defs/HexBytes") {
                continue;
            }
            assert_eq!(
                variant["properties"]["payload"],
                json!({ "$ref": "#/$defs/HexBytes" }),
                "{variant}"
            );
            assert_eq!(text.matches("HexBytes").count(), 1, "{variant}");
            cut.push(
                variant["properties"]["frame"]["const"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
        }
        cut.sort();
        let mut want = PAYLOAD_FRAMES.map(String::from).to_vec();
        want.sort();
        assert_eq!(cut, want);
        // No other definition refers to `HexBytes`.
        let defs = schema["$defs"].as_object().unwrap();
        assert!(defs
            .values()
            .all(|d| !d.to_string().contains("#/$defs/HexBytes")));
    }

    fn big(len: usize) -> Payload {
        Payload::from((0..len).map(|i| i as u8).collect::<Vec<u8>>())
    }

    /// The stand-in keeps every field but the payload, and cuts each payload to its first byte at most.
    #[test]
    fn the_stand_in_cuts_only_the_payload() {
        let attached: ToClient = serde_json::from_value(json!({
            "frame": "attached", "features": ["a"], "terminal_format": "vt",
            "limits": { "max_frame_bytes": 1, "max_screen_frame_bytes": 2, "max_history_page_bytes": 3,
                "max_paste_bytes": 4, "max_file_bytes": 5, "max_query_reply_bytes": 6,
                "query_client_deadline_ms": 7, "stall_deadline_ms": 8, "stall_close_after_ms": 9 }
        }))
        .unwrap();
        assert_eq!(stand_in(&attached), attached);
        for frame in [
            ToClient::Screen { payload: big(300) },
            ToClient::History { payload: big(300) },
            ToClient::Output { payload: big(300) },
            ToClient::output_deflate(big(300)),
        ] {
            let cut = stand_in(&frame);
            assert_eq!(frame_tag(&cut), frame_tag(&frame));
            assert_eq!(payload_of(&cut).unwrap()[..], [0u8][..]);
            assert!(PAYLOAD_FRAMES.contains(&frame_tag(&frame).as_str()));
        }
        let empty = ToClient::Output {
            payload: Payload::default(),
        };
        assert_eq!(stand_in(&empty), empty);
    }

    /// The frozen schema with one more rule that is not about a payload.
    fn tightened(edit: impl FnOnce(&mut Value)) -> SchemaRegistry {
        let mut schema = botster_route_codec::schema::to_client();
        edit(&mut schema);
        let mut registry = SchemaRegistry::new();
        registry.add(TO_CLIENT, &schema).unwrap();
        registry
    }

    /// A frame that a schema rejects for a reason other than its payload is still rejected, with the error that the
    /// validation of its whole JSON form gives; a frame that the schema accepts still passes.
    #[test]
    fn a_frame_that_the_schema_rejects_for_a_non_payload_reason_is_still_rejected() {
        // A field of a frame with no payload: `attached` with a terminal format over a length bound.
        let registry = tightened(|s| {
            s["$defs"]["Attached"]["properties"]["terminal_format"]["maxLength"] = json!(1);
        });
        let attached = |format: &str| -> ToClient {
            serde_json::from_value(json!({
                "frame": "attached", "features": [], "terminal_format": format,
                "limits": { "max_frame_bytes": 1, "max_screen_frame_bytes": 2, "max_history_page_bytes": 3,
                    "max_paste_bytes": 4, "max_file_bytes": 5, "max_query_reply_bytes": 6,
                    "query_client_deadline_ms": 7, "stall_deadline_ms": 8, "stall_close_after_ms": 9 }
            }))
            .unwrap()
        };
        let whole = |frame: &ToClient| registry.validate(TO_CLIENT, &frame_value(frame));
        let long = attached("vt");
        assert!(whole(&long).is_err());
        assert_eq!(validate_frame_in(&registry, &long), whole(&long));
        assert_eq!(validate_frame_in(&registry, &attached("v")), Ok(()));
        // The tag of a frame with a large payload: `output` with a rule on its tag.
        let registry = tightened(|s| {
            for v in s["oneOf"].as_array_mut().unwrap() {
                if v["properties"]["frame"]["const"] == json!("output") {
                    v["properties"]["frame"]["maxLength"] = json!(3);
                }
            }
        });
        let output = ToClient::Output {
            payload: big(70_000),
        };
        let error = registry.validate(TO_CLIENT, &frame_value(&output));
        assert!(error.is_err());
        assert_eq!(validate_frame_in(&registry, &output), error);
        let screen = ToClient::Screen {
            payload: big(70_000),
        };
        assert_eq!(validate_frame_in(&registry, &screen), Ok(()));
    }

    /// The frozen schema accepts each payload frame, of any size, as it accepts its stand-in.
    #[test]
    fn a_payload_frame_validates_as_its_whole_json_form_does() {
        for len in [0, 1, 2, 4097, 1 << 20] {
            for frame in [
                ToClient::Screen { payload: big(len) },
                ToClient::History { payload: big(len) },
                ToClient::Output { payload: big(len) },
                ToClient::output_deflate(big(len)),
            ] {
                let whole = registry().validate(TO_CLIENT, &frame_value(&frame));
                assert_eq!(validate_frame_in(registry(), &frame), whole, "{len}");
                assert_eq!(whole, Ok(()));
            }
        }
    }
}
