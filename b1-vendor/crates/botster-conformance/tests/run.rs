//! The runner: results, seeds, requirements, selection and the report.

use botster_conformance::{
    load::parse_transcript, report::Report, run_transcript, Bindings, Deadline, Limits, NoSchemas,
    Outcome, Poll, SeedSet, Selection, StepDriver, StepError,
};
use serde_json::{json, Value};
use std::cell::RefCell;

/// A subject that gives scripted items. A step `{"fail_on_seed": n}` fails under seed n.
struct Script {
    seed: u64,
    items: Vec<Value>,
    fake: bool,
}

impl StepDriver for Script {
    fn is_fake(&self) -> bool {
        self.fake
    }
    fn has_feature(&self, name: &str) -> bool {
        name == "have"
    }
    fn has_control(&self, op: &str) -> bool {
        op == "have_control"
    }
    fn setup(&mut self, _: &Value) -> Result<(), StepError> {
        Ok(())
    }
    fn exec(&mut self, step: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
        if let Some(n) = step.get("fail_on_seed").and_then(Value::as_u64) {
            if n == self.seed {
                return Err(StepError::Failed {
                    expected: json!("ok"),
                    received: json!("boom"),
                });
            }
            return Ok(());
        }
        if let Some(item) = step.get("emit") {
            self.items.push(item.clone());
            return Ok(());
        }
        Err(StepError::Unsupported {
            op: "mystery".into(),
        })
    }
    fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
        if self.items.is_empty() {
            Poll::Idle
        } else {
            Poll::Item(self.items.remove(0))
        }
    }
}

fn transcript(extra: Value, steps: Value) -> botster_conformance::Transcript {
    let mut doc =
        json!({"id":"conf::t","contract":"hc","clause":"EN-8","subject":"hub","steps":steps});
    doc.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    parse_transcript("conf::t.json", &doc.to_string()).unwrap()
}

fn run(t: &botster_conformance::Transcript, seeds: &[u64]) -> (Outcome, Vec<u64>) {
    let seen = RefCell::new(Vec::new());
    let make = |seed: u64| -> Box<dyn StepDriver> {
        seen.borrow_mut().push(seed);
        Box::new(Script {
            seed,
            items: Vec::new(),
            fake: false,
        })
    };
    let outcome = run_transcript(
        t,
        &make,
        &SeedSet {
            seeds: seeds.to_vec(),
        },
        &NoSchemas,
        &Limits::default(),
    );
    let seeds_run = seen.borrow().clone();
    (outcome, seeds_run)
}

#[test]
fn a_passing_transcript_runs_every_seed() {
    let t = transcript(json!({}), json!([{"emit": {"a": 1}}, {"expect": {"a": 1}}]));
    assert_eq!(run(&t, &[0, 1, 2]), (Outcome::Passed, vec![0, 1, 2]));
}

#[test]
fn a_failure_names_the_step_the_values_and_the_seed() {
    let t = transcript(json!({}), json!([{"emit": 1}, {"fail_on_seed": 1}]));
    let (outcome, seeds_run) = run(&t, &[0, 1, 2]);
    assert_eq!(
        outcome,
        Outcome::Failed {
            step: 1,
            expected: json!("ok"),
            received: json!("boom"),
            seed: 1
        }
    );
    assert_eq!(
        seeds_run,
        vec![0, 1],
        "the run stops at the first failing seed"
    );
}

#[test]
fn pinned_seeds_replace_the_run_seeds() {
    let t = transcript(json!({"seeds": [5]}), json!([{"emit": 1}]));
    assert_eq!(run(&t, &[0, 1]), (Outcome::Passed, vec![5]));
}

#[test]
fn a_missing_feature_is_not_applicable_never_a_pass() {
    let t = transcript(json!({"requires": {"features": ["absent"]}}), json!([]));
    assert_eq!(
        run(&t, &[0]).0,
        Outcome::NotApplicable {
            feature: "absent".into()
        }
    );
}

#[test]
fn a_missing_control_is_unsupported_never_a_pass() {
    let t = transcript(json!({"requires": {"controls": ["absent"]}}), json!([]));
    assert_eq!(
        run(&t, &[0]).0,
        Outcome::UnsupportedControl {
            op: "absent".into()
        }
    );
}

