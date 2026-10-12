//! Codec tests: each proves a clause of the route codec (revision 36).

use botster_route_codec::prelude::*;
use serde_json::json;

fn bounds() -> FrameBounds {
    FrameBounds {
        max_frame: 512,
        max_screen: 1024,
        max_history: 768,
    }
}

fn small() -> FrameBounds {
    FrameBounds {
        max_frame: 64,
        max_screen: 128,
        max_history: 96,
    }
}

// The round-trip of every frame type, with its type byte, is a property (src/props.rs). The tests below pin literals and edges.

// Codec BP-3, A.4: data frames carry raw bytes with no JSON, and `op` is 8 bytes big-endian. A property cannot pin this layout: it round-trips through the same encoder.
#[test]
fn data_frames_are_raw() {
    let out = ToClient::Output {
        payload: Payload::from(vec![0, 255, 7]),
    }
    .encode();
    assert_eq!(out, vec![0x07, 0, 255, 7]);
    let text = ToWorker::Text {
        op: Op(5),
        text: "hé".into(),
    }
    .encode();
    assert_eq!(&text[..9], &[0x84, 0, 0, 0, 0, 0, 0, 0, 5]);
    assert_eq!(&text[9..], "hé".as_bytes());
}

// Codec A.1: a frame over the bound of its own type is frame_too_large. Screen and history have their own bounds.
#[test]
fn bounds_are_per_type() {
    let big = |t: u8, n: usize| {
        let mut v = vec![t];
        v.extend(vec![0u8; n - 1]);
        v
    };
    let b = small();
    assert!(ToClient::decode(&big(0x03, 128), &b).is_ok());
    assert_eq!(
        ToClient::decode(&big(0x03, 129), &b).unwrap_err().code,
        ProtocolErrorCode::FrameTooLarge
    );
    assert_eq!(
        ToClient::decode(&big(0x07, 65), &b).unwrap_err().code,
        ProtocolErrorCode::FrameTooLarge
    );
    assert!(ToClient::decode(&big(0x04, 96), &b).is_ok());
}

// Codec A.4 extension rules: an unknown type is ignorable only with bit 0x40.
#[test]
fn unknown_types() {
    assert_eq!(
        ToClient::decode(&[0x41, 1], &bounds()).unwrap(),
        Decoded::Ignored { type_byte: 0x41 }
    );
    assert_eq!(
        ToClient::decode(&[0x20], &bounds()).unwrap_err().code,
        ProtocolErrorCode::UnknownFrame
    );
    assert_eq!(
        ToWorker::decode(&[0xC3], &bounds()).unwrap(),
        Decoded::Ignored { type_byte: 0xC3 }
    );
    assert_eq!(
        ToWorker::decode(&[0x86], &bounds()).unwrap_err().code,
        ProtocolErrorCode::UnknownFrame
    );
}

// Codec A.1: duplicate keys, invalid JSON and unsafe integers are malformed_frame.
#[test]
fn strict_json() {
    let body = |s: &str| {
        let mut v = vec![0x09];
        v.extend(s.as_bytes());
        v
    };
    for bad in [
        r#"{"reason":"overflow","reason":"stalled"}"#,
        "{",
        r#"{"reason":9007199254740993}"#,
    ] {
        assert_eq!(
            ToClient::decode(&body(bad), &bounds()).unwrap_err().code,
            ProtocolErrorCode::MalformedFrame,
            "{bad}"
        );
    }
}

// Codec A.5, TS-5b: an unknown kind or enum value is refused, while a broken shape ends the route.
#[test]
fn input_classes() {
    let input = |v: serde_json::Value| {
        let mut f = vec![0x81];
        f.extend(serde_json::to_vec(&v).unwrap());
        ToWorker::decode(
            &f,
            &FrameBounds {
                max_frame: 4096,
                max_screen: 4096,
                max_history: 4096,
            },
        )
    };
    assert!(matches!(
        input(json!({"kind":"hologram"})),
        Ok(Decoded::Unsupported { .. })
    ));
    assert!(matches!(
        input(json!({"op":"3","kind":"key","key":{"named":"banana"},"mods":[],"event":"press"})),
        Ok(Decoded::Unsupported {
            op: Some(Op(3)),
            ..
        })
    ));
    assert!(matches!(
        input(json!({"kind":"focus","focused":true})),
        Ok(Decoded::Frame(_))
    ));
    // A paste frame with no op (TS-5d), and a body that is no object.
    assert_eq!(
        input(json!({"kind":"paste_begin"})).unwrap_err().code,
        ProtocolErrorCode::MalformedFrame
    );
    assert_eq!(
        input(json!([1])).unwrap_err().code,
        ProtocolErrorCode::MalformedFrame
    );
}

