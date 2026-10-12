//! R-85: the shared check for a `0d` byte in the expected bytes of an input-log assertion, and its typed transcript defect.
//! The suite drivers call these; this crate's own tests pin them (cargo mutants runs only the mutated crate's tests).

use botster_conformance::vectors::order_vector_failures;
use botster_conformance::{input_log_0d, input_log_0d_defect, StepError};
use serde_json::{json, Value};

#[test]
fn a_0d_byte_is_found_at_its_byte_offset_in_either_case() {
    assert_eq!(input_log_0d("0d"), Some(0));
    assert_eq!(input_log_0d("610d"), Some(1));
    assert_eq!(input_log_0d("61620D63"), Some(2));
    assert_eq!(input_log_0d("0d610d"), Some(0), "the first 0d counts");
}

#[test]
fn no_0d_byte_is_none() {
    assert_eq!(input_log_0d(""), None);
    assert_eq!(input_log_0d("6162"), None);
    // `0d` across a byte boundary is the bytes `a0` and `d1`.
    assert_eq!(input_log_0d("a0d1"), None);
    // Hex that does not decode has no byte to check.
    assert_eq!(input_log_0d("0d0"), None);
    assert_eq!(input_log_0d("610d6"), None);
}

#[test]
fn the_defect_names_the_rule_the_assertion_the_hex_and_the_offset() {
    match input_log_0d_defect("pty_input expect", "610d", 1) {
        StepError::TranscriptDefect { rule, detail } => {
            assert_eq!(rule, "R-85");
            assert_eq!(detail["assertion"], "pty_input expect");
            assert_eq!(detail["expected_hex"], "610d");
            assert_eq!(detail["0d_at_byte"], 1);
        }
        other => panic!("expected a transcript defect, got {other:?}"),
    }
}

#[test]
fn an_order_vector_whose_result_differs_is_a_failure_line() {
    let case = |result: &str| {
        json!({
            "name": "expect: the next item matches",
            "step": {"expect": {"type": "res", "id": "1"}},
            "stream": [{"item": {"type": "res", "id": "1", "ok": true}}],
            "result": result,
        })
    };
    let file = |cases: Vec<Value>| json!({ "cases": cases });
    assert!(order_vector_failures(&file(vec![case("passed")])).is_empty());
    assert_eq!(
        order_vector_failures(&file(vec![case("transcript_defect")])),
        vec!["expect: the next item matches: expected transcript_defect, got passed".to_string()]
    );
}
