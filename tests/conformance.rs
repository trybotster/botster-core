//! The Core conformance suite on `TestkitHarness` (plan section 5, the default tier), one trial per Core id of the pinned
//! ledger: see `suite/mod.rs`. `harness = false`.

mod suite;

use botster_core_testkit::{PeakAllocator, TestkitHarness};

/// `alloc_window` and `alloc_peak` measure through it (Core DP-3, steward ruling R-37; `botster_core_testkit::alloc_controls`).
#[global_allocator]
static ALLOCATOR: PeakAllocator = PeakAllocator;

fn main() {
    // A fake: no step timeout (design 6.1).
    suite::run(
        "conformance",
        |seed| Box::new(TestkitHarness::new(seed).with_core_type(suite::CORE_IS_SEND_NOT_SYNC)),
        None,
        suite::Limits::default(),
    );
}
