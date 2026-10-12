//! The schema resolver of the HC suite: which schema checks a frame, in which direction (design 5.4, 8; HC EN-1, EN-2).

use botster_conformance::{Direction, FrameSchema, SchemaResolver};
use botster_hub_conformance::hc::HcSchemas;
use serde_json::json;

fn schema_of(direction: Direction, frame: serde_json::Value) -> FrameSchema {
    HcSchemas::default().frame_schema(direction, &frame)
}

fn r(name: &str) -> FrameSchema {
    FrameSchema::Ref(name.into())
}

#[test]
fn a_frame_type_selects_the_server_or_the_client_schema_whatever_the_direction() {
    for ty in ["welcome", "res", "watch", "event", "close"] {
        for d in [Direction::Sent, Direction::Received] {
            assert_eq!(
                schema_of(d, json!({"type": ty})),
                r("hc/ServerFrame"),
                "{ty}"
            );
        }
    }
    for ty in ["hello", "req", "cancel"] {
        for d in [Direction::Sent, Direction::Received] {
            assert_eq!(
                schema_of(d, json!({"type": ty})),
                r("hc/ClientFrame"),
                "{ty}"
            );
        }
    }
}

#[test]
fn an_unknown_type_is_checked_by_the_schema_of_its_direction() {
    assert_eq!(
        schema_of(Direction::Sent, json!({"type": "banana"})),
        r("hc/ClientFrame")
    );
    assert_eq!(
        schema_of(Direction::Received, json!({"type": "banana"})),
        r("hc/ServerFrame")
    );
}

#[test]
fn a_frame_without_a_type_is_a_route_frame() {
    match schema_of(Direction::Received, json!({"frame": "live"})) {
        FrameSchema::Ref(name) => assert!(!name.starts_with("hc/"), "{name}"),
        FrameSchema::NotFramed => {}
    }
    let s = HcSchemas::default();
    assert!(s
        .validate("route/to_client", &json!({"frame": "live"}))
        .is_ok());
    // A transcript names the route schemas by their short names when it asks for a violation (`violates: "to_worker"`).
    assert!(s
        .violations("to_client", &json!({"frame": "live"}))
        .unwrap()
        .is_empty());
    assert!(s
        .violations("to-client", &json!({"frame": "live"}))
        .unwrap()
        .is_empty());
    assert!(
        s.validate("route/to_worker", &json!({"frame": "live"}))
            .is_err(),
        "`live` is a frame to the client, not to the worker"
    );
    assert!(!s
        .violations("to_worker", &json!({"frame": "live"}))
        .unwrap()
        .is_empty());
    assert!(!s
        .violations("to-worker", &json!({"frame": "live"}))
        .unwrap()
        .is_empty());
}

#[test]
fn frames_validate_against_the_generated_schemas_and_a_violation_names_its_rule() {
    let s = HcSchemas::default();
    let req = json!({"type": "req", "id": "1", "verb": "get", "kind": "session", "target": "s1"});
    assert!(s.validate("hc/ClientFrame", &req).is_ok());
    assert!(s.violations("hc/ClientFrame", &req).unwrap().is_empty());
    // HC EN-1: `req` has `id`, `verb` and `kind`.
    let bad = json!({"type": "req", "id": "1", "kind": "session"});
    assert!(s.validate("hc/ClientFrame", &bad).is_err());
    let paths = s.violations("hc/ClientFrame", &bad).unwrap();
    assert!(!paths.is_empty(), "{paths:?}");
    // The same frame is not a server frame.
    assert!(s.validate("hc/ServerFrame", &req).is_err());
    // A schema that the suite does not know is an error, never a pass.
    assert!(s.validate("hc/NoSuchFrame", &req).is_err());
    assert!(s.violations("hc/NoSuchFrame", &req).is_err());
    // A kind row has its own schema (design 8).
    assert!(s.validate("kind/spawn_target", &json!({"id": "t1", "label": "x", "path": "/p", "kind": "directory", "source": "device"})).is_ok());
    // The second use of a schema reads it from the cache, with the same answer.
    assert!(s.validate("hc/ClientFrame", &req).is_ok());
}
