//! `CoreHarness::probe_env` (design 6.2; the probe control socket, steward approval of 2026-10-10): the driver adds the harness's
//! probe variables to the `env` of a `Create` request, and to nothing else.

use botster_core_conformance::{normalize_op, normalize_op_env};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn probe_env() -> BTreeMap<String, String> {
    BTreeMap::from([("BOTSTER_PROBE_CONTROL".to_string(), "/s/c".to_string())])
}

#[test]
fn a_create_gets_the_probe_variables_and_keeps_its_own() {
    let mut op = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}], "env": {"A": "1"}}}});
    normalize_op_env(&mut op, "probe", &probe_env());
    assert_eq!(
        op["Create"]["request"]["env"],
        json!({"A": "1", "BOTSTER_PROBE_CONTROL": "/s/c"})
    );
}

#[test]
fn a_create_with_no_env_gets_only_the_probe_variables() {
    let mut op = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op_env(&mut op, "probe", &probe_env());
    assert_eq!(
        op["Create"]["request"]["env"],
        json!({"BOTSTER_PROBE_CONTROL": "/s/c"})
    );
}

#[test]
fn without_probe_variables_the_op_is_as_normalize_op_makes_it() {
    let original = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}], "env": {"A": "1"}}}});
    let mut with_none = original.clone();
    normalize_op_env(&mut with_none, "probe", &BTreeMap::new());
    let mut plain = original;
    normalize_op(&mut plain, "probe");
    assert_eq!(with_none, plain);
    assert_eq!(plain["Create"]["request"]["env"], json!({"A": "1"}));
}

#[test]
fn a_service_gets_no_probe_variable() {
    let mut op = json!({"SpawnService": {"session": "s1"}});
    normalize_op_env(&mut op, "probe", &probe_env());
    assert_eq!(op["SpawnService"]["env"], json!({}));
    assert!(op
        .pointer("/SpawnService/request")
        .is_none_or(Value::is_null));
}

#[test]
fn a_harness_with_no_probe_socket_names_no_probe_variable() {
    use botster_core_conformance::fake::FakeCoreHarness;
    use botster_core_conformance::{from_make, CoreHarness};
    assert!(FakeCoreHarness::new(0).probe_env().is_empty());
    let made = from_make(|| -> Box<dyn botster_core_contract::prelude::CoreApi> {
        unreachable!("no Core is opened")
    });
    assert!(made.probe_env().is_empty());
}
