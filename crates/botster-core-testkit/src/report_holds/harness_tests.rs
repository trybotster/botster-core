//! The report holds through the testkit harness's control registry (Core A12-1a, A5-2; steward ruling R-88, group E): the
//! control reaches the link edge of its handle's host, and the host's own driver and the in-process worker run the flows.

use super::*;
use crate::harness::TestkitHarness;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
const PUMPS: usize = 50;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("report-holds-dir".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

fn now(at: Instant) -> Now {
    Now {
        monotonic: at,
        unix: 1_000_000,
    }
}

fn begin(harness: &TestkitHarness, core: &mut dyn CoreApi, op: Value) -> OpId {
    let mut op = op;
    normalize_op(&mut op, &harness.probe_binary());
    core.begin(serde_json::from_value(op).unwrap()).unwrap()
}

/// Pumps until `op` completes and returns its result. When a pump leaves no runnable work and no event, the clock moves to
/// the deadline that Core armed, as the conformance driver's injected clock does (TM-1, TM-3).
fn complete(core: &mut dyn CoreApi, at: &mut Instant, op: OpId) -> OpResult {
    for _ in 0..PUMPS {
        let report = core.pump(now(*at));
        let events = core.poll_events(64);
        let idle = !report.more && events.is_empty();
        for event in events {
            if let Event::Completed { op: o, result } = event {
                if o == op {
                    return result;
                }
            }
        }
        if let Some(due) = core.next_deadline().filter(|due| idle && *due > *at) {
            *at = due;
        }
    }
    panic!("the op {op:?} did not complete");
}

/// Pumps until the pump reports no more work (`pump_idle`), and returns the events. The clock does not move.
fn pump_idle(core: &mut dyn CoreApi, at: Instant) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..PUMPS {
        let report = core.pump(now(at));
        events.extend(core.poll_events(64));
        if !report.more {
            return events;
        }
    }
    panic!("the pump still reports more work after {PUMPS} pumps");
}

fn completes(events: &[Event], op: OpId) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
}

fn held(harness: &mut TestkitHarness) -> Value {
    harness
        .control("a", "held_reports", &json!({"session": "s1"}))
        .unwrap()
}

/// A harness and a Core on handle `a`, with the session `s1` created (its program holds).
fn created() -> (TestkitHarness, Box<dyn CoreApi>, Instant) {
    let mut harness = TestkitHarness::new(0);
    let mut core = harness.open(&spec()).expect("open");
    let mut at = Instant::now();
    let create = begin(
        &harness,
        core.as_mut(),
        json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}),
    );
    assert!(matches!(
        complete(core.as_mut(), &mut at, create),
        OpResult::Ok(_)
    ));
    (harness, core, at)
}

/// `s1` started, stopped, and its payload gone: the worker's next report is the result of a `Remove` (Core LC-7).
fn stopped() -> (TestkitHarness, Box<dyn CoreApi>, Instant) {
    let (harness, mut core, mut at) = created();
    for op in [
        json!({"Start": {"id": "s1"}}),
        json!({"Stop": {"id": "s1"}}),
    ] {
        let op = begin(&harness, core.as_mut(), op);
        assert!(matches!(
            complete(core.as_mut(), &mut at, op),
            OpResult::Ok(_)
        ));
    }
    (harness, core, at)
}

fn uploads(result: &OpResult) -> Option<UploadsOutcome> {
    match result {
        OpResult::Ok(OpOutput::RemoveReport(r)) => Some(r.uploads.clone()),
        _ => None,
    }
}

