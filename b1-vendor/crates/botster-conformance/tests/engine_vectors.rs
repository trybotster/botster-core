//! The Rust engine passes every case of `conformance/engine-vectors/` (design 6.6).

use botster_conformance::vectors::{matcher_vector_failures, order_vector_failures};
use include_dir::{include_dir, Dir};
use serde_json::Value;

static VECTORS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../conformance/engine-vectors");

fn file(name: &str) -> Value {
    let f = VECTORS
        .get_file(name)
        .unwrap_or_else(|| panic!("missing {name}"));
    serde_json::from_str(f.contents_utf8().unwrap()).unwrap()
}

#[test]
fn matcher_vectors() {
    let failures = matcher_vector_failures(&file("matchers.json"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn order_vectors() {
    let failures = order_vector_failures(&file("order.json"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_vector_file_has_a_runner() {
    let names: Vec<_> = VECTORS
        .files()
        .map(|f| f.path().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["matchers.json", "order.json"]);
}
