//! `route_expect_output` with `parts` (steward ruling R-47, item 3): a fill is the pattern of its bound size (byte k is 0x61 +
//! (k mod 26), with k from 0 for each fill), and the expected output is the parts in order. The subject below sends the bytes that
//! the test writes out, in output frames, so the expected pattern never comes from the driver's own function.
// StepError is the driver API's error type; its size is the runner's, as in the other driver tests.
#![allow(clippy::result_large_err)]

use botster_conformance::{Bindings, Deadline, StepDriver, StepError};
use botster_hub_conformance::route::{ClientModel, Framing, RouteDriver, RouteRead, RouteSubject};
use botster_route_codec::prelude::{stream_wrap, FrameBounds, Payload, ToClient};
use serde_json::{json, Value};

/// Sends its output frames once, then nothing.
struct Output(Option<Vec<u8>>);

impl RouteSubject for Output {
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
            max_frame: 4096,
            max_screen: 4096,
            max_history: 4096,
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

/// Runs one `route_expect_output` step whose subject sends `frames` (each one `output` frame) with these bindings.
fn run(frames: &[&[u8]], bindings: Value, spec: Value) -> Result<(), StepError> {
    let mut stream = vec![];
    for payload in frames {
        let frame = ToClient::Output {
            payload: Payload::from(*payload),
        };
        stream.extend(stream_wrap(&frame.encode()));
    }
    let mut d = RouteDriver::new(Output(Some(stream)));
    let mut b: Bindings = serde_json::from_value(bindings).unwrap();
    d.exec(&json!({ "route_attach": {} }), &mut b, &Deadline::none())
        .unwrap();
    d.exec(
        &json!({ "route_expect_output": spec }),
        &mut b,
        &Deadline::none(),
    )
}

#[test]
fn a_fill_alone_is_its_pattern_in_any_frames() {
    // 30 bytes: the alphabet, then k restarts its letters at 26.
    let result = run(
        &[b"abcdefghijklmnopq", b"rstuvwxyzabcd"],
        json!({ "f": { "bytes": 30 } }),
        json!({ "parts": [{ "fill": "$f" }] }),
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_fill_then_a_suffix_is_the_pattern_then_the_bytes() {
    let result = run(
        &[b"abc\r\n"],
        json!({ "f": { "bytes": 3 } }),
        json!({ "parts": [{ "fill": "$f" }, { "bytes_hex": "0d0a" }] }),
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn two_fills_each_start_their_pattern_at_a() {
    // A size bound as the number (an `expect` of `{bytes: "$g"}`) and one bound as the result both count.
    let parts = json!({ "parts": [{ "fill": "$f" }, { "fill": "$g" }] });
    let bindings = json!({ "f": { "bytes": 27 }, "g": 2 });
    let result = run(
        &[b"abcdefghijklmnopqrstuvwxyza", b"ab"],
        bindings.clone(),
        parts.clone(),
    );
    assert!(result.is_ok(), "{result:?}");
    // One pattern that runs on (k not restarted) is not two fills.
    let result = run(&[b"abcdefghijklmnopqrstuvwxyzabc"], bindings, parts);
    assert!(
        matches!(result, Err(StepError::Failed { .. })),
        "{result:?}"
    );
}

#[test]
fn an_unbound_fill_is_a_malformed_step() {
    let result = run(&[b"abc"], json!({}), json!({ "parts": [{ "fill": "$f" }] }));
    let Err(StepError::Failed { expected, received }) = result else {
        panic!("{result:?}");
    };
    assert_eq!(expected, json!("a well-formed route step"));
    assert!(
        received.to_string().contains("unbound variable $f"),
        "{received}"
    );
}

#[test]
fn a_mismatch_inside_a_fill_reports_its_offset_in_the_fill() {
    // The prefix is 2 bytes; the fill's byte 28 ('c') arrives as 'X', at offset 30 of the whole output.
    let result = run(
        &[b"xx", b"abcdefghijklmnopqrstuvwxyzabXd"],
        json!({ "f": { "bytes": 30 } }),
        json!({ "parts": [{ "bytes_hex": "7878" }, { "fill": "$f" }] }),
    );
    let Err(StepError::Failed { expected, received }) = result else {
        panic!("{result:?}");
    };
    assert_eq!(expected["at"], json!(30), "{expected}");
    assert_eq!(expected["part"], json!({ "fill": "$f" }), "{expected}");
    assert_eq!(expected["offset_in_part"], json!(28), "{expected}");
    assert_eq!(expected["byte_hex"], json!("63"), "{expected}");
    assert_eq!(received["byte_hex"], json!("58"), "{received}");
    assert_eq!(expected["output_bytes"], json!(32), "{expected}");
}

#[test]
fn a_part_is_exactly_one_form() {
    // Each of these would hide part of the expected value if the driver took one key of it: the output "abc" matches the
    // fill alone, so only the check of the form refuses them.
    for part in [
        json!({ "fill": "$f", "bytes_hex": "58595a" }),
        json!({ "fill": "$f", "extra": 1 }),
        json!({ "fill": 3 }),
        json!({ "fill": { "bytes": 3 } }),
        json!({}),
        json!("$f"),
    ] {
        let result = run(
            &[b"abc"],
            json!({ "f": { "bytes": 3 } }),
            json!({ "parts": [part.clone()] }),
        );
        let Err(StepError::Failed { expected, .. }) = &result else {
            panic!("{part}: {result:?}");
        };
        assert_eq!(expected, &json!("a well-formed route step"), "{part}");
    }
    // The well-formed fill of the same size passes.
    let result = run(
        &[b"abc"],
        json!({ "f": { "bytes": 3 } }),
        json!({ "parts": [{ "fill": "$f" }] }),
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn a_stream_that_ends_inside_the_parts_reports_the_offset_in_the_fill() {
    // The fill is 5 bytes; the subject sends 3 of them, then nothing.
    let result = run(
        &[b"abc"],
        json!({ "f": { "bytes": 5 } }),
        json!({ "parts": [{ "fill": "$f" }] }),
    );
    let Err(StepError::Failed { expected, received }) = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        expected,
        json!({ "output_bytes": 5, "at": 3, "part": { "fill": "$f" }, "offset_in_part": 3, "byte_hex": "64" })
    );
    assert_eq!(
        received,
        json!({ "output_bytes": 3, "at": 3, "byte_hex": null })
    );
}

#[test]
fn a_stream_that_ends_inside_plain_bytes_reports_no_frame() {
    let result = run(&[b"abc"], json!({}), json!({ "bytes_hex": "61626364" }));
    let Err(StepError::Failed { received, .. }) = result else {
        panic!("{result:?}");
    };
    assert_eq!(
        received,
        json!({ "so_far_hex": "616263", "then": "no frame" })
    );
}
