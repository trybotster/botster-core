//! A test binary that does not install `PeakAllocator` (`alloc_controls`): the harness does not offer the allocation
//! controls, and a call of either is unsupported, never a pass. The other controls are offered as before. The library's
//! own unit tests and the conformance binary install the allocator, so this binary is the one that shows the other side.

use botster_core_conformance::{ControlError, CoreHarness};
use botster_core_testkit::TestkitHarness;

#[test]
fn without_the_peak_allocator_the_allocation_controls_are_not_offered() {
    let mut harness = TestkitHarness::new(0);
    drop(std::hint::black_box(Vec::<u8>::with_capacity(16)));
    assert!(!harness.has_control("alloc_window"));
    assert!(!harness.has_control("alloc_peak"));
    assert!(harness.has_control("edges_quiet"));
    for op in ["alloc_window", "alloc_peak"] {
        assert!(matches!(
            harness.control("a", op, &serde_json::json!({})),
            Err(ControlError::Unsupported)
        ));
    }
}
