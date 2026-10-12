//! The generated schemas against the wire forms (Codec A.1, A.2, A.5).

use botster_route_codec::prelude::*;
use botster_route_codec::schema;
use serde_json::{json, Value};

fn valid(schema: &Value, v: &Value) -> bool {
    jsonschema::validator_for(schema).unwrap().is_valid(v)
}

// Codec A.2, A.4 (extension rules): a reason that this version does not know stays readable, as a string or as a tagged object.
#[test]
fn unknown_reasons_stay_readable_on_the_wire_and_in_the_schema() {
    let schema = schema::to_client();
    for reason in [
        json!("future"),
        json!({ "future_reason": { "detail": 1 } }),
        json!({ "unsupported": { "what": "x" } }),
    ] {
        let frame = json!({ "frame": "input_refused", "reason": reason });
        assert!(valid(&schema, &frame), "{frame}");
        let mut object = frame.as_object().unwrap().clone();
        object.remove("frame");
        let body = serde_json::to_vec(&object).unwrap();
        let mut bytes = vec![0x0A];
        bytes.extend(body);
        let bounds = FrameBounds {
            max_frame: 4096,
            max_screen: 4096,
            max_history: 4096,
        };
        assert!(
            matches!(
                ToClient::decode(&bytes, &bounds),
                Ok(Decoded::Frame(ToClient::InputRefused(_)))
            ),
            "{frame}"
        );
    }
    let kind: QueryKind = serde_json::from_value(json!({ "future_kind": { "a": 1 } })).unwrap();
    assert_eq!(kind, QueryKind::Other);
    let close: CloseReason = serde_json::from_value(json!({ "future": {} })).unwrap();
    assert_eq!(close, CloseReason::Other);
}

