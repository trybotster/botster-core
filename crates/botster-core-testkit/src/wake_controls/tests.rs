use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::json;
use std::time::Instant;

/// The most pumps that a test gives Core to settle: far above what one session needs.
const PUMPS: usize = 50;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("quiet-dir".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn quiet(harness: &mut TestkitHarness) -> bool {
    harness.control("a", "edges_quiet", &json!({})).unwrap()["quiet"] == json!(true)
}

/// Core A5-2, the fence of `await_quiet`: the edges are not quiet while the new worker's hello waits, and they are quiet
/// once Core has consumed every report and has no runnable work. The control never polls: the events stay queued.
#[test]
fn the_edges_are_quiet_only_when_core_has_consumed_every_report() {
    let mut harness = TestkitHarness::new(0);
    let mut core = harness.open(&spec()).expect("open");
    assert!(
        format!("{harness:?}").contains("quiet-dir"),
        "the harness shows the handle's directory"
    );
    assert!(quiet(&mut harness), "no worker, no report");
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    core.begin(serde_json::from_value(create).unwrap()).unwrap();
    core.begin(Op::Start {
        id: SessionId("s1".into()),
    })
    .unwrap();
    let at = Instant::now();
    let now = Now {
        monotonic: at,
        unix: 1_000_000,
    };
    let mut seen_busy = false;
    let mut settled = false;
    for _ in 0..PUMPS {
        let report = core.pump(now);
        let is_quiet = quiet(&mut harness);
        seen_busy |= !is_quiet;
        if !report.more && is_quiet {
            settled = true;
            break;
        }
    }
    assert!(
        seen_busy,
        "the spawned worker's hello was a report that Core had not consumed"
    );
    assert!(settled, "Core consumed every report");
    assert_eq!(
        core.get(&SessionId("s1".into())).unwrap().state,
        SessionState::Running
    );
    assert!(
        !core.poll_events(64).is_empty(),
        "the control polled nothing: the events are still queued"
    );
}

/// `edges_quiet` refuses an unknown handle and an argument that it does not take; `no_spurious_wakes` is known and gives
/// the typed `Unsupported`.
#[test]
fn edges_quiet_refuses_what_it_cannot_read_and_no_spurious_wakes_waits() {
    let mut harness = TestkitHarness::new(0);
    assert!(matches!(
        harness.control("a", "edges_quiet", &json!({})),
        Err(ControlError::Bad(_))
    ));
    let _core = harness.open(&spec()).expect("open");
    assert!(matches!(
        harness.control("a", "edges_quiet", &json!({"session": "s1"})),
        Err(ControlError::Bad(_))
    ));
    assert!(harness.has_control("no_spurious_wakes"));
    assert_eq!(
        harness.control(
            "a",
            "no_spurious_wakes",
            &json!({"op": "no_spurious_wakes"})
        ),
        Err(ControlError::Unsupported)
    );
}
