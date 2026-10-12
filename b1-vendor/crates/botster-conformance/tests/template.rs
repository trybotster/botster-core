//! `{"$ref": name, ..overrides}` expands to the template of that name, deep-merged with the overrides (design 5.1).

use botster_conformance::template::expand;
use serde_json::json;

fn templates() -> serde_json::Value {
    json!({"hello": {"epoch": 1, "limits": {"a": 1, "b": 2}, "list": [1, 2]}})
}

#[test]
fn an_override_replaces_a_scalar_and_merges_an_object() {
    let v = expand(
        &json!({"$ref": "hello", "epoch": 7, "limits": {"b": 9, "c": 3}}),
        &templates(),
    )
    .unwrap();
    assert_eq!(
        v,
        json!({"epoch": 7, "limits": {"a": 1, "b": 9, "c": 3}, "list": [1, 2]})
    );
}

#[test]
fn an_override_replaces_an_array_whole_and_a_scalar_can_replace_an_object() {
    let v = expand(
        &json!({"$ref": "hello", "list": [3], "limits": 5}),
        &templates(),
    )
    .unwrap();
    assert_eq!(v["list"], json!([3]));
    assert_eq!(v["limits"], json!(5));
}

#[test]
fn references_expand_inside_arrays_and_objects_and_other_values_stay() {
    let v = expand(
        &json!([{"x": {"$ref": "hello", "epoch": 2}}, 5, "s", null]),
        &templates(),
    )
    .unwrap();
    assert_eq!(v[0]["x"]["epoch"], 2);
    assert_eq!(v[0]["x"]["limits"]["a"], 1);
    assert_eq!(v[1], json!(5));
    assert_eq!(v[2], json!("s"));
    assert_eq!(v[3], json!(null));
}

#[test]
fn an_unknown_template_is_an_error_that_names_it() {
    let e = expand(&json!({"$ref": "nope"}), &templates()).unwrap_err();
    assert!(e.contains("nope"), "{e}");
}

#[test]
fn an_object_without_a_ref_is_expanded_member_by_member() {
    let v = expand(&json!({"a": {"$ref": "hello"}, "b": 1}), &templates()).unwrap();
    assert_eq!(v["b"], 1);
    assert_eq!(v["a"]["epoch"], 1);
}
