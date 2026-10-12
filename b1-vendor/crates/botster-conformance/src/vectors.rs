//! The engine vectors (design 6.6): JSON cases that the Rust engine passes and the TypeScript engine will pass.
//!
//! Matcher file (`"suite": "matchers"`): `schemas` (optional map of reference to JSON Schema) and `cases`.
//! A case has `name`, `matcher`, `value` (omit it for a missing value), `bindings_in` (optional), `result` (`match`, `no_match` or `bad_matcher`) and `bindings` (the exact bindings after a match; optional).
//!
//! Order file (`"suite": "order"`): `cases`. A case has `name`, `step`, `stream`, `result`, and optionally `max_polls`, `bindings` and `executed`.
//! The stream lists what the driver gives, in order: `{"item": v}`, `{"pending": true}` and `{"closed": v}`. After the last entry the driver is idle.
//! `result` is `passed`, `failed` or `inconclusive`. `executed` lists the steps that the engine handed to the driver.

use crate::driver::{Deadline, Poll, StepDriver, StepError};
use crate::matcher::{matches, Bindings, MatchFailure, SchemaRegistry};
use crate::order::{run_step, Limits};
use serde_json::{json, Value};
use std::collections::VecDeque;

fn bindings_from(value: Option<&Value>) -> Bindings {
    value
        .and_then(Value::as_object)
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

fn bindings_json(b: &Bindings) -> Value {
    Value::Object(b.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// Runs a matcher vector file. The result lists a line for every case that fails.
pub fn matcher_vector_failures(file: &Value) -> Vec<String> {
    let mut registry = SchemaRegistry::new();
    if let Some(schemas) = file.get("schemas").and_then(Value::as_object) {
        for (name, schema) in schemas {
            registry
                .add(name, schema)
                .unwrap_or_else(|e| panic!("schema {name}: {e}"));
        }
    }
    let mut failures = Vec::new();
    for case in file["cases"].as_array().expect("cases is an array") {
        let name = case["name"].as_str().unwrap_or("?");
        let mut binds = bindings_from(case.get("bindings_in"));
        let outcome = matches(&case["matcher"], case.get("value"), &mut binds, &registry);
        let got = match &outcome {
            Ok(()) => "match",
            Err(MatchFailure::Mismatch { .. }) => "no_match",
            Err(MatchFailure::BadMatcher { .. }) => "bad_matcher",
        };
        let want = case["result"].as_str().unwrap_or("?");
        if got != want {
            failures.push(format!("{name}: expected {want}, got {got}"));
        } else if let Some(want_binds) = case.get("bindings") {
            if &bindings_json(&binds) != want_binds {
                failures.push(format!(
                    "{name}: bindings {} differ from {want_binds}",
                    bindings_json(&binds)
                ));
            }
        }
    }
    failures
}

struct VectorDriver {
    stream: VecDeque<Value>,
    executed: Vec<Value>,
}

impl StepDriver for VectorDriver {
    fn has_feature(&self, _: &str) -> bool {
        true
    }
    fn has_control(&self, _: &str) -> bool {
        true
    }
    fn setup(&mut self, _: &Value) -> Result<(), StepError> {
        Ok(())
    }
    fn exec(&mut self, step: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
        self.executed.push(step.clone());
        // A step with `"fail": true` makes the driver refuse it (the vector of an `ack` that fails).
        if step.get("fail") == Some(&Value::Bool(true)) {
            return Err(StepError::Failed {
                expected: json!("a step that the driver accepts"),
                received: step.clone(),
            });
        }
        Ok(())
    }
    fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
        match self.stream.pop_front() {
            None => Poll::Idle,
            Some(entry) => {
                if let Some(item) = entry.get("item") {
                    Poll::Item(item.clone())
                } else if let Some(reason) = entry.get("closed") {
                    Poll::Closed(reason.clone())
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

/// Runs an order vector file. The result lists a line for every case that fails.
pub fn order_vector_failures(file: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    for case in file["cases"].as_array().expect("cases is an array") {
        let name = case["name"].as_str().unwrap_or("?");
        let mut driver = VectorDriver {
            stream: case["stream"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into(),
            executed: Vec::new(),
        };
        let limits = Limits {
            max_polls: case
                .get("max_polls")
                .and_then(Value::as_u64)
                .map_or(Limits::default().max_polls, |n| n as usize),
            step_timeout: None,
        };
        let mut binds = Bindings::new();
        let outcome = run_step(
            &case["step"],
            &mut driver,
            &mut binds,
            &crate::matcher::NoSchemas,
            &limits,
        );
        let got = match outcome {
            Ok(()) => "passed",
            Err(StepError::Failed { .. }) => "failed",
            Err(StepError::Inconclusive { .. }) => "inconclusive",
            Err(StepError::Unsupported { .. }) => "unsupported",
            Err(StepError::TranscriptDefect { .. }) => "transcript_defect",
        };
        let want = case["result"].as_str().unwrap_or("?");
        if got != want {
            failures.push(format!("{name}: expected {want}, got {got}"));
            continue;
        }
        if let Some(want_binds) = case.get("bindings") {
            if &bindings_json(&binds) != want_binds {
                failures.push(format!(
                    "{name}: bindings {} differ from {want_binds}",
                    bindings_json(&binds)
                ));
            }
        }
        if let Some(want_exec) = case.get("executed") {
            if &json!(driver.executed) != want_exec {
                failures.push(format!(
                    "{name}: executed {} differ from {want_exec}",
                    json!(driver.executed)
                ));
            }
        }
    }
    failures
}
