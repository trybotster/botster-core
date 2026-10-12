//! Framing: the frame table (steward ruling R-4), the round trip of every frame, and one distinct error per WF violation.

mod common;
use botster_plugin_contract::prelude::*;
use common::*;
use serde_json::json;
use std::collections::BTreeSet;

const FB: u32 = 1 << 20;

fn decode_err(bytes: &[u8]) -> WireError {
    decode(bytes, FB, None).expect_err("a refusal")
}

#[test]
fn r4_type_bytes_are_unique_and_state_changing() {
    // R-4 (1), (2): unique across all lanes; all ten frames of section 2 are state-changing.
    let mut seen = BTreeSet::new();
    for f in FRAMES {
        assert!(seen.insert(f.byte), "{}: duplicate type byte", f.name);
        assert_eq!(
            FrameClass::of_byte(f.byte),
            FrameClass::StateChanging,
            "{}",
            f.name
        );
    }
    assert_eq!(FRAMES.len(), 10);
}

#[test]
fn r4_lane_bytes_follow_section_2() {
    // R-4 (3): control 0, invoke 1, call 2, bulk 3.
    let bytes: Vec<u8> = Lane::ALL.iter().map(|l| l.byte()).collect();
    assert_eq!(bytes, [0, 1, 2, 3]);
    for f in FRAMES {
        let want: &[Lane] = match f.name {
            "Hello" | "Ready" | "Activated" | "Shutdown" | "Fault" | "Cancel" => &[Lane::Control],
            "Invoke" | "Result" => &[Lane::Invoke],
            "HostCall" | "HostResult" => &[Lane::Call, Lane::Bulk],
            other => panic!("unexpected frame {other}"),
        };
        assert_eq!(f.lanes, want, "{}", f.name);
    }
}

#[test]
fn r4_pinned_type_bytes() {
    // R-4 (4): a published byte never changes. This list is the published table of botster_plugin_api 1.
    let pinned = [
        ("Hello", 0x01),
        ("Ready", 0x02),
        ("Activated", 0x03),
        ("Shutdown", 0x04),
        ("Fault", 0x05),
        ("Cancel", 0x06),
        ("Invoke", 0x10),
        ("Result", 0x11),
        ("HostCall", 0x20),
        ("HostResult", 0x21),
    ];
    for (name, byte) in pinned {
        assert_eq!(
            FRAMES.iter().find(|f| f.name == name).unwrap().byte,
            byte,
            "{name}"
        );
    }
}

#[test]
fn a_frame_without_an_epoch_is_refused() {
    let bytes = wire(0, FrameType::Activated.byte(), b"{}");
    assert_eq!(decode_err(&bytes), WireError::EpochMissing { ty: 0x03 });
}

#[test]
fn a_big_epoch_travels_as_a_decimal_string() {
    // WF-3.
    let a = Activated {
        epoch: Epoch((1 << 53) + 1),
    };
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["epoch"], "9007199254740993");
    assert_eq!(serde_json::from_value::<Activated>(v).unwrap(), a);
    let bytes = wire(0, 0x03, br#"{"epoch":9007199254740993}"#);
    assert!(matches!(
        decode_err(&bytes),
        WireError::IntegerOutOfRange { .. }
    ));
}

#[test]
fn wf1_len_over_the_limit_is_refused_before_the_body_is_read() {
    // Only the header is present: the refusal needs no body.
    let header = (FB + 1).to_le_bytes();
    assert_eq!(
        decode_err(&header),
        WireError::LenOverLimit {
            len: FB + 1,
            limit: FB
        }
    );
    assert_eq!(
        WireError::LenOverLimit { len: 0, limit: 0 }.disposition(),
        Disposition::EndsEpoch
    );
}

#[test]
fn wf1_len_below_two_is_refused() {
    assert_eq!(
        decode_err(&1u32.to_le_bytes()),
        WireError::LenTooShort { len: 1 }
    );
}

