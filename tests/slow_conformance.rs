//! The Core conformance suite on `RealCoreHarness` (plan sections 4.2 and 5, the real-process tier): the trials of
//! `conformance.rs` on the real `Core::open` with real worker processes and the real probe, all prebuilt and verified against
//! the candidate manifest (`cargo xtask prebuild-worker`). `harness = false`; the `slow` feature builds it.
//!
//! A pending, deferred or withdrawn id is reported as on the default tier and never counted as passed. A real
//! implementation ignores the seed (plan 4.2c).

mod suite;

use botster_core_testkit::candidate::Candidate;
use botster_core_testkit::real::RealCoreHarness;

fn main() {
    suite::run(
        "real conformance",
        suite::Tier::Real,
        |_seed| {
            let dir = Candidate::beside_test_binary().expect("the candidate directory");
            let candidate = Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"));
            Box::new(
                RealCoreHarness::new(candidate)
                    .expect("the harness's root, guard and probe wrapper")
                    .with_core_type(suite::CORE_IS_SEND_NOT_SYNC),
            )
        },
        // A real clock: the wall clock ends a waiting step, never the poll count (design 6.1, R-43; contracts-v0.1.22).
        suite::Limits::real(),
    );
}
