//! The Core conformance suite on `RealCoreHarness` (plan sections 4.2 and 5, the real-process tier): the trials of
//! `conformance.rs` on the real `Core::open` with real worker processes and the real probe, all prebuilt and verified against
//! the candidate manifest (`cargo xtask prebuild-worker`). `harness = false`; the `slow` feature builds it.
//!
//! A pending, deferred or withdrawn id is reported as on the default tier and never counted as passed. A real
//! implementation ignores the seed (plan 4.2c).

mod suite;

use botster_core_testkit::candidate::Candidate;
use botster_core_testkit::real::RealCoreHarness;

/// PROBE SEAM, not for merge: the step timeout of a probe run, in milliseconds, set only by the gate command. The harness PR
/// replaces it with the step limit that the contracts export (lead, 2026-10-09). Without it the step timeout stays `None`.
const PROBE_STEP_TIMEOUT_MS: &str = "BOTSTER_PROBE_STEP_TIMEOUT_MS";

fn main() {
    let step_timeout = std::env::var(PROBE_STEP_TIMEOUT_MS).ok().map(|ms| {
        let ms = ms
            .parse()
            .unwrap_or_else(|error| panic!("{PROBE_STEP_TIMEOUT_MS}={ms}: {error}"));
        std::time::Duration::from_millis(ms)
    });
    println!("real conformance: step_timeout {step_timeout:?}");
    suite::run(
        "real conformance",
        |_seed| {
            let dir = Candidate::beside_test_binary().expect("the candidate directory");
            let candidate = Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"));
            Box::new(
                RealCoreHarness::new(candidate)
                    .expect("the harness's root, guard and probe wrapper")
                    .with_core_type(suite::CORE_IS_SEND_NOT_SYNC),
            )
        },
        suite::Limits {
            step_timeout,
            ..suite::Limits::default()
        },
    );
}
