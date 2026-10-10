use super::*;
use botster_core_conformance::CoreHarness;
use std::hint::black_box;

/// Far above every allocation that the controls make themselves.
const LARGE: usize = 1 << 20;

fn peak_bytes(harness: &mut TestkitHarness) -> usize {
    let value = harness.control("a", "alloc_peak", &json!({})).unwrap();
    usize::try_from(value["bytes"].as_u64().unwrap()).unwrap()
}

fn window(harness: &mut TestkitHarness) {
    assert_eq!(
        harness.control("a", "alloc_window", &json!({})).unwrap(),
        Value::Null
    );
}

#[test]
fn the_unit_test_binary_installs_the_allocator_and_the_harness_offers_both_controls() {
    drop(black_box(Vec::<u8>::with_capacity(16)));
    assert!(installed());
    let harness = TestkitHarness::new(0);
    assert!(harness.has_control("alloc_window"));
    assert!(harness.has_control("alloc_peak"));
}

#[test]
fn the_peak_is_the_largest_single_allocation_since_the_window() {
    let mut harness = TestkitHarness::new(0);
    drop(black_box(Vec::<u8>::with_capacity(4 * LARGE)));
    window(&mut harness);
    let after_window = peak_bytes(&mut harness);
    assert!(after_window < LARGE, "the window starts at 0: {after_window}");

    drop(black_box(Vec::<u8>::with_capacity(LARGE)));
    drop(black_box(Vec::<u8>::with_capacity(16)));
    let peak = peak_bytes(&mut harness);
    assert!(
        (LARGE..2 * LARGE).contains(&peak),
        "the largest allocation, not the last one or a sum: {peak}"
    );
}

#[test]
fn a_zeroed_allocation_and_the_new_size_of_a_realloc_are_measured() {
    let mut harness = TestkitHarness::new(0);
    window(&mut harness);
    drop(black_box(vec![0u8; LARGE]));
    assert!(peak_bytes(&mut harness) >= LARGE, "alloc_zeroed");

    let mut grown = black_box(Vec::<u8>::with_capacity(16));
    window(&mut harness);
    grown.reserve_exact(2 * LARGE);
    drop(black_box(grown));
    assert!(peak_bytes(&mut harness) >= 2 * LARGE, "realloc's new size");
}

#[test]
fn an_allocation_on_another_thread_is_not_in_this_threads_peak() {
    let mut harness = TestkitHarness::new(0);
    window(&mut harness);
    std::thread::spawn(|| drop(black_box(Vec::<u8>::with_capacity(4 * LARGE))))
        .join()
        .unwrap();
    let peak = peak_bytes(&mut harness);
    assert!(peak < 4 * LARGE, "another trial's allocation: {peak}");
}

#[test]
fn without_the_allocator_both_controls_are_unsupported() {
    assert!(matches!(window_of(false), Err(ControlError::Unsupported)));
    assert!(matches!(peak(false), Err(ControlError::Unsupported)));
    assert!(!supported("alloc_window", false));
    assert!(!supported("alloc_peak", false));
    assert!(supported("edges_quiet", false));
    assert!(supported("alloc_window", true));
    assert!(supported("alloc_peak", true));
}

fn window_of(installed: bool) -> Result<Value, ControlError> {
    super::window(installed)
}

#[test]
fn an_unknown_argument_is_bad() {
    let mut harness = TestkitHarness::new(0);
    for op in ["alloc_window", "alloc_peak"] {
        assert!(matches!(
            harness.control("a", op, &json!({"bytes": 1})),
            Err(ControlError::Bad(_))
        ));
    }
}