#[test]
fn an_unsupported_step_gives_unsupported_control() {
    let t = transcript(json!({}), json!([{"mystery": 1}]));
    assert_eq!(
        run(&t, &[0]).0,
        Outcome::UnsupportedControl {
            op: "mystery".into()
        }
    );
}

#[test]
fn a_waiting_step_that_hits_its_limit_is_inconclusive() {
    struct Forever;
    impl StepDriver for Forever {
        fn has_feature(&self, _: &str) -> bool {
            true
        }
        fn has_control(&self, _: &str) -> bool {
            true
        }
        fn setup(&mut self, _: &Value) -> Result<(), StepError> {
            Ok(())
        }
        fn exec(&mut self, _: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
            Ok(())
        }
        fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
            Poll::Pending
        }
    }
    let t = transcript(json!({}), json!([{"expect": {"a": 1}}]));
    let make = |_: u64| -> Box<dyn StepDriver> { Box::new(Forever) };
    let outcome = run_transcript(
        &t,
        &make,
        &SeedSet { seeds: vec![3] },
        &NoSchemas,
        &Limits {
            max_polls: 10,
            step_timeout: None,
        },
    );
    assert_eq!(
        outcome,
        Outcome::Inconclusive {
            step: 0,
            limit: "max_polls".into(),
            seed: 3
        }
    );
}

#[test]
fn real_limits_end_a_step_by_the_wall_clock_not_the_poll_count() {
    let real = Limits::real();
    assert_eq!(real.step_timeout, Some(Limits::REAL_STEP_TIMEOUT));
    assert_eq!(real.max_polls, usize::MAX);
    assert_ne!(real, Limits::default());
}

#[test]
fn a_withdrawn_transcript_with_no_steps_passes_without_a_subject() {
    let t = transcript(json!({"kind": "withdrawn"}), json!([]));
    assert_eq!(run(&t, &[0, 1]), (Outcome::Passed, vec![]));
}

#[test]
fn the_file_name_must_equal_the_id() {
    let doc = json!({"id":"conf::t","contract":"hc","clause":"EN-8","subject":"hub"}).to_string();
    assert!(parse_transcript("conf::other.json", &doc).is_err());
    assert!(parse_transcript("conf::t.json", &doc).is_ok());
}

#[test]
fn an_unknown_envelope_field_is_refused() {
    let doc = json!({"id":"conf::t","contract":"hc","clause":"EN-8","subject":"hub","typo":1})
        .to_string();
    assert!(parse_transcript("conf::t.json", &doc).is_err());
}

#[test]
fn seed_sets_parse() {
    assert_eq!(SeedSet::local().seeds, (0..8).collect::<Vec<_>>());
    assert_eq!(SeedSet::ci().seeds.len(), 32);
    assert_eq!(SeedSet::parse("0-3").unwrap().seeds, vec![0, 1, 2, 3]);
    assert_eq!(SeedSet::parse("5,1,3-4,1").unwrap().seeds, vec![1, 3, 4, 5]);
    assert!(SeedSet::parse("4-2").is_err());
    assert!(SeedSet::parse("x").is_err());
}

#[test]
fn selection_filters_by_prefix_clause_and_seed() {
    let t = transcript(json!({}), json!([]));
    let args = |a: &[&str]| Selection::from_args(a.iter().map(|s| s.to_string())).unwrap();
    assert!(args(&[]).selects(&t));
    assert!(args(&["--only", "conf::"]).selects(&t));
    assert!(!args(&["--only", "conf::x"]).selects(&t));
    assert!(args(&["--clause", "EN-8"]).selects(&t));
    assert!(!args(&["--clause", "EN-9"]).selects(&t));
    assert_eq!(
        args(&["--seed", "4"]).seeds(&SeedSet::local()).seeds,
        vec![4]
    );
    assert!(Selection::from_args(["--nope".to_string()]).is_err());
}

#[test]
fn the_report_lists_failures_with_a_reproduce_command() {
    let mut report = Report::new("botster-hub-conformance");
    report.push("conf::ok", Outcome::Passed);
    report.push(
        "conf::en_8_x",
        Outcome::Failed {
            step: 2,
            expected: json!({"a": 1}),
            received: json!({"a": 2}),
            seed: 7,
        },
    );
    report.push(
        "conf::na",
        Outcome::NotApplicable {
            feature: "f".into(),
        },
    );
    assert!(!report.is_clean());
    let mut out = Vec::new();
    report.print(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("FAIL conf::en_8_x seed=7 step=2"), "{text}");
    assert!(
        text.contains(
            "BOTSTER_SEEDS=7 cargo test -p botster-hub-conformance -- --exact conf_en_8_x"
        ),
        "{text}"
    );
    assert!(text.contains("NOT_APPLICABLE conf::na feature=f"), "{text}");
    assert!(
        text.contains(
            "passed 1, failed 1, inconclusive 0, not_applicable 1, unsupported_control 0"
        ),
        "{text}"
    );
}

