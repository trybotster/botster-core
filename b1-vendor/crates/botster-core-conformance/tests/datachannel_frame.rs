//! The frame that a `datachannel_send` control carries is checked against the codec schemas before it is sent (Core DP-3, design 5.4).

use botster_conformance::stimulus::prepare;
use botster_conformance::Bindings;
use botster_core_conformance::CoreSchemas;
use serde_json::json;

fn control(frame: serde_json::Value) -> serde_json::Value {
    json!({"control": {"op": "datachannel_send", "route": "$r1", "frame": frame, "chunk_bytes": 4}})
}

#[test]
fn a_valid_frame_passes_and_a_bad_frame_is_a_transcript_error() {
    let mut b = Bindings::new();
    b.insert("r1".into(), json!(1));
    let ok = control(json!({"frame": "bytes", "op": "1", "bytes": {"$bytes_hex": "61"}}));
    prepare(&ok, &b, &CoreSchemas).unwrap();
    // A structural run has no bindings: the route is unbound, and a bad frame is still found.
    let none = Bindings::new();
    let unbound = |e: botster_conformance::StepError| format!("{e:?}").contains("unbound variable");
    assert!(prepare(&ok, &none, &CoreSchemas).is_err_and(unbound));
    let bad_unbound = control(json!({"frame": "bytes", "op": "1"}));
    let err = prepare(&bad_unbound, &none, &CoreSchemas).unwrap_err();
    assert!(
        !unbound(err),
        "a bad frame is not an unbound-variable error"
    );
    let bad = control(json!({"frame": "bytes", "op": "1"}));
    assert!(prepare(&bad, &b, &CoreSchemas).is_err());
    let unknown = control(json!({"frame": "no_such_frame"}));
    assert!(prepare(&unknown, &b, &CoreSchemas).is_err());
    // Only `datachannel_send` carries a frame: another control is passed on unchanged.
    let other = json!({"control": {"op": "datachannel_close", "route": "$r1", "frame": {"frame": "no_such_frame"}}});
    assert_eq!(prepare(&other, &b, &CoreSchemas).unwrap(), other);
}