// Codec A.1: null is not accepted in a route body, in the schema or on the wire.
#[test]
fn null_is_refused() {
    let frame = json!({ "frame": "input_refused", "op": null, "reason": "refused" });
    assert!(!valid(&schema::to_client(), &frame));
    let mut bytes = vec![0x0A];
    bytes.extend(br#"{"op":null,"reason":"refused"}"#);
    let bounds = FrameBounds {
        max_frame: 4096,
        max_screen: 4096,
        max_history: 4096,
    };
    assert_eq!(
        ToClient::decode(&bytes, &bounds).unwrap_err().code,
        ProtocolErrorCode::MalformedFrame
    );
}

// Codec A.5, Core IN-9: the field constraints are in the schema and enforced when decoding.
#[test]
fn input_constraints() {
    let schema = schema::to_worker();
    let key = |extra: Value| {
        let mut v = json!({ "frame": "input", "kind": "key", "key": { "char": "a" }, "mods": [], "event": "press" });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    assert!(valid(&schema, &key(json!({}))));
    assert!(!valid(&schema, &key(json!({ "key": { "char": "ab" } }))));
    assert!(!valid(
        &schema,
        &key(json!({ "shifted_key": { "char": "A" } }))
    ));
    assert!(valid(
        &schema,
        &key(json!({ "mods": ["shift"], "shifted_key": { "char": "A" } }))
    ));
    assert!(!valid(
        &schema,
        &key(json!({ "mods": ["shift"], "shifted_key": { "named": "enter" } }))
    ));
    assert!(!valid(&schema, &key(json!({ "mods": ["shift", "shift"] }))));
    let mouse = |button: &str, action: &str, extra: Value| {
        let mut v = json!({ "frame": "input", "kind": "mouse", "action": action, "button": button, "row": 1, "col": 1, "mods": [] });
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        v
    };
    assert!(valid(
        &schema,
        &mouse("wheel_up", "wheel", json!({ "notches": 2 }))
    ));
    assert!(!valid(&schema, &mouse("wheel_up", "press", json!({}))));
    assert!(!valid(&schema, &mouse("left", "wheel", json!({}))));
    assert!(!valid(
        &schema,
        &mouse("left", "press", json!({ "notches": 1 }))
    ));
    assert!(!valid(
        &schema,
        &mouse("left", "press", json!({ "mods": ["meta"] }))
    ));
    // Pixel fields are optional and ignored outside sgr_pixels: one of them alone is valid.
    assert!(valid(&schema, &mouse("left", "press", json!({ "x": 3 }))));
    let reply = |pos: Value| json!({ "frame": "input", "kind": "reply_terminal_query", "query_id": "q", "reply": { "position": pos } });
    assert!(valid(&schema, &reply(json!({ "x": 65535, "y": 0 }))));
    assert!(!valid(&schema, &reply(json!({ "x": 65536, "y": 0 }))));
    assert!(!valid(
        &schema,
        &json!({ "frame": "input", "kind": "paste_begin", "op": "18446744073709551616" })
    ));
    // The decoder enforces the same rules: a parseable input that breaks one is `Invalid`, and the route stays open.
    let bounds = FrameBounds {
        max_frame: 4096,
        max_screen: 4096,
        max_history: 4096,
    };
    let wire = |v: Value| {
        let mut body = v.as_object().unwrap().clone();
        body.remove("frame");
        let mut b = vec![0x81];
        b.extend(serde_json::to_vec(&body).unwrap());
        ToWorker::decode(&b, &bounds)
    };
    assert!(matches!(
        wire(key(json!({ "key": { "char": "ab" } }))),
        Ok(Decoded::Invalid { .. })
    ));
    assert!(matches!(
        wire(key(json!({ "shifted_key": { "char": "A" } }))),
        Ok(Decoded::Invalid { .. })
    ));
    assert!(matches!(
        wire(mouse("left", "wheel", json!({}))),
        Ok(Decoded::Invalid { .. })
    ));
    assert!(matches!(
        wire(reply(json!({ "x": 65536, "y": 0 }))),
        Ok(Decoded::Invalid { .. })
    ));
    assert!(matches!(
        wire(json!({ "frame": "input", "kind": "resize", "rows": 0, "cols": 80 })),
        Ok(Decoded::Invalid { .. })
    ));
    assert_eq!(
        wire(json!({ "frame": "input", "kind": "paste_begin", "op": "0" }))
            .unwrap_err()
            .code,
        ProtocolErrorCode::MalformedFrame
    );
}

// Codec TR-3: a chunk over the route's chunk bound is refused as it arrives.
#[test]
fn chunk_bound() {
    let bounds = FrameBounds {
        max_frame: 64,
        max_screen: 64,
        max_history: 64,
    };
    let mut r = ChunkReader::with_max_chunk(4);
    assert!(r.push_message(&[0, 7, 1, 2, 3], &bounds).unwrap().is_none());
    assert_eq!(
        r.push_message(&[1, 7, 1, 2, 3, 4], &bounds)
            .unwrap_err()
            .code,
        ProtocolErrorCode::FrameTooLarge
    );
}

// Codec A.2c: the lifecycle class of each state, and what each class allows.
#[test]
fn lifecycle_table() {
    use SessionState::*;
    for s in [Starting, Running, Stopping] {
        assert_eq!(s.lifecycle_class(), LifecycleClass::Current);
        assert!(s.accepts_input() && !s.can_delete() && !s.can_restart(true));
    }
    assert_eq!(Ended.lifecycle_class(), LifecycleClass::Ended);
    assert!(
        !Ended.accepts_input()
            && Ended.can_delete()
            && Ended.can_restart(true)
            && !Ended.can_restart(false)
    );
    assert_eq!(Lost.lifecycle_class(), LifecycleClass::Indeterminate);
    assert!(!Lost.accepts_input() && Lost.can_delete() && !Lost.can_restart(true));
}

// Codec A.1, A.2: only an unknown string or an unknown one-key tagged object is `Other`. A wrong shape, or bad data for a known name, is an error.
#[test]
fn malformed_reason_is_not_other() {
    let bounds = FrameBounds {
        max_frame: 4096,
        max_screen: 4096,
        max_history: 4096,
    };
    let decode = |body: &str| {
        let mut b = vec![0x0A];
        b.extend(body.as_bytes());
        ToClient::decode(&b, &bounds)
    };
    for bad in [
        r#"{"reason":17}"#,
        r#"{"reason":[1]}"#,
        r#"{"reason":{"unsupported":5}}"#,
        r#"{"reason":"unsupported"}"#,
        r#"{"reason":{"unsupported":{"what":1}}}"#,
        r#"{"reason":{"future":{},"other":{}}}"#,
        r#"{"reason":{"future":5}}"#,
    ] {
        assert!(decode(bad).is_err(), "{bad}");
    }
    for good in [
        r#"{"reason":"future"}"#,
        r#"{"reason":{"future":{"a":1}}}"#,
        r#"{"reason":"other"}"#,
    ] {
        assert!(decode(good).is_ok(), "{good}");
    }
}

// Codec A.4, BP-3: a payload is `{"$bytes_hex": <lowercase hex>}` in the schema, under the name `HexBytes`, and only that shape is valid.
#[test]
fn payload_schema_is_the_bytes_hex_object() {
    let schema = schema::to_client();
    let def = &schema["$defs"]["HexBytes"];
    assert_eq!(def["required"], json!(["$bytes_hex"]));
    assert_eq!(def["additionalProperties"], json!(false));
    let screen = |payload: Value| json!({ "frame": "screen", "payload": payload });
    assert!(valid(&schema, &screen(json!({ "$bytes_hex": "00ff" }))));
    assert!(!valid(&schema, &screen(json!({ "$bytes_hex": "0F" }))));
    assert!(!valid(&schema, &screen(json!({ "$bytes_hex": "abc" }))));
    assert!(!valid(&schema, &screen(json!("00ff"))));
    // The three conversions give the same payload.
    let from_vec = Payload::from(vec![1, 2]);
    assert_eq!(from_vec, Payload::from(&[1u8, 2][..]));
    assert_eq!(from_vec, Payload::from(bytes::Bytes::from_static(&[1, 2])));
}
