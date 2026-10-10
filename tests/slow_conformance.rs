//! The Core conformance suite on `RealCoreHarness` (plan sections 4.2 and 5, the real-process tier): the trials of
//! `conformance.rs` on Core's own composition (`open_parts`, then `HostDriver` over the pass-through `EdgeTap`) with real
//! worker processes and the real probe, all prebuilt and verified against the candidate manifest (`cargo xtask prebuild-worker`). `harness = false`; the `slow` feature builds it.
//!
//! A pending, deferred or withdrawn id is reported as on the default tier and never counted as passed; an id of
//! `core-real-pending.txt` runs and is reported as pending-real (plan 23l). Every id that passes also runs on a plain
//! `Core::open` (the pass-through test). A real implementation ignores the seed (plan 4.2c).

mod suite;

use botster_core_testkit::candidate::Candidate;
use botster_core_testkit::real::RealCoreHarness;

fn candidate() -> Candidate {
    let dir = Candidate::beside_test_binary().expect("the candidate directory");
    Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"))
}

fn main() {
    suite::run(
        "real conformance",
        |_seed| {
            Box::new(
                RealCoreHarness::new(candidate())
                    .expect("the harness's root, guard and probe wrapper")
                    .with_core_type(suite::CORE_IS_SEND_NOT_SYNC),
            )
        },
        // The pass-through test (plan 23l): the same transcript on a plain `Core::open`, with no control.
        Some(|_seed| {
            Box::new(
                RealCoreHarness::plain(candidate())
                    .expect("the harness's root, guard and probe wrapper")
                    .with_core_type(suite::CORE_IS_SEND_NOT_SYNC),
            )
        }),
        // A real clock: the wall clock ends a waiting step, never the poll count (design 6.1, R-43; contracts-v0.1.22).
        suite::Limits::real(),
    );
}
