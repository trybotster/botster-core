//! The route driver against a subject that sends a malformed frame (design 5.4: every received frame is validated).

use botster_conformance::{Bindings, Deadline, StepDriver, StepError};
use botster_hub_conformance::route::{ClientModel, Framing, RouteDriver, RouteRead, RouteSubject};
use botster_route_codec::prelude::{
    stream_wrap, FrameBounds, InputRefused, Op, RefusalReason, ToClient,
};
use serde_json::{json, Value};

/// Sends one frame once: a `baseline_end` whose history is a number, which does not decode.
struct Malformed(Option<Vec<u8>>, Value);

impl RouteSubject for Malformed {
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
    fn control(&mut self, op: &str, _: &Value, _: Option<u32>) -> Result<Value, String> {
        Ok(if op == "pty_input" {
            self.1.clone()
        } else {
            Value::Null
        })
    }
    fn client_model(&self, _: &str) -> Option<Box<dyn ClientModel>> {
        None
    }
}

fn driver() -> RouteDriver<Malformed> {
    let mut frame = vec![0x05];
    frame.extend(br#"{"history":5}"#);
    let mut d = RouteDriver::new(Malformed(Some(stream_wrap(&frame)), Value::Null));
    let mut b = Bindings::new();
    d.exec(&json!({ "route_attach": {} }), &mut b, &Deadline::none())
        .unwrap();
    d
}

// Design 5.4: a malformed frame that arrives during a drain fails the step.
#[test]
fn drain_fails_on_a_malformed_frame() {
    let mut d = driver();
    let r = d.exec(
        &json!({ "route_drain": true }),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(matches!(r, Err(StepError::Failed { .. })), "{r:?}");
}

// Design 5.4: a model comparison never passes while an invalid frame waits.
#[test]
fn model_comparison_fails_on_a_malformed_frame() {
    let mut d = driver();
    let r = d.exec(
        &json!({ "route_expect_model_equal": true }),
        &mut Bindings::new(),
        &Deadline::none(),
    );
    assert!(r.is_err(), "{r:?}");
}

/// A driver whose subject reports the byte log `hex`, with route `r1` attached.
fn pty_driver(hex: &str) -> RouteDriver<Malformed> {
    let mut d = RouteDriver::new(Malformed(None, json!({ "bytes": { "$bytes_hex": hex } })));
    attach(&mut d, "r1");
    d
}

fn attach(d: &mut RouteDriver<Malformed>, route: &str) {
    d.exec(
        &json!({ "route_attach": {}, "route": route }),
        &mut Bindings::new(),
        &Deadline::none(),
    )
    .unwrap();
}

/// Sends `frames` on `route`, as the transcript's `route_send` steps do.
fn send(d: &mut RouteDriver<Malformed>, route: &str, frames: &[Value]) {
    for frame in frames {
        d.exec(
            &json!({ "route_send": frame, "route": route }),
            &mut Bindings::new(),
            &Deadline::none(),
        )
        .unwrap();
    }
}

fn paste(op: &str, hex: &str, require_bracketed: Option<bool>) -> Vec<Value> {
    let mut begin = json!({ "frame": "input", "kind": "paste_begin", "op": op });
    if let Some(required) = require_bracketed {
        begin["require_bracketed"] = json!(required);
    }
    vec![
        begin,
        json!({ "frame": "paste_chunk", "op": op, "bytes": { "$bytes_hex": hex } }),
        json!({ "frame": "input", "kind": "paste_commit", "op": op }),
    ]
}

fn bytes(op: &str, hex: &str) -> Value {
    json!({ "frame": "bytes", "op": op, "bytes": { "$bytes_hex": hex } })
}

fn pty_matches(d: &mut RouteDriver<Malformed>, parts: Value) -> bool {
    let mut b = Bindings::new();
    b.insert("n".into(), json!(3));
    d.exec(
        &json!({ "route_expect_pty": parts }),
        &mut b,
        &Deadline::none(),
    )
    .is_ok()
}

// BUILD.md architecture rule 2, steward ruling R-73: the bytes of an input are named by relation to the op that the transcript's
// own frames give them, in the byte log, and a mismatch fails.
#[test]
fn route_expect_pty_names_the_run_of_an_op_by_the_transcripts_frames() {
    let mut d = pty_driver("61010203ff");
    send(&mut d, "r1", &[bytes("1", "61")]);
    send(&mut d, "r1", &paste("4", "010203", Some(false)));
    let ok = json!([
        { "hex": "61" },
        { "written_by_op": "4", "len_var": "$n", "contains": [{ "hex": "0203" }] },
        { "written_without_op": true }
    ]);
    assert!(pty_matches(&mut d, ok));
    assert!(pty_matches(
        &mut d,
        json!([{ "written_by_op": "1" }, { "written_by_op": "4" }, { "written_without_op": true }])
    ));
    for bad in [
        json!([{ "hex": "61" }, { "written_by_op": "5" }, { "written_without_op": true }]),
        json!([{ "hex": "61" }, { "written_by_op": "4", "contains": [{ "hex": "0302" }] }, { "written_without_op": true }]),
        json!([{ "hex": "61" }, { "written_by_op": "4", "len_var": "$n" }]),
        json!([{ "hex": "61" }, { "written_by_op": "4", "len_var": "$n" }, { "written_without_op": true }, { "hex": "00" }]),
        // One input of an op is one run: op 1 has one.
        json!([{ "written_by_op": "1" }, { "written_by_op": "1" }, { "hex": "010203ff" }]),
        // A run that no frame accounts for is not empty.
        json!([{ "hex": "61" }, { "written_without_op": true }, { "hex": "010203ff" }]),
    ] {
        assert!(!pty_matches(&mut d, bad.clone()), "{bad}");
    }
    let one_of = json!({ "one_of": [[{ "hex": "00" }], [{ "hex": "61" }, { "written_by_op": "4" }, { "written_without_op": true }]] });
    assert!(pty_matches(&mut d, one_of));
}

// R-73 evidence (follow-up 3): a paste whose bytes another writer splits fails `written_by_op`, with an exact other writer or a
// `contains` of the paste body; the literal bytes still show where the other writer's bytes went.
#[test]
fn a_paste_split_by_another_writer_fails_written_by_op() {
    let mut d = pty_driver("41417a7a4242");
    send(&mut d, "r1", &paste("1", "41414242", None));
    send(&mut d, "r1", &[bytes("2", "7a7a")]);
    for split in [
        json!([{ "written_by_op": "1" }, { "written_by_op": "2" }]),
        json!([{ "written_by_op": "2" }, { "written_by_op": "1" }]),
        json!([{ "written_by_op": "1", "contains": [{ "hex": "41414242" }] }, { "written_without_op": true }]),
        json!([{ "hex": "4141" }, { "written_by_op": "2" }, { "written_by_op": "1", "contains": [{ "hex": "41414242" }] }]),
    ] {
        assert!(!pty_matches(&mut d, split.clone()), "{split}");
    }
    assert!(pty_matches(
        &mut d,
        json!([{ "hex": "4141" }, { "written_by_op": "2" }, { "hex": "4242" }])
    ));
    let mut whole = pty_driver("7a7a41414242");
    send(&mut whole, "r1", &paste("1", "41414242", None));
    send(&mut whole, "r1", &[bytes("2", "7a7a")]);
    assert!(pty_matches(
        &mut whole,
        json!([{ "written_by_op": "2" }, { "written_by_op": "1" }])
    ));
}

// R-73 follow-up 1: a paste's bytes are its encoder's (Codec 5.1A), so a paste run is not empty and checks only its position,
// its length bound and its listed bytes, with or without the bracket markers. An aborted paste wrote nothing.
#[test]
fn a_paste_run_is_the_encoders_bytes_and_an_aborted_paste_wrote_nothing() {
    let wrapped = "1b5b3230307e41421b5b3230317e";
    for (log, parts, ok) in [
        (
            wrapped,
            json!([{ "written_by_op": "3", "contains": [{ "hex": "4142" }] }]),
            true,
        ),
        (
            "4142",
            json!([{ "written_by_op": "3", "contains": [{ "hex": "4142" }] }]),
            true,
        ),
        (
            "4142",
            json!([{ "written_by_op": "3", "contains": [{ "hex": "4241" }] }]),
            false,
        ),
        (
            "4142",
            json!([{ "written_by_op": "3", "len_var": "$n" }]),
            false,
        ),
        (
            "414243",
            json!([{ "written_by_op": "3", "len_var": "$n" }]),
            true,
        ),
        ("", json!([{ "written_by_op": "3" }]), false),
    ] {
        let mut d = pty_driver(log);
        send(&mut d, "r1", &paste("3", "4142", Some(true)));
        assert_eq!(pty_matches(&mut d, parts.clone()), ok, "{log} {parts}");
    }
    let mut d = pty_driver("4142");
    send(&mut d, "r1", &paste("3", "4142", None)[..2]);
    send(
        &mut d,
        "r1",
        &[json!({ "frame": "input", "kind": "paste_abort", "op": "3" })],
    );
    assert!(
        !pty_matches(&mut d, json!([{ "written_by_op": "3" }])),
        "aborted"
    );
}

// R-73, Codec TS-5: ops are per route, so the same op on two routes is two runs; a raw frame that decodes counts; op 0 is no op.
#[test]
fn the_runs_of_an_op_come_from_every_route_and_from_raw_frames() {
    let mut d = pty_driver("616263");
    attach(&mut d, "r2");
    send(&mut d, "r1", &[bytes("1", "61")]);
    send(&mut d, "r2", &[bytes("1", "62")]);
    // `bytes` op 9, `63`, as a raw frame: type 0x83, the op in 8 bytes, the payload.
    d.exec(
        &json!({ "route_send_raw": { "bytes_hex": "83000000000000000963" } }),
        &mut Bindings::new(),
        &Deadline::none(),
    )
    .unwrap();
    assert!(pty_matches(
        &mut d,
        json!([{ "written_by_op": "1" }, { "written_by_op": "1" }, { "written_by_op": "9" }])
    ));
    assert!(!pty_matches(
        &mut d,
        json!([{ "written_by_op": "1" }, { "written_by_op": "1" }, { "written_by_op": "1" }])
    ));
    // R-73 item 2: a run that an op's `bytes` frame accounts for is not written without an op, unless a part places that op.
    let mut owned = pty_driver("78");
    send(&mut owned, "r1", &[bytes("1", "78")]);
    assert!(
        !pty_matches(&mut owned, json!([{ "written_without_op": true }])),
        "op 1's bytes"
    );
    assert!(pty_matches(&mut owned, json!([{ "written_by_op": "1" }])));
    let mut both = pty_driver("7878");
    send(&mut both, "r1", &[bytes("1", "78")]);
    assert!(pty_matches(
        &mut both,
        json!([{ "written_without_op": true }, { "written_by_op": "1" }])
    ));
    let mut zero = pty_driver("61");
    send(&mut zero, "r1", &[bytes("0", "61")]);
    assert!(
        !pty_matches(&mut zero, json!([{ "written_by_op": "0" }])),
        "op 0 is no op"
    );
    assert!(pty_matches(
        &mut zero,
        json!([{ "written_without_op": true }])
    ));
}

#[test]
fn route_send_binds_the_size_of_the_frame_that_it_sent() {
    let mut d = driver();
    let mut b = Bindings::new();
    d.exec(
        &json!({"route_send": {"frame": "input", "kind": "focus", "op": "1", "focused": true}, "bind": "$size"}),
        &mut b,
        &Deadline::none(),
    )
    .unwrap();
    // The type byte plus the compact JSON body.
    let body = r#"{"kind":"focus","op":"1","focused":true}"#;
    assert_eq!(b.get("size"), Some(&json!(1 + body.len())));
    // Without `bind` nothing is bound.
    let mut b = Bindings::new();
    d.exec(
        &json!({"route_send": {"frame": "input", "kind": "focus", "op": "2", "focused": false}}),
        &mut b,
        &Deadline::none(),
    )
    .unwrap();
    assert!(b.is_empty());
}

/// The result of `route_expect_pty` with `parts`, as its error text.
fn pty_error(d: &mut RouteDriver<Malformed>, parts: Value) -> String {
    match d.exec(
        &json!({ "route_expect_pty": parts }),
        &mut Bindings::new(),
        &Deadline::none(),
    ) {
        Ok(_) => "matched".to_string(),
        Err(e) => format!("{e:?}"),
    }
}

// R-73: a `text` frame with an op is a run of its UTF-8 bytes, and with op 0 it is no op's run (Codec TS-5b).
#[test]
fn a_text_frame_is_a_run_of_its_op_and_op_0_is_no_op() {
    let mut owned = pty_driver("78");
    send(
        &mut owned,
        "r1",
        &[json!({ "frame": "text", "op": "3", "text": "x" })],
    );
    assert!(pty_matches(&mut owned, json!([{ "written_by_op": "3" }])));
    assert!(!pty_matches(
        &mut owned,
        json!([{ "written_without_op": true }])
    ));
    let mut zero = pty_driver("78");
    send(
        &mut zero,
        "r1",
        &[json!({ "frame": "text", "op": "0", "text": "x" })],
    );
    assert!(pty_matches(
        &mut zero,
        json!([{ "written_without_op": true }])
    ));
    assert!(!pty_matches(&mut zero, json!([{ "written_by_op": "3" }])));
}

// A mismatch names the furthest part that the search reached and its byte, or the bytes that no part covers.
#[test]
fn a_mismatch_names_the_part_and_the_byte_or_the_bytes_left_over() {
    let mut d = pty_driver("7878");
    let left = pty_error(&mut d, json!([{ "hex": "78" }]));
    assert!(left.contains("1 more bytes were written"), "{left}");
    let part = pty_error(&mut d, json!([{ "hex": "78" }, { "hex": "79" }]));
    assert!(
        part.contains("part 1 does not match the log at byte 1"),
        "{part}"
    );
    assert_eq!(pty_error(&mut d, json!([{ "hex": "7878" }])), "matched");
}

// Design 6.1: the search has an execution limit. A sequence whose placements exceed it is a bad step, not a mismatch.
#[test]
fn a_search_past_the_placement_limit_is_a_bad_step() {
    let mut d = pty_driver(&"61".repeat(30));
    let mut parts = vec![json!({ "written_without_op": true }); 15];
    parts.push(json!({ "hex": "62" }));
    let failure = pty_error(&mut d, Value::Array(parts));
    assert!(
        failure.contains("more than 1000000 placements of the parts"),
        "{failure}"
    );
}

/// A driver whose subject reports the byte log `hex` and, at its first read, sends `refusal` on route `r1`.
fn refusing_driver(
    hex: &str,
    op: u64,
    reason: RefusalReason,
    written_bytes: Option<u64>,
) -> RouteDriver<Malformed> {
    let refusal = ToClient::InputRefused(InputRefused {
        op: Some(Op(op)),
        reason,
        written_bytes,
        path: None,
    });
    let mut d = RouteDriver::new(Malformed(
        Some(stream_wrap(&refusal.encode())),
        json!({ "bytes": { "$bytes_hex": hex } }),
    ));
    attach(&mut d, "r1");
    d
}

/// The driver reads the route: it observes the refusal.
fn observe(d: &mut RouteDriver<Malformed>) {
    d.exec(
        &json!({ "route_drain": true }),
        &mut Bindings::new(),
        &Deadline::none(),
    )
    .unwrap();
}

// R-73 follow-up 2 (1): an observed refusal with zero effect (no `written_bytes`, or 0) removes the op's run, so the op's bytes
// are not that op's, and a written_without_op span may hold them.
#[test]
fn a_zero_effect_refusal_removes_the_run_of_its_op() {
    for (reason, written) in [
        (RefusalReason::NotWritable, None),
        (RefusalReason::Failed, Some(0)),
    ] {
        let mut d = refusing_driver("78", 1, reason.clone(), written);
        send(&mut d, "r1", &[bytes("1", "78")]);
        assert!(
            !pty_matches(&mut d, json!([{ "written_without_op": true }])),
            "before the refusal is read"
        );
        observe(&mut d);
        assert!(
            pty_matches(&mut d, json!([{ "written_without_op": true }])),
            "{reason:?}"
        );
        assert!(
            !pty_matches(&mut d, json!([{ "written_by_op": "1" }])),
            "{reason:?}"
        );
    }
    // The latest run of the op is the refused one.
    let mut two = refusing_driver("61", 1, RefusalReason::NotWritable, None);
    send(&mut two, "r1", &[bytes("1", "61"), bytes("1", "62")]);
    observe(&mut two);
    assert!(pty_matches(&mut two, json!([{ "written_by_op": "1" }])));
    // A refusal of another op, or on another route, changes nothing.
    let mut other = refusing_driver("78", 2, RefusalReason::NotWritable, None);
    send(&mut other, "r1", &[bytes("1", "78")]);
    observe(&mut other);
    assert!(pty_matches(&mut other, json!([{ "written_by_op": "1" }])));
    let mut elsewhere = refusing_driver("78", 1, RefusalReason::NotWritable, None);
    attach(&mut elsewhere, "r2");
    send(&mut elsewhere, "r2", &[bytes("1", "78")]);
    observe(&mut elsewhere);
    assert!(pty_matches(
        &mut elsewhere,
        json!([{ "written_by_op": "1" }])
    ));
}

// R-73 follow-up 2 (1): a partial write is a run of `written_bytes` when it is exact, and of unknown bytes when a `failed` or
// `session_ended` refusal has none.
#[test]
fn a_partial_refusal_is_a_run_of_its_written_bytes_or_of_unknown_bytes() {
    let mut exact = refusing_driver("4142", 1, RefusalReason::Failed, Some(2));
    send(&mut exact, "r1", &[bytes("1", "414243")]);
    assert!(!pty_matches(&mut exact, json!([{ "written_by_op": "1" }])));
    observe(&mut exact);
    assert!(pty_matches(&mut exact, json!([{ "written_by_op": "1" }])));
    // The written prefix is still the op's known bytes.
    let mut whole = refusing_driver("4142", 1, RefusalReason::SessionEnded, Some(2));
    send(&mut whole, "r1", &[bytes("1", "4142")]);
    observe(&mut whole);
    assert!(pty_matches(&mut whole, json!([{ "written_by_op": "1" }])));
    assert!(!pty_matches(
        &mut whole,
        json!([{ "written_without_op": true }])
    ));
    // A count longer than the frame's bytes is no prefix of them: the run is unknown bytes of that length.
    let mut longer = refusing_driver("4142", 1, RefusalReason::Failed, Some(2));
    send(&mut longer, "r1", &[bytes("1", "41")]);
    observe(&mut longer);
    assert!(pty_matches(&mut longer, json!([{ "written_by_op": "1" }])));
    // A paste's partial write has the exact length only.
    let mut paste_part = refusing_driver("1b5b", 4, RefusalReason::Failed, Some(2));
    send(&mut paste_part, "r1", &paste("4", "6162", None));
    observe(&mut paste_part);
    assert!(pty_matches(
        &mut paste_part,
        json!([{ "written_by_op": "4" }])
    ));
    let mut paste_long = refusing_driver("1b5b41", 4, RefusalReason::Failed, Some(2));
    send(&mut paste_long, "r1", &paste("4", "6162", None));
    observe(&mut paste_long);
    assert!(!pty_matches(
        &mut paste_long,
        json!([{ "written_by_op": "4" }])
    ));
    for reason in [RefusalReason::Failed, RefusalReason::SessionEnded] {
        let mut absent = refusing_driver("41", 1, reason.clone(), None);
        send(&mut absent, "r1", &[bytes("1", "414243")]);
        observe(&mut absent);
        assert!(
            pty_matches(&mut absent, json!([{ "written_by_op": "1" }])),
            "{reason:?}"
        );
        assert!(
            pty_matches(&mut absent, json!([{ "written_without_op": true }])),
            "{reason:?}"
        );
    }
}

// R-73 follow-up 2 (1): a refusal of a paste that is still open ends it: with zero effect it wrote nothing, and a partial write is
// a run of its length.
#[test]
fn a_refusal_of_an_open_paste_ends_it() {
    let begin = json!({ "frame": "input", "kind": "paste_begin", "op": "5" });
    let commit = json!({ "frame": "input", "kind": "paste_commit", "op": "5" });
    let mut zero = refusing_driver("61", 5, RefusalReason::NotReady, None);
    send(&mut zero, "r1", std::slice::from_ref(&begin));
    observe(&mut zero);
    send(&mut zero, "r1", std::slice::from_ref(&commit));
    assert!(!pty_matches(&mut zero, json!([{ "written_by_op": "5" }])));
    let mut part = refusing_driver("61", 5, RefusalReason::Failed, Some(1));
    send(&mut part, "r1", &[begin]);
    observe(&mut part);
    send(&mut part, "r1", &[commit]);
    assert!(pty_matches(&mut part, json!([{ "written_by_op": "5" }])));
}

// R-73 follow-up 2 (2): a paste's body is known bytes (IN-8 writes it verbatim), so a written_without_op span that holds an
// unplaced paste's body fails. A key's bytes are known only to the encoder (R-7): a span may cover them, the stated limit.
#[test]
fn an_unplaced_paste_body_fails_a_without_op_span_and_a_key_is_the_stated_limit() {
    let mut pasted = pty_driver("1b5b3230307e61621b5b3230317e");
    send(&mut pasted, "r1", &paste("2", "6162", None));
    assert!(!pty_matches(
        &mut pasted,
        json!([{ "written_without_op": true }])
    ));
    assert!(pty_matches(&mut pasted, json!([{ "written_by_op": "2" }])));
    let mut keyed = pty_driver("71");
    send(
        &mut keyed,
        "r1",
        &[
            json!({ "frame": "input", "kind": "key", "op": "3", "key": { "char": "q" }, "mods": [], "event": "press" }),
        ],
    );
    assert!(
        pty_matches(&mut keyed, json!([{ "written_without_op": true }])),
        "the stated limit"
    );
    assert!(pty_matches(&mut keyed, json!([{ "written_by_op": "3" }])));
}

/// R-85: whether `route_expect_pty` of `parts` (with `$cr` bound to a carriage return) fails with the R-85 transcript defect.
fn pty_0d_defect(d: &mut RouteDriver<Malformed>, parts: Value) -> bool {
    let mut b = Bindings::new();
    b.insert("cr".into(), json!("\r"));
    b.insert("ab".into(), json!("ab"));
    matches!(
        d.exec(&json!({ "route_expect_pty": parts }), &mut b, &Deadline::none()),
        Err(StepError::TranscriptDefect { rule, .. }) if rule == "R-85"
    )
}

// R-85: the bytes that a `route_expect_pty` compares with the input log hold no `0d`, whatever their source: a `hex` part, a
// bound `var`, a `contains` item, the known bytes of the run that a `written_by_op` part names (a `bytes` input, a committed
// paste's body), or, with a `written_without_op` part, the known bytes of every run (the part must not hold them). The log
// itself does not matter: the check fails the step before any match.
#[test]
fn a_route_read_that_compares_a_0d_byte_is_a_transcript_defect() {
    let mut d = pty_driver("610d62");
    send(&mut d, "r1", &[bytes("1", "610d")]);
    send(&mut d, "r1", &paste("2", "0d", Some(false)));
    send(&mut d, "r1", &[bytes("3", "62")]);
    for parts in [
        json!([{ "hex": "0d" }]),
        json!({ "one_of": [[{ "hex": "61" }], [{ "hex": "610d" }]] }),
        json!([{ "var": "$cr" }]),
        json!([{ "written_by_op": "3", "contains": [{ "hex": "0d" }] }]),
        json!([{ "written_without_op": true, "contains": [{ "var": "$cr" }] }]),
        json!([{ "written_by_op": "1" }]),
        json!([{ "written_by_op": "2" }]),
        json!([{ "written_without_op": true }]),
    ] {
        assert!(pty_0d_defect(&mut d, parts.clone()), "{parts}");
    }
    // Bytes without a 0d byte reach the match as before: here they do not match the log, and that is a plain failure.
    for parts in [
        json!([{ "hex": "a0d1" }]),
        json!([{ "var": "$ab" }]),
        json!([{ "written_by_op": "3" }]),
    ] {
        assert!(!pty_0d_defect(&mut d, parts.clone()), "{parts}");
    }
    // With no known 0d byte in any run, a `written_without_op` part is only a part.
    let mut clean = pty_driver("6162");
    send(&mut clean, "r1", &[bytes("1", "62")]);
    assert!(!pty_0d_defect(
        &mut clean,
        json!([{ "written_without_op": true }])
    ));
}

// R-85 (017f F2, 016c item 5 on #61): every sequence of `one_of` is checked before any one matches. Here the first sequence
// matches the log, and a later one holds a 0d byte: the step is still a transcript defect, on every tier.
#[test]
fn a_later_one_of_sequence_with_a_0d_byte_is_a_transcript_defect_when_an_earlier_one_matches() {
    let mut d = pty_driver("61");
    send(&mut d, "r1", &[bytes("1", "61")]);
    let mut b = Bindings::new();
    assert_eq!(
        d.exec(
            &json!({ "route_expect_pty": [{ "hex": "61" }] }),
            &mut b,
            &Deadline::none()
        ),
        Ok(())
    );
    assert!(pty_0d_defect(
        &mut d,
        json!({ "one_of": [[{ "hex": "61" }], [{ "hex": "0d" }]] })
    ));
    assert!(pty_0d_defect(
        &mut d,
        json!({ "one_of": [[{ "written_by_op": "1" }], [{ "var": "$cr" }]] })
    ));
}