#[test]
fn the_driver_rng_is_deterministic_per_seed_and_id() {
    use rand_core::RngCore;
    let draw = |seed, id| botster_conformance::driver_rng(seed, id).next_u64();
    assert_eq!(draw(1, "conf::a"), draw(1, "conf::a"));
    assert_ne!(draw(1, "conf::a"), draw(2, "conf::a"));
    assert_ne!(draw(1, "conf::a"), draw(1, "conf::b"));
}

#[test]
fn the_transcript_schema_is_stable_json() {
    let text = botster_conformance::transcript_schema_json();
    assert!(text.ends_with('\n'));
    let schema: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(schema["title"], "Transcript");
    assert_eq!(text, botster_conformance::transcript_schema_json());
}

fn run_on_fake(t: &botster_conformance::Transcript) -> Outcome {
    let make = |seed: u64| -> Box<dyn StepDriver> {
        Box::new(Script {
            seed,
            items: Vec::new(),
            fake: true,
        })
    };
    run_transcript(
        t,
        &make,
        &SeedSet { seeds: vec![0] },
        &NoSchemas,
        &Limits::default(),
    )
}

// Design 5.1 `fake`: a structural transcript is only checked structurally on a fake, and it runs in full on a real subject.
#[test]
fn a_structural_transcript_runs_in_full_only_on_a_real_subject() {
    let steps = json!([{"fail_on_seed": 0}]);
    let t = transcript(
        json!({"fake": "structural", "ruling": "R-5"}),
        steps.clone(),
    );
    assert!(run_on_fake(&t).is_pass());
    assert!(matches!(run(&t, &[0]).0, Outcome::Failed { .. }));
    // A structural transcript without a ruling fails on a fake: the list of real-only ids names the ruling of each.
    let t = transcript(json!({"fake": "structural"}), steps);
    assert!(matches!(run_on_fake(&t), Outcome::Failed { .. }));
}

// Design 5.6, rule 6: an order construct takes `because`, and the check lists the expectations that lack a well-formed one.
#[test]
fn every_expectation_must_cite_a_clause() {
    let t = transcript(
        json!({}),
        json!([
            {"emit": 1},
            {"expect": 1, "because": "HC EN-8"},
            {"emit": 2},
            {"expect": 2},
            {"emit": 3},
            {"expect": 3, "because": "no clause"},
            {"control": {"op": "x"}, "expect": {}},
            {"route_expect_pty": []}
        ]),
    );
    assert_eq!(
        botster_conformance::citation::missing_because(&t),
        vec![3, 5, 6, 7]
    );
    assert!(botster_conformance::citation::is_citation("Codec TS-5b"));
    assert!(!botster_conformance::citation::is_citation("TS-5b"));
}

// The CLI suite's observation steps are expectations too (design 5.5, 5.6 rule 6): each needs a `because`.
#[test]
fn the_cli_observation_steps_must_cite_a_clause() {
    let t = transcript(
        json!({}),
        json!([
            {"expect_exit": {"code": 0}},
            {"expect_stdout": "x", "because": "CLI CLI-G1"},
            {"expect_stderr": "x"},
            {"expect_file": {"path": "a"}},
            {"expect_exit": {"code": 0}, "because": "CLI CLI-G2"},
            {"expect_file": {"path": "a"}, "because": "CLI CLI-G3"}
        ]),
    );
    assert_eq!(
        botster_conformance::citation::missing_because(&t),
        vec![0, 2, 3]
    );
}

#[test]
fn an_expectation_with_because_still_runs() {
    let t = transcript(
        json!({}),
        json!([{"emit": 1}, {"expect": 1, "because": "HC EN-8"}]),
    );
    assert!(run(&t, &[0]).0.is_pass());
}

/// A subject that records the runner's calls, and whose `start` may fail.
struct Recorder {
    calls: std::rc::Rc<RefCell<Vec<String>>>,
    start_error: Option<StepError>,
}