/// `hold_worker_report` (Core A12-1a): the worker's cleanup result waits at the link edge, so the `Remove` does not complete
/// while it is held (the worker has ended, and its exit waits too). After the release, Core reads it and the `Remove`
/// completes as with no hold: the uploads are deleted.
#[test]
fn a_held_cleanup_result_is_read_after_the_release() {
    let (mut harness, mut core, mut at) = stopped();
    assert_eq!(
        harness.control("a", "hold_worker_report", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let remove = begin(&harness, core.as_mut(), json!({"Remove": {"id": "s1"}}));
    let events = pump_idle(core.as_mut(), at);
    assert!(!completes(&events, remove), "{events:?}");
    assert_eq!(held(&mut harness), json!({"count": 1}));
    assert_eq!(
        harness.control("a", "release_worker_report", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    assert_eq!(held(&mut harness), json!({"count": 0}));
    let result = complete(core.as_mut(), &mut at, remove);
    assert_eq!(
        uploads(&result),
        Some(UploadsOutcome::Deleted),
        "{result:?}"
    );
}

/// `break_control` on a held report (Core A12-1a, A6-3): the link breaks with the report unread, so Core does not know the
/// cleanup's outcome.
#[test]
fn a_break_leaves_the_held_cleanup_result_unread() {
    let (mut harness, mut core, mut at) = stopped();
    harness
        .control("a", "hold_worker_report", &json!({"session": "s1"}))
        .unwrap();
    let remove = begin(&harness, core.as_mut(), json!({"Remove": {"id": "s1"}}));
    let events = pump_idle(core.as_mut(), at);
    assert!(!completes(&events, remove), "{events:?}");
    assert_eq!(held(&mut harness), json!({"count": 1}));
    assert_eq!(
        harness.control("a", "break_control", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let result = complete(core.as_mut(), &mut at, remove);
    assert_eq!(
        uploads(&result),
        Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)),
        "{result:?}"
    );
}

/// `scheduler_delay {point: running, pumps: 3}` (Core A5-2, R-88 (E)): the pump that holds the ready report is pump 1; the
/// report waits through pumps 2 and 3, and the edge gives it out at the start of pump 4, which reads it. Then the `Start`
/// completes `Running`.
#[test]
fn a_delayed_ready_report_is_read_in_pump_pumps_plus_one() {
    let (mut harness, mut core, mut at) = created();
    assert_eq!(
        harness.control(
            "a",
            "scheduler_delay",
            &json!({"session": "s1", "point": "running", "pumps": 3})
        ),
        Ok(Value::Null)
    );
    let start = begin(&harness, core.as_mut(), json!({"Start": {"id": "s1"}}));
    let mut pumps = 0;
    while held(&mut harness) == json!({"count": 0}) {
        pumps += 1;
        assert!(pumps < PUMPS, "the ready report never came");
        core.pump(now(at));
        assert!(!completes(&core.poll_events(64), start));
    }
    for pump in [2, 3] {
        core.pump(now(at));
        assert!(!completes(&core.poll_events(64), start), "pump {pump}");
        assert_eq!(held(&mut harness), json!({"count": 1}), "pump {pump}");
    }
    core.pump(now(at));
    assert_eq!(held(&mut harness), json!({"count": 0}), "pump 4 read it");
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Running);
}

/// One session never has holds of both controls (R-88 (E)): `mixed_holds`. The arguments are checked: `pumps` 0, an unknown
/// argument, a session with no row, and a release with no hold are `Bad`.
#[test]
fn mixed_holds_are_refused_and_bad_arguments_are_bad() {
    let (mut harness, _core, _at) = created();
    let bad = |r: Result<Value, ControlError>| matches!(r, Err(ControlError::Bad(_)));
    assert!(bad(harness.control(
        "a",
        "release_worker_report",
        &json!({"session": "s1"})
    )));
    assert!(bad(harness.control(
        "a",
        "scheduler_delay",
        &json!({"session": "s1", "point": "exited", "pumps": 0})
    )));
    assert!(bad(harness.control(
        "a",
        "held_reports",
        &json!({"session": "s1", "more": 1})
    )));
    assert!(bad(harness.control(
        "a",
        "hold_worker_report",
        &json!({"session": "nobody"})
    )));
    assert!(bad(harness.control(
        "b",
        "held_reports",
        &json!({"session": "s1"})
    )));
    harness
        .control(
            "a",
            "scheduler_delay",
            &json!({"session": "s1", "point": "exited", "pumps": 1}),
        )
        .unwrap();
    assert_eq!(
        harness.control("a", "hold_worker_report", &json!({"session": "s1"})),
        Err(ControlError::Refused(json!({"reason": "mixed_holds"})))
    );
    assert_eq!(held(&mut harness), json!({"count": 0}));
}

/// `break_control` on a running session with an armed hold (Core LC-5, A2-1): the worker's end of the link breaks too, so
/// the next op on the link fails `WorkerLinkFailed`, as with no hold.
#[test]
fn a_break_with_a_hold_still_breaks_a_live_workers_link() {
    let (mut harness, mut core, mut at) = created();
    let start = begin(&harness, core.as_mut(), json!({"Start": {"id": "s1"}}));
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    harness
        .control("a", "hold_worker_report", &json!({"session": "s1"}))
        .unwrap();
    assert_eq!(
        harness.control("a", "break_control", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let read = core
        .begin(Op::ReadScreen {
            session: sid("s1"),
            history: false,
        })
        .unwrap();
    match complete(core.as_mut(), &mut at, read) {
        OpResult::Err(e) => assert_eq!(e.code, ErrorCode::WorkerLinkFailed),
        other => panic!("{other:?}"),
    }
}
