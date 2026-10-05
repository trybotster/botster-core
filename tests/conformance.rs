//! The Core conformance suite, one trial per Core id of the pinned ledger (plan section 5). `harness = false`.
//!
//! The runner's `conformance_tests!` macro cannot mark an id as not yet expected and cannot see a ledger id without a
//! transcript, so this harness drives the runner's public functions instead:
//!
//! - an id in `conformance/core-pending.txt` is an ignored trial of kind `pending`;
//! - an id in `conformance/core-deferred.toml` is an ignored trial of kind `deferred`;
//! - an id in the contracts' `withdrawn.txt` is an ignored trial of kind `withdrawn`: never pending, never a pass;
//! - a `not-applicable` line of the contracts' `deferred.txt` names a CASE of an active id: the id runs, and the report lists the
//!   case;
//! - a ledger id without a transcript that is not deferred is an ignored trial of kind `pending: no transcript`;
//! - every other id runs `run_transcript` over the seed set and passes only on `Outcome::Passed`.
//!
//! A pending or deferred id is never counted as passed. `cargo xtask ci` validates the three files. The seed set and the
//! selection come from the runner's environment variables (`BOTSTER_SEEDS`, `BOTSTER_ONLY`, `BOTSTER_CLAUSE`, `BOTSTER_SEED`).

mod suite;

use botster_core_testkit::TestkitHarness;

/// The default tier: `TestkitHarness`. With the `slow` feature, `slow_conformance.rs` runs the same trials on
/// `RealCoreHarness` (plan section 5).
fn main() {
    suite::run(|seed| Box::new(TestkitHarness::new(seed)));
}
