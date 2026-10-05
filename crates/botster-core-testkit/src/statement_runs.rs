//! Statement reports that the harness can dispatch after Core wiring lands (Core A5-1 and A5-3).
//!
//! A report preserves every missing proof. A sync-column row is permission to script a refusal, not proof of reachability.

use botster_core_conformance::{run_script_events, ControlError, CoreHarness};
use serde_json::{json, Value};

/// Run the script on fresh harnesses. The factory must use the same seed and configuration for every run.
/// The conformance driver checks event equality and instance identity. This function never normalizes or reorders events.
pub fn run_deterministic(
    spec: &Value,
    mut fresh: impl FnMut() -> Box<dyn CoreHarness>,
) -> Result<Value, ControlError> {
    collect_runs(spec, |steps| {
        run_script_events(fresh(), steps).map_err(|error| ControlError::Bad(format!("{error:?}")))
    })
}

fn collect_runs(
    spec: &Value,
    mut run: impl FnMut(&[Value]) -> Result<Value, ControlError>,
) -> Result<Value, ControlError> {
    let count = match spec.get("runs") {
        None => 2,
        Some(value) => value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n >= 2)
            .ok_or_else(|| ControlError::Bad("runs must be an integer of at least two".into()))?,
    };
    let steps = spec
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| ControlError::Bad("steps must be an array".into()))?;
    let runs: Vec<Value> = (0..count).map(|_| run(steps)).collect::<Result<_, _>>()?;
    Ok(json!({"runs": runs}))
}

/// Execute each reachability probe through the caller's real Core or refusal layer.
/// `probe` returns true only after it observes the code at the requested timing.
/// The callback receives the category and the unchanged specification for that case.
/// Unsupported sync-column entries are harness errors. Other missing proofs stay in `not_reached`.
pub fn error_codes_reachable(
    spec: &Value,
    mut probe: impl FnMut(&str, &Value) -> Result<bool, ControlError>,
) -> Result<Value, ControlError> {
    let mut missing = Vec::new();
    for category in ["scripted_sync", "by_edge", "by_limit", "by_input"] {
        let cases = spec
            .get(category)
            .and_then(Value::as_array)
            .ok_or_else(|| ControlError::Bad(format!("{category} must be an array")))?;
        for case in cases {
            if category == "scripted_sync" {
                let call = case
                    .get("op")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ControlError::Bad("a sync probe needs op".into()))?;
                let code = case
                    .get("code")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ControlError::Bad("a sync probe needs code".into()))?;
                let row = crate::refusal::row(call)
                    .ok_or_else(|| ControlError::Bad(format!("no sync row for {call}")))?;
                if !row.codes.contains(&code) {
                    return Err(ControlError::Refused(json!({"call": call, "code": code,
                        "reason": "not_in_sync_column"})));
                }
            }
            if !probe(category, case)? {
                missing.push(json!({"category": category, "case": case}));
            }
        }
    }
    let typed = spec
        .get("typed_outcomes")
        .and_then(|v| v.get("reached"))
        .and_then(Value::as_array)
        .ok_or_else(|| ControlError::Bad("typed_outcomes.reached must be an array".into()))?;
    for case in typed {
        if !probe("typed_outcomes", case)? {
            missing.push(json!({"category": "typed_outcomes", "case": case}));
        }
    }
    let mut report = json!({"not_reached": missing});
    for key in ["no_scenario", "open_steward"] {
        if let Some(value) = spec.get(key) {
            report[key] = value.clone();
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_reports_keep_each_run_in_order() {
        let steps = vec![json!({"input": 1})];
        for count in [None, Some(3)] {
            let mut spec = json!({"steps": steps});
            if let Some(count) = count {
                spec["runs"] = json!(count);
            }
            let mut calls = 0;
            let report = collect_runs(&spec, |seen| {
                assert_eq!(seen, steps);
                calls += 1;
                Ok(json!([{"instance": calls}]))
            })
            .unwrap();
            assert_eq!(calls, count.unwrap_or(2));
            let expected: Vec<Value> = (1..=calls).map(|n| json!([{"instance": n}])).collect();
            assert_eq!(report, json!({"runs": expected}));
        }
    }

    #[test]
    fn deterministic_runs_reject_invalid_specs_and_propagate_failure() {
        for spec in [
            json!({"runs": 1, "steps": []}),
            json!({"runs": "two", "steps": []}),
            json!({}),
        ] {
            assert!(collect_runs(&spec, |_| panic!("invalid input must not run")).is_err());
        }
        assert!(
            collect_runs(&json!({"steps": []}), |_| Err(ControlError::Bad(
                "failed".into()
            )))
            .is_err()
        );
        // Core LC-1: a missing worker path fails even after the real harness can open a valid Core.
        assert!(run_deterministic(
            &json!({"steps": [{"open": {"worker": null}}]}),
            || Box::new(crate::TestkitHarness::new(0))
        )
        .is_err());
    }

    fn spec() -> Value {
        json!({"scripted_sync": [{"op": "Start", "code": "WrongState"}],
            "by_edge": [{"code": "WorkerLinkFailed", "edge": "process"}],
            "by_limit": ["PendingLimit"], "by_input": ["MissingWorkerPath"],
            "typed_outcomes": {"reached": ["Written"]},
            "no_scenario": [{"code": "Internal"}], "open_steward": []})
    }

    #[test]
    fn reachability_preserves_every_missing_probe_and_exclusion() {
        let spec = spec();
        let mut seen = Vec::new();
        let report = error_codes_reachable(&spec, |category, case| {
            seen.push(json!({"category": category, "case": case}));
            Ok(false)
        })
        .unwrap();
        assert_eq!(seen.len(), 5);
        assert_eq!(
            report,
            json!({"not_reached": seen, "no_scenario": spec["no_scenario"], "open_steward": []})
        );
        assert_eq!(
            error_codes_reachable(&spec, |_, _| Ok(true)).unwrap()["not_reached"],
            json!([])
        );
        assert!(
            error_codes_reachable(&spec, |_, _| Err(ControlError::Bad("failed".into()))).is_err()
        );
    }

    #[test]
    fn reachability_requires_real_probes_and_valid_sync_rows() {
        for case in [
            json!({}),
            json!({"op": "unknown", "code": "WrongState"}),
            json!({"op": "Start"}),
        ] {
            let mut spec = spec();
            spec["scripted_sync"] = json!([case]);
            assert!(
                error_codes_reachable(&spec, |_, _| panic!("invalid row must not run")).is_err()
            );
        }
        let mut spec = spec();
        spec["scripted_sync"][0]["code"] = json!("RegistryFailed");
        assert!(matches!(
            error_codes_reachable(&spec, |_, _| panic!("an async failure cannot be scripted")),
            Err(ControlError::Refused(_))
        ));
        for key in [
            "scripted_sync",
            "by_edge",
            "by_limit",
            "by_input",
            "typed_outcomes",
        ] {
            let mut spec = super::tests::spec();
            spec.as_object_mut().unwrap().remove(key);
            assert!(error_codes_reachable(&spec, |_, _| Ok(true)).is_err());
        }
    }
}