impl StepDriver for Recorder {
    fn has_feature(&self, name: &str) -> bool {
        self.calls.borrow_mut().push(format!("has_feature {name}"));
        true
    }
    fn has_control(&self, op: &str) -> bool {
        self.calls.borrow_mut().push(format!("has_control {op}"));
        op == "have_control"
    }
    fn start(&mut self, steps: &[Value]) -> Result<(), StepError> {
        self.calls
            .borrow_mut()
            .push(format!("start {}", Value::from(steps.to_vec())));
        self.start_error.take().map_or(Ok(()), Err)
    }
    fn setup(&mut self, _: &Value) -> Result<(), StepError> {
        self.calls.borrow_mut().push("setup".into());
        Ok(())
    }
    fn exec(&mut self, step: &Value, _: &mut Bindings, _: &Deadline) -> Result<(), StepError> {
        self.calls.borrow_mut().push(format!("exec {step}"));
        Ok(())
    }
    fn next(&mut self, _: &Value, _: &Deadline) -> Poll {
        Poll::Idle
    }
}

fn record(
    t: &botster_conformance::Transcript,
    start_error: Option<StepError>,
) -> (Outcome, Vec<String>) {
    let calls = std::rc::Rc::new(RefCell::new(Vec::new()));
    let error = RefCell::new(start_error);
    let make = |_: u64| -> Box<dyn StepDriver> {
        Box::new(Recorder {
            calls: calls.clone(),
            start_error: error.borrow_mut().take(),
        })
    };
    let outcome = run_transcript(
        t,
        &make,
        &SeedSet { seeds: vec![0] },
        &NoSchemas,
        &Limits::default(),
    );
    let calls = calls.borrow().clone();
    (outcome, calls)
}

/// Steward ruling R-43(5): `start` takes the transcript's steps after the `requires.controls` check and before `setup`, so a
/// driver can prepare the run (a test control) before the first step and before `setup` opens the subject.
#[test]
fn start_takes_the_steps_after_the_control_check_and_before_setup() {
    let t = transcript(
        json!({"requires": {"controls": ["have_control"], "features": ["f"]}}),
        json!([{"x": 1}, {"y": 2}]),
    );
    assert_eq!(
        record(&t, None),
        (
            Outcome::Passed,
            vec![
                "has_control have_control".to_string(),
                r#"start [{"x":1},{"y":2}]"#.into(),
                "setup".into(),
                "has_feature f".into(),
                r#"exec {"x":1}"#.into(),
                r#"exec {"y":2}"#.into(),
            ]
        )
    );
}

#[test]
fn a_missing_control_is_reported_before_start() {
    let t = transcript(
        json!({"requires": {"controls": ["absent"]}}),
        json!([{"x": 1}]),
    );
    assert_eq!(
        record(&t, None),
        (
            Outcome::UnsupportedControl {
                op: "absent".into()
            },
            vec!["has_control absent".to_string()]
        )
    );
}

/// An error of `start` fails the transcript at step 0, and no step runs: the run never goes on without what `start` prepares.
#[test]
fn an_error_of_start_fails_the_transcript_at_step_0() {
    let t = transcript(json!({}), json!([{"x": 1}]));
    let error = StepError::Failed {
        expected: json!("the control runs"),
        received: json!("no"),
    };
    assert_eq!(
        record(&t, Some(error)),
        (
            Outcome::Failed {
                step: 0,
                expected: json!("the control runs"),
                received: json!("no"),
                seed: 0
            },
            vec![r#"start [{"x":1}]"#.to_string()]
        )
    );
    let unsupported = StepError::Unsupported { op: "c".into() };
    assert_eq!(
        record(&t, Some(unsupported)).0,
        Outcome::UnsupportedControl { op: "c".into() }
    );
}

/// The default `start` (the Hub drivers keep it) does nothing: `Script` does not define it, and its transcripts run as before.
#[test]
fn the_default_start_does_nothing() {
    let mut s = Script {
        seed: 0,
        items: Vec::new(),
        fake: false,
    };
    assert_eq!(s.start(&[json!({"emit": 1})]), Ok(()));
    assert!(s.items.is_empty());
    let t = transcript(json!({}), json!([{"emit": {"a": 1}}, {"expect": {"a": 1}}]));
    assert_eq!(run(&t, &[0]), (Outcome::Passed, vec![0]));
}
