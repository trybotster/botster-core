//! The codec transcripts against `botster-fake-route` (design 7.1: subject `worker`). One `#[test]` per transcript id, under the run's seed set.

use botster_conformance::StepDriver;
use botster_hub_conformance::route::fake::FakeRouteSubject;
use botster_hub_conformance::route::{RouteDriver, RouteSchemas};

fn make(seed: u64) -> Box<dyn StepDriver> {
    Box::new(RouteDriver::new(FakeRouteSubject::new(seed)))
}

botster_hub_conformance::codec_conformance_tests!(make, RouteSchemas);

// Design 5.6, rule 6: every expectation of every codec transcript cites the clause that fixes its value.
#[test]
fn every_codec_expectation_cites_its_clause() {
    let mut missing = vec![];
    for t in botster_conformance::load_dir(&botster_hub_conformance::CODEC_TRANSCRIPTS).unwrap() {
        for step in botster_conformance::citation::missing_because(&t) {
            missing.push(format!("{} step {step}", t.id));
        }
    }
    assert!(missing.is_empty(), "no `because`: {missing:#?}");
}