// Codec A.4, TS-5b: a text frame with invalid UTF-8 is refused as unsupported utf8, and a paste chunk with op 0 is malformed.
#[test]
fn binary_input_rules() {
    let b = FrameBounds {
        max_frame: 64,
        max_screen: 64,
        max_history: 64,
    };
    let mut f = vec![0x84, 0, 0, 0, 0, 0, 0, 0, 9, 0xFF];
    assert_eq!(
        ToWorker::decode(&f, &b).unwrap(),
        Decoded::Unsupported {
            op: Some(Op(9)),
            what: "utf8".into()
        }
    );
    f[0] = 0x82;
    f[8] = 0;
    assert_eq!(
        ToWorker::decode(&f, &b).unwrap_err().code,
        ProtocolErrorCode::MalformedFrame
    );
    assert_eq!(
        ToWorker::decode(&[0x83, 1, 2], &b).unwrap_err().code,
        ProtocolErrorCode::InvalidInput
    );
}

// Codec A.4 0x0E: the inflated payload is bounded by the frame bound minus the type byte.
#[test]
fn output_deflate_is_bounded() {
    use botster_route_codec::prelude::{deflate_output, inflate_output};
    let payload = vec![b'a'; 63];
    let stream = deflate_output(&payload);
    assert_eq!(inflate_output(&stream, 63).unwrap(), payload);
    let mut frame = vec![0x0E];
    frame.extend(deflate_output(&[b'a'; 64]));
    assert_eq!(
        ToClient::decode(&frame, &small()).unwrap_err().code,
        ProtocolErrorCode::InvalidInput
    );
    assert_eq!(
        ToClient::decode(&[0x0E, 0xFF, 0xFF], &small())
            .unwrap_err()
            .code,
        ProtocolErrorCode::InvalidInput
    );
}

// Codec TR-3, TB-L2: an incomplete frame is discarded at close, the bound is checked when the type byte arrives, and a reserved flag bit
// ends the route. (Reassembly from any split is a property, src/props.rs.)
#[test]
fn delimiter_edges() {
    let mut c = ChunkReader::new();
    c.push_message(&[0, 7, 7], &bounds()).unwrap();
    assert_eq!(c.pending(), 2);
    c.discard_incomplete();
    assert_eq!(c.pending(), 0);
    // The bound is checked when the type byte arrives, before the rest.
    let mut r = StreamReader::new();
    r.push(&[0, 0, 1, 0, 0x07]);
    assert_eq!(
        r.next_frame(&small()).unwrap_err().code,
        ProtocolErrorCode::FrameTooLarge
    );
    assert_eq!(
        ChunkReader::new()
            .push_message(&[2, 7], &bounds())
            .unwrap_err()
            .code,
        ProtocolErrorCode::MalformedFrame
    );
}

// Codec A.2: an unknown reason decodes as other.
#[test]
fn unknown_reasons_are_other() {
    let r: RefusalReason = serde_json::from_value(json!("brand_new")).unwrap();
    assert_eq!(r, RefusalReason::Other);
    let c: CloseReason = serde_json::from_value(json!("brand_new")).unwrap();
    assert_eq!(c, CloseReason::Other);
}

// Codec BP-2, BP-3: an encoded frame is one shared buffer, so every route holds the same bytes; a shared decode slices that buffer.
#[test]
fn one_encoded_frame_is_shared_and_a_shared_decode_copies_nothing() {
    use bytes::Bytes;
    let frame = ToClient::Output {
        payload: Payload::from(vec![9; 32]),
    };
    let shared = frame.encode_shared();
    let for_route = shared.clone();
    assert_eq!(shared.as_ptr(), for_route.as_ptr());
    assert_eq!(&shared[..], &frame.encode()[..]);
    let Decoded::Frame(ToClient::Output { payload }) =
        ToClient::decode_shared(&for_route, &bounds()).unwrap()
    else {
        panic!("an output frame")
    };
    // The payload starts one byte after the type byte, in the same allocation.
    assert_eq!(payload.as_ptr(), for_route.as_ptr().wrapping_add(1));
    // `decode` of a plain slice copies once, so it does not point into the input.
    let plain = ToClient::decode(&for_route, &bounds()).unwrap();
    let Decoded::Frame(ToClient::Output { payload: copy }) = plain else {
        panic!()
    };
    assert_ne!(copy.as_ptr(), for_route.as_ptr().wrapping_add(1));
    let _: Bytes = payload.0;
}

