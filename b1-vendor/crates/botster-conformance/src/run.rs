//! Running one transcript over its seeds (design 6.1).

use crate::driver::{StepDriver, StepError};
use crate::matcher::{Bindings, SchemaResolver};
use crate::order::{run_steps, Limits};
use crate::report::test_name;
use crate::result::Outcome;
use crate::seeds::SeedSet;
use crate::stimulus::prepare;
use crate::transcript::{FakeMode, Transcript, TranscriptKind};
use include_dir::Dir;
use serde_json::Value;

/// Builds the subject for one seed.
pub type DriverFactory<'a> = &'a dyn Fn(u64) -> Box<dyn StepDriver>;

/// Runs a transcript under each seed. The result is the first result that is not `passed`, or `passed`.
///
/// A transcript with `seeds` runs those seeds only. A `withdrawn` transcript with no steps passes without a subject (design section 2).
pub fn run_transcript(
    transcript: &Transcript,
    make: DriverFactory<'_>,
    seeds: &SeedSet,
    schemas: &dyn SchemaResolver,
    limits: &Limits,
) -> Outcome {
    if transcript.kind == Some(TranscriptKind::Withdrawn) && transcript.steps.is_empty() {
        return Outcome::Passed;
    }
    let seed_list = transcript.seeds.as_ref().unwrap_or(&seeds.seeds);
    if seed_list.is_empty() {
        // A run that executes nothing proves nothing (design 6.1): never a pass.
        return Outcome::Failed {
            step: 0,
            expected: serde_json::json!("at least one seed"),
            received: serde_json::json!("an empty seed list"),
            seed: 0,
        };
    }
    for &seed in seed_list {
        let outcome = run_seed(transcript, make, seed, schemas, limits);
        if !outcome.is_pass() {
            return outcome;
        }
    }
    Outcome::Passed
}

fn run_seed(
    t: &Transcript,
    make: DriverFactory<'_>,
    seed: u64,
    schemas: &dyn SchemaResolver,
    limits: &Limits,
) -> Outcome {
    let mut driver = make(seed);
    if t.fake == Some(FakeMode::Structural) && driver.is_fake() {
        return structural(t, schemas, seed);
    }
    for op in &t.requires.controls {
        if !driver.has_control(op) {
            return Outcome::UnsupportedControl { op: op.clone() };
        }
    }
    if let Err(error) = driver.start(&t.steps) {
        return from_error(0, error, seed);
    }
    if let Err(error) = driver.setup(&t.setup) {
        return from_error(0, error, seed);
    }
    // A subject reports its features through the handle that `setup` opened (Core A2-6), so the check follows the setup.
    for feature in &t.requires.features {
        if !driver.has_feature(feature) {
            return Outcome::NotApplicable {
                feature: feature.clone(),
            };
        }
    }
    let mut bindings = Bindings::new();
    match run_steps(&t.steps, driver.as_mut(), &mut bindings, schemas, limits) {
        Ok(()) => Outcome::Passed,
        Err(failure) => from_error(failure.step, failure.error, seed),
    }
}

/// The structural check of a transcript whose subject the fake does not model (design 7.1): it needs a `ruling`, every valid frame
/// validates, and every typed negative frame fails its schema. A step with an unbound variable cannot be checked before a run.
fn structural(t: &Transcript, schemas: &dyn SchemaResolver, seed: u64) -> Outcome {
    let fail = |step, expected: Value, received: Value| Outcome::Failed {
        step,
        expected,
        received,
        seed,
    };
    if t.ruling.is_none() {
        return fail(
            0,
            "a structural transcript names its ruling".into(),
            "no ruling".into(),
        );
    }
    for (i, step) in t.steps.iter().enumerate() {
        match prepare(step, &Bindings::new(), schemas) {
            Ok(_) => {}
            Err(StepError::Failed { received, .. })
                if received
                    .as_str()
                    .is_some_and(|r| r.starts_with("unbound variable")) => {}
            Err(StepError::Failed { expected, received }) => return fail(i, expected, received),
            Err(_) => {}
        }
    }
    Outcome::Passed
}

fn from_error(step: usize, error: StepError, seed: u64) -> Outcome {
    match error {
        StepError::Unsupported { op } => Outcome::UnsupportedControl { op },
        StepError::Failed { expected, received } => Outcome::Failed {
            step,
            expected,
            received,
            seed,
        },
        StepError::Inconclusive { limit } => Outcome::Inconclusive { step, limit, seed },
        // A transcript defect fails the run on every tier; the report names the rule.
        StepError::TranscriptDefect { rule, detail } => Outcome::Failed {
            step,
            expected: serde_json::json!({ "transcript_defect": rule }),
            received: detail,
            seed,
        },
    }
}

/// Runs one transcript by id for a generated `#[test]` (see [`crate::conformance_tests`]). Panics with the report line unless it passed.
///
/// The seed set and the selection come from `BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE` and `BOTSTER_SEED`.
pub fn run_one_test(
    crate_name: &str,
    dir: &Dir<'_>,
    id: &str,
    make: DriverFactory<'_>,
    schemas: &dyn SchemaResolver,
) {
    let transcript = crate::load::load_one(dir, id)
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("no transcript with id {id}"));
    let transcript = &transcript;
    let selection = crate::seeds::Selection::from_env();
    if !selection.selects(transcript) {
        return;
    }
    let seeds = selection.seeds(&crate::seeds::SeedSet::from_env());
    let outcome = run_transcript(transcript, make, &seeds, schemas, &Limits::default());
    if !outcome.is_pass() {
        panic!(
            "{}\n{}",
            crate::report::describe(id, &outcome),
            crate::report::reproduce(crate_name, &test_name(id), &outcome)
        );
    }
}
