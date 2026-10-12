//! `scheduler_hold` and `scheduler_release` through the testkit harness's control registry (Core A5-2; steward ruling R-88,
//! group E): the control reaches the scheduler of its handle's host, and the host's `TestkitCore` records what Core admits.

use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
const PUMPS: usize = 50;

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("holds-dir".into()),
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

/// Pumps until the pump reports no more work (`pump_idle`), and returns the events.
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

/// A held `Stop` makes no progress until its release; then the pumps run it to its end. (The seeded wake may be spurious
/// (TH-2), so the release's signal is proved by the unit test of `release` and on the real tier.)
#[test]
fn a_held_stop_waits_for_its_release() {
    let (mut harness, mut core, mut at) = created();
    let start = begin(&harness, core.as_mut(), json!({"Start": {"id": "s1"}}));
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    let stop = begin(&harness, core.as_mut(), json!({"Stop": {"id": "s1"}}));
    assert_eq!(
        harness.control("a", "scheduler_hold", &json!({"for_op": stop.0})),
        Ok(Value::Null)
    );
    let events = pump_idle(core.as_mut(), at);
    assert_eq!(events, [], "a held stop posts nothing");
    assert_eq!(core.get(&sid("s1")).unwrap().state, SessionState::Running);
    assert_eq!(
        harness.control("a", "scheduler_release", &json!({"for_op": stop.0})),
        Ok(Value::Null)
    );
    assert!(matches!(
        complete(core.as_mut(), &mut at, stop),
        OpResult::Ok(_)
    ));
}

/// R-20: a `Resize` of a `Created` session has fixed timing, so its hold is refused; a hold of an op that Core did not admit,
/// and a control on a handle that is not open, are `Bad`.
#[test]
fn a_hold_of_a_fixed_timing_op_or_an_unknown_op_is_refused() {
    let (mut harness, mut core, _) = created();
    let resize = begin(
        &harness,
        core.as_mut(),
        json!({"Resize": {"session": "s1", "size": {"rows": 30, "cols": 90}}}),
    );
    assert_eq!(
        harness.control("a", "scheduler_hold", &json!({"for_op": resize.0})),
        Err(ControlError::Refused(json!({"reason": "fixed_timing"})))
    );
    assert!(matches!(
        harness.control("a", "scheduler_hold", &json!({"for_op": resize.0 + 1})),
        Err(ControlError::Bad(_))
    ));
    assert!(matches!(
        harness.control("b", "scheduler_hold", &json!({"for_op": resize.0})),
        Err(ControlError::Bad(_))
    ));
}
