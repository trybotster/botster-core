//! The per-expectation citation of design 5.6, rule 6: every expectation names the clause that fixes its value.

use crate::transcript::Transcript;
use serde_json::Value;

/// Steps whose value is an expectation. A `control` step is one only when it carries `expect`.
const EXPECT_STEPS: [&str; 9] = [
    "expect",
    "expect_set",
    "expect_eventually",
    "expect_quiet_until",
    "expect_closed",
    // The CLI suite (design 5.5): what the command exits with, prints and writes.
    "expect_exit",
    "expect_stdout",
    "expect_stderr",
    "expect_file",
];

fn is_expectation(step: &Value) -> bool {
    let Some(obj) = step.as_object() else {
        return false;
    };
    obj.keys().any(|k| {
        EXPECT_STEPS.contains(&k.as_str())
            || k.starts_with("route_expect")
            || k == "route_check_path"
            // Core suite: an error that a step must give, and the wait that ends on an expected event.
            || k == "expect_error"
            || k == "pump_until"
            || k == "pump_collect"
            || k == "check"
    }) || (obj.contains_key("control") && obj.contains_key("expect"))
}

/// True when the citation has the form `<contract> <clause id>`, for example `HC EN-8`, `Codec TS-5b` or `Codec A.1`.
pub fn is_citation(text: &str) -> bool {
    let mut parts = text.split(' ');
    let (Some(contract), Some(clause), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    contract.chars().next().is_some_and(char::is_alphabetic)
        && contract.chars().all(char::is_alphanumeric)
        && clause.chars().any(|c| c.is_ascii_digit())
        && clause
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// A `because` is one citation, or a non-empty list of citations.
fn well_formed(because: &Value) -> bool {
    match because {
        Value::String(text) => is_citation(text),
        Value::Array(list) => {
            !list.is_empty() && list.iter().all(|v| v.as_str().is_some_and(is_citation))
        }
        _ => false,
    }
}

/// The positions of the steps that are expectations without a well-formed `because`.
pub fn missing_because(transcript: &Transcript) -> Vec<usize> {
    transcript
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| is_expectation(s))
        .filter(|(_, s)| !s.get("because").is_some_and(well_formed))
        .map(|(i, _)| i)
        .collect()
}