// Codec A.4 0x0E: the decoder returns the inflated payload it had to inflate, so the receiver does not inflate again.
#[test]
fn output_deflate_decode_returns_the_inflated_payload() {
    use bytes::Bytes;
    let raw = vec![b'z'; 40];
    let mut wire = vec![0x0E];
    wire.extend(deflate_output(&raw));
    let wire = Bytes::from(wire);
    // Every public decode path keeps it: the frame holds what the decoder inflated to validate it.
    for decoded in [
        ToClient::decode_shared(&wire, &small()).unwrap(),
        ToClient::decode(&wire, &small()).unwrap(),
    ] {
        let Decoded::Frame(frame @ ToClient::OutputDeflate { .. }) = decoded else {
            panic!("an output_deflate frame")
        };
        assert_eq!(frame.inflated().unwrap(), &raw[..]);
        // The cache is no part of the frame: it is not on the wire, and a frame that was built equals the decoded one.
        assert_eq!(frame.encode(), wire.to_vec());
        assert_eq!(
            frame,
            ToClient::output_deflate(Payload::from(deflate_output(&raw)))
        );
        assert!(
            ToClient::output_deflate(Payload::from(deflate_output(&raw)))
                .inflated()
                .is_none()
        );
    }
    // Another frame type has no inflated payload.
    let out = Bytes::from(vec![0x07, 1]);
    let Decoded::Frame(frame) = ToClient::decode_shared(&out, &small()).unwrap() else {
        panic!()
    };
    assert!(frame.inflated().is_none());
}

// Codec A.1: the typed detail says why, and every detail has one code.
#[test]
fn decode_errors_carry_a_typed_detail() {
    let e = ToClient::decode(&[0x20], &bounds()).unwrap_err();
    assert_eq!(e.code, ProtocolErrorCode::UnknownFrame);
    assert_eq!(e.detail, DecodeDetail::UnknownType { type_byte: 0x20 });
    let e = ToClient::decode(&[0x07; 70], &small()).unwrap_err();
    assert_eq!(
        e.detail,
        DecodeDetail::FrameOverBound {
            type_byte: 7,
            size: 70,
            bound: 64
        }
    );
    assert_eq!(e.detail.code(), e.code);
}

// Codec TS-5d, TS-5f: a paste chunk and a file chunk need a nonzero op, and the other binary frames accept op 0 ("no op").
#[test]
fn only_paste_and_file_chunks_refuse_op_zero() {
    let b = FrameBounds {
        max_frame: 64,
        max_screen: 64,
        max_history: 64,
    };
    for (t, refused) in [(0x82u8, true), (0x85, true), (0x83, false), (0x84, false)] {
        let frame = [t, 0, 0, 0, 0, 0, 0, 0, 0, b'x'];
        let got = ToWorker::decode(&frame, &b);
        if refused {
            assert_eq!(
                got.unwrap_err().code,
                ProtocolErrorCode::MalformedFrame,
                "{t:#x}"
            );
            assert_eq!(
                ToWorker::decode(&frame, &b).unwrap_err().detail,
                DecodeDetail::ChunkNeedsOp
            );
        } else {
            assert!(matches!(got, Ok(Decoded::Frame(_))), "{t:#x}");
        }
    }
}

// Codec A.1: the text of an error names its detail, so a log line says what was wrong.
#[test]
fn decode_errors_display_their_detail() {
    let e = ToClient::decode(&[0x07; 70], &small()).unwrap_err();
    assert_eq!(
        e.to_string(),
        "FrameTooLarge: frame of 70 bytes is over the bound 64 of type 0x07"
    );
    assert_eq!(
        DecodeDetail::UnknownType { type_byte: 0x20 }.to_string(),
        "unknown frame type 0x20"
    );
    for detail in [
        DecodeDetail::EmptyFrame,
        DecodeDetail::ZeroLengthFrame,
        DecodeDetail::EmptyMessage,
        DecodeDetail::ReservedFlagBits,
        DecodeDetail::ZeroSizeFrame,
        DecodeDetail::LiveHasBody,
        DecodeDetail::BinaryShorterThanOp,
        DecodeDetail::InputNotObject,
        DecodeDetail::InputNoKind,
        DecodeDetail::InputNeedsOp,
        DecodeDetail::ChunkNeedsOp,
    ] {
        assert!(!detail.to_string().is_empty(), "{detail:?}");
    }
    assert_eq!(
        DecodeDetail::ChunkOverMax { size: 9, max: 4 }.to_string(),
        "chunk of 9 bytes is over max_chunk_bytes 4"
    );
    assert_eq!(
        DecodeDetail::BodyShape {
            frame: "resync",
            reason: "x".into()
        }
        .to_string(),
        "resync body: x"
    );
    assert_eq!(
        DecodeDetail::InvalidJson("j".into()).to_string(),
        "invalid JSON body: j"
    );
    assert_eq!(
        DecodeDetail::InputShape("s".into()).to_string(),
        "input shape: s"
    );
    assert_eq!(
        DecodeDetail::OutputDeflate("d".into()).to_string(),
        "output_deflate stream: d"
    );
}
