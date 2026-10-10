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
    assert!(
        after_window < LARGE,
        "the window starts at 0: {after_window}"
    );

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

/// Lead ruling 2026-10-10: the allocator does not recurse into itself. The first allocation of a new thread is the first
/// access to its peak cell, inside the allocator; a cell that allocated on its first access would enter the allocator again
/// and never return. The thread allocates, measures its own peak and ends.
#[test]
fn the_first_allocation_of_a_new_thread_is_measured_without_recursion() {
    let peak = std::thread::spawn(|| {
        drop(black_box(Vec::<u8>::with_capacity(LARGE)));
        PEAK.with(Cell::get)
    })
    .join()
    .unwrap();
    assert!(peak >= LARGE, "{peak}");
}

/// Steward ruling R-52, condition 1 (the guard of the per-thread scope): an allocation of the subject is in the calling
/// thread's peak. A route's client sends one `bytes` frame of `LARGE` payload bytes; the in-process worker's route reader
/// takes the frame on this thread, so the peak is at least the frame. If the testkit ever runs worker code on another
/// thread, this test fails.
#[test]
fn a_frame_that_the_in_process_route_reader_takes_is_in_the_calling_threads_peak() {
    use botster_conformance::Deadline;
    use botster_core_conformance::{normalize_op, DataDirRef, OpenSpec, WorkerBuild, WorkerRef};
    use botster_core_contract::prelude::*;
    use botster_route_codec::prelude::{stream_wrap, Op as WireOp, ToWorker};
    use std::time::Instant;

    let mut harness = TestkitHarness::new(0);
    let spec = OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    };
    let mut core = harness.open(&spec).expect("open");
    let at = Instant::now();
    let now = Now {
        monotonic: at,
        unix: 1_000_000,
    };
    let mut run = |core: &mut Box<dyn CoreApi>, op: OpId| {
        for _ in 0..50 {
            core.pump(now);
            for event in core.poll_events(64) {
                if let Event::Completed { op: o, result } = event {
                    if o == op {
                        assert!(matches!(result, OpResult::Ok(_)), "{result:?}");
                        return;
                    }
                }
            }
        }
        panic!("the op {op:?} did not complete");
    };
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    run(&mut core, create);
    let start = core
        .begin(Op::Start {
            id: SessionId("s1".into()),
        })
        .unwrap();
    run(&mut core, start);
    let options: AttachOptions = serde_json::from_value(
        json!({"file_directory": "/tmp", "answers_queries": false, "input": true}),
    )
    .unwrap();
    let (_, mut route) = harness
        .attach_stream(
            "a",
            core.as_mut(),
            ClientId("c".into()),
            &SessionId("s1".into()),
            options,
        )
        .expect("attached");
    // The baseline first, so that the window holds only the frame.
    while !matches!(
        route.read(1 << 20, &Deadline::none()),
        botster_hub_conformance::route::RouteRead::Empty
    ) {}
    window(&mut harness);
    route.write(&stream_wrap(
        &ToWorker::Bytes {
            op: WireOp(1),
            bytes: vec![b'x'; LARGE].into(),
        }
        .encode(),
    ));
    let window_peak = peak_bytes(&mut harness);
    assert!(
        window_peak >= LARGE,
        "the client's own frame: {window_peak}"
    );
    // A new window after the client's write. The client's end now only moves its buffered bytes into a stream of
    // ROUTE_STREAM_BYTES, which allocates less than LARGE, so an allocation of LARGE is the worker's.
    assert!(crate::route_client::ROUTE_STREAM_BYTES < LARGE);
    window(&mut harness);
    for _ in 0..8 {
        route.read(1 << 20, &Deadline::none());
    }
    let peak = peak_bytes(&mut harness);
    assert!(peak >= LARGE, "the route reader's frame: {peak}");
}
