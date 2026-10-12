//! The probe-script format (Core A5-1, design 6.2): the parser, the validator and the generated schema.

use botster_probe_script::{Script, ScriptError, Step, PROBE_BINARY};
use serde_json::{json, Value};

const EVERY_STEP: &str = r#"{"program":[
    {"print":{"bytes_hex":"6869","fake":{"modes":{"a":1}}}},
    {"print_after_input":{"match_hex":"0d","bytes_hex":"6f6b"}},
    {"ignore_sigterm":{}},
    {"fork_child":{"script":[{"hold":{}}]}},
    {"signal_self":{"n":9}},
    {"exit":{"code":3}},
    {"hold":{}}
  ],"fake":{"anything":true}}"#;

#[test]
fn every_step_kind_parses_and_round_trips() {
    let script: Script = Script::parse(EVERY_STEP).unwrap();
    assert_eq!(script.program.len(), 7);
    assert!(script.ignores_sigterm());
    script.validate().unwrap();
    let again: Script = Script::parse(&serde_json::to_string(&script).unwrap()).unwrap();
    assert_eq!(again, script);
}

#[test]
fn argv_selects_the_script() {
    let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let named = argv(&[
        &format!("/opt/bin/{PROBE_BINARY}"),
        r#"{"program":[{"exit":{"code":7}}]}"#,
    ]);
    let script: Script = Script::from_argv(&named).unwrap();
    assert_eq!(script.program, vec![Step::Exit { code: 7 }]);
    // Another program, or no script text: the default script, which has no steps.
    assert_eq!(
        Script::<Value>::from_argv(&argv(&["sh", "{bad"])).unwrap(),
        Script::default()
    );
    assert_eq!(
        Script::<Value>::from_argv(&argv(&[PROBE_BINARY])).unwrap(),
        Script::default()
    );
    let bad = argv(&[PROBE_BINARY, "{bad"]);
    assert!(matches!(
        Script::<Value>::from_argv(&bad),
        Err(ScriptError::Parse(_))
    ));
}

#[test]
fn unknown_step_does_not_parse() {
    assert!(Script::<Value>::parse(r#"{"program":[{"dance":{}}]}"#).is_err());
}

#[test]
fn validate_names_the_bad_step() {
    let check = |text: &str| {
        Script::<Value>::parse(text)
            .unwrap()
            .validate()
            .unwrap_err()
    };
    let ScriptError::Invalid { path, .. } =
        check(r#"{"program":[{"hold":{}},{"print":{"bytes_hex":"zz"}}]}"#)
    else {
        panic!("expected Invalid")
    };
    assert_eq!(path, "program[1].bytes_hex");
    let ScriptError::Invalid { path, .. } =
        check(r#"{"program":[{"print_after_input":{"match_hex":"00","bytes_hex":"abc"}}]}"#)
    else {
        panic!("expected Invalid")
    };
    assert_eq!(path, "program[0].bytes_hex");
    let ScriptError::Invalid { path, .. } =
        check(r#"{"program":[{"fork_child":{"script":[{"signal_self":{"n":0}}]}}]}"#)
    else {
        panic!("expected Invalid")
    };
    assert_eq!(path, "program[0].script[0].n");
}

#[test]
fn schema_accepts_a_script_and_rejects_an_unknown_step() {
    let schema = botster_probe_script::schema();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let good: Value = serde_json::from_str(EVERY_STEP).unwrap();
    assert!(validator.is_valid(&good));
    assert!(!validator.is_valid(&json!({"program":[{"dance":{}}]})));
}

#[test]
fn only_a_script_with_the_step_ignores_sigterm() {
    let plain: Script = Script::parse(r#"{"program":[{"hold":{}},{"exit":{"code":0}}]}"#).unwrap();
    assert!(!plain.ignores_sigterm());
    assert!(!Script::<Value>::default().ignores_sigterm());
}

#[test]
fn an_error_displays_its_reason_and_path() {
    let parse = Script::<Value>::parse("{bad").unwrap_err();
    assert!(
        parse.to_string().contains("key must be a string"),
        "{parse}"
    );
    let invalid = Script::<Value>::parse(r#"{"program":[{"signal_self":{"n":99}}]}"#)
        .unwrap()
        .validate()
        .unwrap_err();
    assert_eq!(
        invalid.to_string(),
        "program[0].n: a signal number is 1 to 64"
    );
}

#[test]
fn the_checked_in_schema_is_what_render_makes() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/core/probe-script.schema.json"
    );
    let on_disk = std::fs::read_to_string(path).unwrap();
    assert_eq!(
        botster_probe_script::render(&botster_probe_script::schema()),
        on_disk
    );
}