#[test]
fn incomplete_input_asks_for_more_bytes() {
    let bytes = encode(
        Lane::Control,
        &Frame::Activated(Activated { epoch: Epoch(1) }),
        FB,
    )
    .unwrap();
    assert_eq!(
        decode(&bytes[..2], FB, None).unwrap(),
        Decoded::Incomplete { needed: 2 }
    );
    assert_eq!(
        decode(&bytes[..bytes.len() - 3], FB, None).unwrap(),
        Decoded::Incomplete { needed: 3 }
    );
}

#[test]
fn wf2_invalid_utf8_ends_the_epoch() {
    let e = decode_err(&wire(0, 0x03, &[b'{', 0xff, 0xfe, b'}']));
    assert_eq!(e, WireError::InvalidUtf8);
    assert_eq!(e.disposition(), Disposition::EndsEpoch);
}

#[test]
fn wf2_non_object_ends_the_epoch() {
    for payload in [&b"[1]"[..], b"\"x\"", b"7", b"null"] {
        let e = decode_err(&wire(0, 0x03, payload));
        assert_eq!(e, WireError::NotAnObject, "{payload:?}");
        assert_eq!(e.disposition(), Disposition::EndsEpoch);
    }
}

#[test]
fn wf2_trailing_bytes_end_the_epoch() {
    let e = decode_err(&wire(0, 0x03, br#"{"epoch":1} x"#));
    assert_eq!(e, WireError::TrailingBytes);
    assert_eq!(e.disposition(), Disposition::EndsEpoch);
    assert_eq!(
        decode_err(&wire(0, 0x03, br#"{"epoch":1}{"epoch":2}"#)),
        WireError::TrailingBytes
    );
}

#[test]
fn wf2_duplicate_keys_are_refused_without_ending_the_epoch() {
    let e = decode_err(&wire(0, 0x03, br#"{"epoch":1,"epoch":2}"#));
    assert_eq!(
        e,
        WireError::DuplicateKey {
            key: "epoch".into()
        }
    );
    assert_eq!(e.disposition(), Disposition::InvalidRequest);
    // A duplicate key in a nested object is refused as well.
    let e = decode_err(&wire(
        0,
        0x11,
        br#"{"epoch":1,"request_id":1,"result":{"ok":true,"value":{"a":1,"a":2}}}"#,
    ));
    assert_eq!(e, WireError::DuplicateKey { key: "a".into() });
}

#[test]
fn invalid_json_is_its_own_refusal() {
    let e = decode_err(&wire(0, 0x03, b"{epoch"));
    assert_eq!(e, WireError::InvalidJson);
    assert_eq!(e.disposition(), Disposition::EndsEpoch);
}

#[test]
fn wf3_out_of_range_integers_are_refused() {
    let e = decode_err(&wire(
        1,
        0x11,
        br#"{"epoch":1,"request_id":1,"result":{"ok":true,"value":{"n":9007199254740993}}}"#,
    ));
    assert_eq!(
        e,
        WireError::IntegerOutOfRange {
            pointer: "/result/value/n".into()
        }
    );
    assert_eq!(e.disposition(), Disposition::InvalidRequest);
    // The limit itself is allowed, and so are negative values at the limit.
    decode(&wire(1, 0x11, br#"{"epoch":1,"request_id":1,"result":{"ok":true,"value":{"n":9007199254740992,"m":-9007199254740992}}}"#), FB, None).unwrap();
    // A big integer as a decimal string is fine.
    decode(
        &wire(
            1,
            0x11,
            br#"{"epoch":1,"request_id":"9007199254740993","result":{"ok":true,"value":null}}"#,
        ),
        FB,
        None,
    )
    .unwrap();
}

#[test]
fn wf3_the_encoder_refuses_an_out_of_range_integer() {
    let f = Frame::Result(ResultFrame {
        epoch: Epoch(1),
        request_id: WireId::Num(1),
        result: PluginResult::Ok(json!({"n": 18014398509481984u64})),
    });
    assert!(matches!(
        encode(Lane::Invoke, &f, FB),
        Err(EncodeError::IntegerOutOfRange { .. })
    ));
}

#[test]
fn wf4_null_where_the_schema_has_none_is_a_schema_refusal() {
    let e = decode_err(&wire(1, 0x10, br#"{"epoch":1,"request_id":1,"handler":null,"kind":"timer","payload":{},"caller":{"kind":"system"},"origin":"timer","descriptor_id":"d","deadline_ms":5}"#));
    assert!(matches!(e, WireError::Schema { ty: 0x10, .. }));
    assert_eq!(e.disposition(), Disposition::InvalidRequest);
}

#[test]
fn md2_the_encoder_refuses_an_oversize_frame_and_truncates_nothing() {
    let f = Frame::Hello(hello());
    let full = encode(Lane::Control, &f, FB).unwrap();
    let need = full.len() as u64 - 4;
    let e = encode(Lane::Control, &f, (need - 1) as u32).unwrap_err();
    assert_eq!(
        e,
        EncodeError::FrameTooLarge {
            len: need,
            limit: (need - 1) as u32
        }
    );
    encode(Lane::Control, &f, need as u32).unwrap();
}

/// A payload with an epoch and nothing else.
fn epoch_payload(epoch: u64) -> Vec<u8> {
    format!(r#"{{"epoch":{epoch}}}"#).into_bytes()
}

#[test]
fn r6_unknown_state_changing_type_with_the_current_epoch_gets_a_fault_outcome() {
    // R-6.2 (4): the outcome is for the receiver to answer with `Fault{unsupported_frame, type}`.
    let bytes = wire(0, 0x7F, &epoch_payload(7));
    assert_eq!(
        decode(&bytes, FB, Some(Epoch(7))).unwrap(),
        Decoded::UnknownStateChanging {
            ty: 0x7F,
            used: bytes.len()
        }
    );
}

#[test]
fn r6_unknown_advisory_type_with_the_current_epoch_is_ignored_and_counted() {
    let bytes = wire(0, 0x80, &epoch_payload(7));
    assert_eq!(
        decode(&bytes, FB, Some(Epoch(7))).unwrap(),
        Decoded::IgnoredAdvisory {
            ty: 0x80,
            used: bytes.len()
        }
    );
    let bytes = wire(0, 0xFF, br#"{"epoch":7,"future":[1,2]}"#);
    assert!(matches!(
        decode(&bytes, FB, Some(Epoch(7))).unwrap(),
        Decoded::IgnoredAdvisory { ty: 0xFF, .. }
    ));
}

#[test]
fn r6_a_malformed_payload_ends_the_epoch_whatever_the_type() {
    // R-6.2 (2): an unknown advisory frame with an empty or non-JSON payload ends the epoch; an unknown state-changing frame with one gets no Fault.
    for ty in [0x80u8, 0xFF, 0x7F, 0x42] {
        for payload in [&b""[..], b"not json", b"[]", b"{\"epoch\":7} x"] {
            let e = decode(&wire(0, ty, payload), FB, Some(Epoch(7))).unwrap_err();
            assert_eq!(
                e.disposition(),
                Disposition::EndsEpoch,
                "type {ty:#x}, payload {payload:?}: {e:?}"
            );
        }
        let e = decode(&wire(0, ty, &[0xff]), FB, Some(Epoch(7))).unwrap_err();
        assert_eq!(e, WireError::InvalidUtf8);
    }
}

#[test]
fn r6_a_stale_frame_is_dropped_whatever_its_type() {
    // R-6.2 (3): stale known, unknown advisory and unknown state-changing frames are dropped and counted; the last gets no Fault.
    for ty in [0x03u8, 0x80, 0x7F] {
        let bytes = wire(0, ty, &epoch_payload(6));
        assert_eq!(
            decode(&bytes, FB, Some(Epoch(7))).unwrap(),
            Decoded::Stale {
                ty,
                epoch: Epoch(6),
                used: bytes.len()
            },
            "type {ty:#x}"
        );
    }
    // A stale known frame is dropped before its schema is read: a bad body does not matter.
    let bytes = wire(1, 0x10, br#"{"epoch":6,"request_id":null}"#);
    assert!(matches!(
        decode(&bytes, FB, Some(Epoch(7))).unwrap(),
        Decoded::Stale { ty: 0x10, .. }
    ));
    // The same bad body with the current epoch is a schema refusal.
    let bytes = wire(1, 0x10, br#"{"epoch":7,"request_id":null}"#);
    assert!(matches!(
        decode(&bytes, FB, Some(Epoch(7))).unwrap_err(),
        WireError::Schema { ty: 0x10, .. }
    ));
}

#[test]
fn r6_a_connection_without_an_epoch_accepts_any_epoch() {
    // The host before `Hello` has no epoch yet (HP HS-1).
    let bytes = wire(0, 0x80, &epoch_payload(99));
    assert!(matches!(
        decode(&bytes, FB, None).unwrap(),
        Decoded::IgnoredAdvisory { .. }
    ));
}

#[test]
fn a_payload_without_an_epoch_is_refused() {
    // HP section 2: every frame carries the epoch.
    for ty in [0x03u8, 0x80, 0x7F] {
        let e = decode(&wire(0, ty, b"{}"), FB, Some(Epoch(7))).unwrap_err();
        assert_eq!(e, WireError::EpochMissing { ty });
        assert_eq!(e.disposition(), Disposition::InvalidRequest);
    }
    assert!(matches!(
        decode(&wire(0, 0x03, br#"{"epoch":"x"}"#), FB, None).unwrap_err(),
        WireError::EpochMissing { .. }
    ));
}

#[test]
fn an_unknown_lane_is_refused() {
    let e = decode_err(&wire(9, 0x03, br#"{"epoch":1}"#));
    assert_eq!(e, WireError::UnknownLane { lane: 9 });
}

#[test]
fn a_known_type_on_the_wrong_lane_is_a_malformed_request_not_a_fault() {
    // R-4 P1b.
    let e = decode_err(&wire(1, 0x03, br#"{"epoch":1}"#));
    assert_eq!(e, WireError::WrongLane { ty: 0x03, lane: 1 });
    assert_eq!(e.disposition(), Disposition::InvalidRequest);
}

#[test]
fn log_write_uses_lane_3_and_every_other_call_lane_2() {
    // R-4 P1b, HP section 2.
    let bulk = Frame::HostCall(host_call(BULK_CALL));
    assert_eq!(bulk.default_lane(), Lane::Bulk);
    let b = encode(Lane::Bulk, &bulk, FB).unwrap();
    assert!(matches!(
        decode(&b, FB, None).unwrap(),
        Decoded::Frame {
            lane: Lane::Bulk,
            ..
        }
    ));
    assert!(matches!(
        encode(Lane::Call, &bulk, FB),
        Err(EncodeError::WrongLane { .. })
    ));
    let other = Frame::HostCall(host_call("store.get"));
    assert!(matches!(
        encode(Lane::Bulk, &other, FB),
        Err(EncodeError::WrongLane { .. })
    ));
    // The decoder refuses a call on the lane of the other class as a malformed request.
    let payload =
        serde_json::to_vec(&serde_json::to_value(host_call("store.get")).unwrap()).unwrap();
    assert!(matches!(
        decode_err(&wire(3, 0x20, &payload)),
        WireError::WrongLane { ty: 0x20, lane: 3 }
    ));
    let payload = serde_json::to_vec(&serde_json::to_value(host_call(BULK_CALL)).unwrap()).unwrap();
    assert!(matches!(
        decode_err(&wire(2, 0x20, &payload)),
        WireError::WrongLane { ty: 0x20, lane: 2 }
    ));
}

#[test]
fn a_host_result_answers_on_the_lane_of_its_call() {
    let f = Frame::HostResult(HostResult {
        epoch: Epoch(1),
        call_id: WireId::Num(5),
        result: PluginResult::Ok(json!({"seq": 1})),
        effect: Effect::None,
    });
    for lane in [Lane::Call, Lane::Bulk] {
        let bytes = encode(lane, &f, FB).unwrap();
        assert!(
            matches!(decode(&bytes, FB, None).unwrap(), Decoded::Frame { lane: l, .. } if l == lane)
        );
    }
    assert!(matches!(
        encode(Lane::Control, &f, FB),
        Err(EncodeError::WrongLane { .. })
    ));
}

#[test]
fn the_context_has_three_forms() {
    // HP section 2, CA-3.
    for (ctx, text) in [
        (CallContext::Request(WireId::Num(3)), json!({"request": 3})),
        (CallContext::Bootstrap, json!("bootstrap")),
        (CallContext::Autonomous, json!("autonomous")),
    ] {
        assert_eq!(serde_json::to_value(&ctx).unwrap(), text);
        assert_eq!(serde_json::from_value::<CallContext>(text).unwrap(), ctx);
    }
    assert!(serde_json::from_value::<CallContext>(json!("other")).is_err());
}

#[test]
fn an_unknown_caller_or_handler_kind_fails_closed() {
    // HP VR-4.
    assert!(serde_json::from_value::<Caller>(json!({"kind": "root"})).is_err());
    assert!(serde_json::from_value::<HandlerKind>(json!("command")).is_err());
    assert!(serde_json::from_value::<HandlerKind>(json!("hook")).is_err());
    assert!(serde_json::from_value::<HandlerKind>(json!("session_action")).is_err());
}

#[test]
fn an_unknown_error_kind_acts_as_failed() {
    // HP ER-1.
    let k: ErrorKind = serde_json::from_value(json!("exploded")).unwrap();
    assert_eq!(k, ErrorKind::Unknown("exploded".into()));
    assert_eq!(k.effective(), ErrorKind::Failed);
    assert_eq!(serde_json::to_value(&k).unwrap(), json!("exploded"));
}

#[test]
fn a_result_is_exactly_ok_or_error() {
    // HP section 3, IV-2.
    assert!(serde_json::from_value::<PluginResult>(json!({"ok": true})).is_err());
    assert!(serde_json::from_value::<PluginResult>(json!({"ok": false})).is_err());
    assert!(
        serde_json::from_value::<PluginResult>(json!({"ok": true, "value": 1, "error": {}}))
            .is_err()
    );
    let ok: PluginResult = serde_json::from_value(json!({"ok": true, "value": null})).unwrap();
    assert_eq!(ok, PluginResult::Ok(json!(null)));
    let err: PluginResult = serde_json::from_value(json!({"ok": false, "error": {"kind": "conflict", "message": "m", "retryable": false, "effect": "none", "detail": {"code": "revision_conflict", "op_index": 2}}})).unwrap();
    match err {
        PluginResult::Err(e) => {
            assert_eq!(e.kind, ErrorKind::Conflict);
            assert_eq!(e.detail.unwrap().extra["op_index"], 2);
        }
        _ => panic!(),
    }
}

#[test]
fn wf1_len_equal_to_the_limit_is_accepted() {
    let bytes = wire(
        1,
        0x11,
        br#"{"epoch":1,"request_id":1,"result":{"ok":true,"value":null}}"#,
    );
    let len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    decode(&bytes, len, None).unwrap();
    assert_eq!(
        decode(&bytes, len - 1, None).unwrap_err(),
        WireError::LenOverLimit {
            len,
            limit: len - 1
        }
    );
}

/// The `value` of a decoded `Result` frame.
fn result_value(payload: &str) -> Result<serde_json::Value, WireError> {
    match decode(&wire(1, 0x11, payload.as_bytes()), FB, None)? {
        Decoded::Frame {
            frame:
                Frame::Result(ResultFrame {
                    result: PluginResult::Ok(v),
                    ..
                }),
            ..
        } => Ok(v),
        other => panic!("not an ok Result frame: {other:?}"),
    }
}

#[test]
fn wf3_floats_keep_their_value_and_integral_floats_have_the_same_limit() {
    let frame = |v: &str| {
        format!(r#"{{"epoch":1,"request_id":1,"result":{{"ok":true,"value":{{"n":{v}}}}}}}"#)
    };
    // A fraction and a small integral float are values, not out of range.
    assert_eq!(result_value(&frame("1.5")).unwrap(), json!({"n": 1.5}));
    assert_eq!(result_value(&frame("1.0")).unwrap(), json!({"n": 1.0}));
    // An integral float at 2^53 is allowed; above it, it is refused like an integer.
    result_value(&frame("9.007199254740992e15")).unwrap();
    for big in ["1e16", "-1e16"] {
        assert_eq!(
            result_value(&frame(big)).unwrap_err(),
            WireError::IntegerOutOfRange {
                pointer: "/result/value/n".into()
            },
            "{big}"
        );
    }
}
