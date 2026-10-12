//! The route driver keeps a received frame typed and validates it through a stand-in with a short payload (design 5.4). These
//! tests pin what a step sees: a payload over its frame's bound is still rejected by every step that reads it, a frame at the
//! bound passes, and `route_expect` still matches the whole JSON form of a frame, payload included.
// StepError is the driver API's error type; its size is the runner's, as in the other driver tests.
#![allow(clippy::result_large_err)]

use botster_conformance::{Bindings, Deadline, StepDriver, StepError};
use botster_hub_conformance::route::{ClientModel, Framing, RouteDriver, RouteRead, RouteSubject};
use botster_route_codec::prelude::{hex_encode, stream_wrap, FrameBounds, Payload, ToClient};
use serde_json::{json, Value};

const MAX_FRAME: u64 = 4096;

/// Sends its bytes once, then nothing.
struct Once(Option<Vec<u8>>);

impl RouteSubject for Once {
    fn has_feature(&self, _: &str) -> bool {
        true
    }
    fn has_control(&self, _: &str) -> bool {
        true
    }
    fn configure(&mut self, _: &Value) -> Result<(), String> {
        Ok(())
    }
    fn attach(&mut self, _: &Value) -> Result<u32, String> {
        Ok(1)
    }
    fn framing(&self) -> Framing {
        Framing::Stream
    }
    fn bounds(&self) -> FrameBounds {
        FrameBounds {
            max_frame: MAX_FRAME,
            max_screen: MAX_FRAME,
            max_history: MAX_FRAME,
        }
    }
    fn max_chunk_bytes(&self) -> usize {
        64
    }
    fn write(&mut self, _: u32, _: &[u8]) {}
    fn read(&mut self, _: u32, _: usize, _: &Deadline) -> RouteRead {
        self.0.take().map_or(RouteRead::Empty, RouteRead::Bytes)
    }
    fn control(&mut self, _: &str, _: &Value, _: Option<u32>) -> Result<Value, String> {
        Ok(Value::Null)
    }
    fn client_model(&self, _: &str) -> Option<Box<dyn ClientModel>> {
        None
    }
}

fn output(len: usize) -> (Vec<u8>, Vec<u8>) {
    let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    let frame = ToClient::Output {
        payload: Payload::from(payload.clone()),
    };
    (payload, stream_wrap(&frame.encode()))
}

/// Runs one step after the attach, on a subject that sends `stream`.
fn step(stream: Vec<u8>, step: Value) -> Result<(), StepError> {
    let mut d = RouteDriver::new(Once(Some(stream)));
    let mut b = Bindings::default();
    d.exec(&json!({ "route_attach": {} }), &mut b, &Deadline::none())
        .unwrap();
    d.exec(&step, &mut b, &Deadline::none())
}

fn received(result: Result<(), StepError>) -> Value {
    match result {
        Err(StepError::Failed { received, .. }) => received,
        other => panic!("expected a failure, got {other:?}"),
    }
}

/// A frame of `MAX_FRAME` bytes (its type byte and a payload of `MAX_FRAME - 1`) passes; one more payload byte is over the
/// bound, and every step that reads the frame fails on the decoder's report.
#[test]
fn a_payload_over_the_frame_bound_is_still_rejected() {
    let at = MAX_FRAME as usize - 1;
    let (payload, stream) = output(at);
    let expect = json!({ "route_expect_output": { "bytes_hex": hex_encode(&payload) } });
    assert!(step(stream, expect).is_ok());

    let (payload, over) = output(at + 1);
    let got = received(step(
        over.clone(),
        json!({ "route_expect_output": { "bytes_hex": hex_encode(&payload) } }),
    ));
    assert!(got["frame"]["undecodable"].is_string(), "{got}");
    assert!(got["error"].is_string(), "{got}");
    let got = received(step(over.clone(), json!({ "route_drain": {} })));
    assert!(got["undecodable"].is_string(), "{got}");
    let got = received(step(over, json!({ "route_expect": { "frame": "output" } })));
    assert!(got["frame"]["undecodable"].is_string(), "{got}");
}

/// `route_expect` matches the whole JSON form of a received frame: a payload that differs in its last byte fails.
#[test]
fn route_expect_sees_the_whole_payload() {
    let (payload, stream) = output(300);
    let matcher = |hex: String| json!({ "route_expect": { "frame": "output", "payload": { "$bytes_hex": hex } } });
    assert!(step(stream.clone(), matcher(hex_encode(&payload))).is_ok());
    let mut other = payload.clone();
    *other.last_mut().unwrap() ^= 1;
    let got = received(step(stream, matcher(hex_encode(&other))));
    assert_eq!(
        got["frame"]["payload"]["$bytes_hex"],
        json!(hex_encode(&payload))
    );
}
