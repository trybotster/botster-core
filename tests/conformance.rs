//! The Core conformance suite on `TestkitHarness` (plan section 5, the default tier), one trial per Core id of the pinned
//! ledger: see `suite/mod.rs`. `harness = false`.

mod suite;

use botster_core_testkit::TestkitHarness;

fn main() {
    // A fake: no step timeout (design 6.1).
    suite::run(
        "conformance",
        |seed| Box::new(TestkitHarness::new(seed).with_core_type(suite::CORE_IS_SEND_NOT_SYNC)),
        None,
        suite::Limits::default(),
    );
}
