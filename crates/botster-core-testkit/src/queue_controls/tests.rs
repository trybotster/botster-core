use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what this session needs.
const PUMPS: usize = 50;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn now(at: Instant) -> Now {
    Now {
        monotonic: at,
        unix: 1_000_000,
    }
}

fn sid() -> SessionId {
    SessionId("s1".into())
}

/// Pumps until `op` completes.
fn complete(core: &mut dyn CoreApi, at: Instant, op: OpId) {
    for _ in 0..PUMPS {
        core.pump(now(at));
        if core
            .poll_events(64)
            .iter()
            .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
        {
            return;
        }
    }
    panic!("the op {op:?} did not complete");
}

/// A Core on handle `a` with the session `s1` running a payload that holds.
fn running(harness: &mut TestkitHarness) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec()).expect("open");
    let at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    complete(core.as_mut(), at, create);
    let start = core.begin(Op::Start { id: sid() }).unwrap();
    complete(core.as_mut(), at, start);
    (core, at)
}

fn write(core: &mut dyn CoreApi) -> OpId {
    core.begin(Op::WriteInput {
        session: sid(),
        payload: InputPayload::Text { text: "a".into() },
        guard: None,
    })
    .unwrap()
}

/// Core IN-6, R-91: `control_queue {session}` (on by default) leaves the session's control link no room at the testkit's
/// link edge, so Core refuses a `cancel` `Full`; `on: false` gives the room back, the cancel is admitted, and the next pump
/// posts the `SessionWritable` that the refusal owes.
#[test]
fn control_queue_takes_the_links_room_and_gives_it_back() {
    let mut harness = TestkitHarness::new(0);
    assert!(harness.has_control("control_queue"));
    let (mut core, at) = running(&mut harness);
    assert_eq!(
        harness
            .control("a", "control_queue", &json!({"session": "s1"}))
            .unwrap(),
        json!({})
    );
    let op = write(core.as_mut());
    assert_eq!(core.cancel(op), CancelResult::Refused(CancelRefusal::Full));
    harness
        .control("a", "control_queue", &json!({"session": "s1", "on": false}))
        .unwrap();
    assert_eq!(core.cancel(op), CancelResult::Admitted, "the room is back");
    core.pump(now(at));
    let events = core.poll_events(64);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::SessionWritable { id, .. } if *id == sid())),
        "{events:?}"
    );
}
