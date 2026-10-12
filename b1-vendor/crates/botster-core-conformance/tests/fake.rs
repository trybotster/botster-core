//! The Core transcripts against FakeCore (design 7.1: subject `core`). One `#[test]` per transcript id, under the run's seed set.

use botster_conformance::StepDriver;
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{driver_for, CoreSchemas};

fn make(seed: u64) -> Box<dyn StepDriver> {
    driver_for(Box::new(FakeCoreHarness::new(seed)))
}

botster_core_conformance::conformance_tests!(make, CoreSchemas);

// Design 5.6, rule 6: every expectation of every Core transcript cites the clause that fixes its value.
#[test]
fn every_core_expectation_cites_its_clause() {
    let mut missing = vec![];
    for t in botster_conformance::load_dir(&botster_core_conformance::CORE_TRANSCRIPTS).unwrap() {
        for step in botster_conformance::citation::missing_because(&t) {
            missing.push(format!("{} step {step}", t.id));
        }
    }
    assert!(missing.is_empty(), "no `because`: {missing:#?}");
}
