use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::json;
use std::time::{Duration, Instant};

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
const PUMPS: usize = 50;

fn spec(handle: &str) -> OpenSpec {
    OpenSpec {
        handle: handle.into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

/// The op of a transcript, with its program turned into the probe's argv as the driver turns it (design 6.2).
fn op(harness: &TestkitHarness, mut op: Value) -> Op {
    normalize_op(&mut op, &harness.probe_binary());
    serde_json::from_value(op).expect("an op")
}

/// Pumps until `op` completes, and returns its result. The test is the host (TM-1, TM-3): when a pump leaves no runnable
/// work and no event, it moves its clock to the deadline that Core armed, as the conformance driver's injected clock does.
fn complete(core: &mut dyn CoreApi, at: &mut Instant, op: OpId) -> OpResult {
    for _ in 0..PUMPS {
        let report = core.pump(Now {
            monotonic: *at,
            unix: 1_000_000,
        });
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

/// A Core on handle `a` with the session `s1` created; `start` also starts it.
fn session(harness: &mut TestkitHarness, start: bool) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec("a")).expect("open");
    let mut at = Instant::now();
    let create = op(
        harness,
        json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}}),
    );
    let create = core.begin(create).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, create),
        OpResult::Ok(_)
    ));
    if start {
        let id = core.begin(Op::Start { id: sid("s1") }).unwrap();
        assert!(matches!(
            complete(core.as_mut(), &mut at, id),
            OpResult::Ok(_)
        ));
    }
    (core, at)
}

fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

fn read_screen(core: &mut dyn CoreApi, at: &mut Instant) -> OpResult {
    let read = core
        .begin(Op::ReadScreen {
            session: sid("s1"),
            history: false,
        })
        .unwrap();
    complete(core, at, read)
}

/// Core LC-5, A2-1: after `break_control`, an op on the session's link fails `WorkerLinkFailed`, and a stop still ends the
/// session. (Without the break, the op goes to the worker on the link; the in-process worker answers `ReadScreen` with M2.)
#[test]
fn a_broken_control_link_fails_the_next_op_and_a_stop_still_ends_the_session() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    assert_eq!(
        harness.control(
            "a",
            "break_control",
            &json!({"op": "break_control", "session": "s1"})
        ),
        Ok(Value::Null)
    );
    match read_screen(core.as_mut(), &mut at) {
        OpResult::Err(e) => assert_eq!(e.code, ErrorCode::WorkerLinkFailed),
        other => panic!("{other:?}"),
    }
    let stop = core.begin(Op::Stop { id: sid("s1") }).unwrap();
    match complete(core.as_mut(), &mut at, stop) {
        // The stop completes with the session's end (LC-5): `Exited` or `Lost`, as the transcript allows.
        OpResult::Ok(OpOutput::End(_)) => {}
        other => panic!("{other:?}"),
    }
}

/// Core TM-6: the break is an edge event, so it wakes the host that owns the worker, as the end of file of a real socket
/// does. The host is idle before it: a pump that leaves no work clears the wake.
#[test]
fn a_break_wakes_the_host_that_owns_the_worker() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    let mut settled = false;
    for _ in 0..PUMPS {
        let report = core.pump(Now {
            monotonic: at,
            unix: 1_000_000,
        });
        core.poll_events(64);
        if !report.more {
            settled = true;
            break;
        }
    }
    assert!(settled, "the host has no runnable work left");
    let wake = core.wake_handle();
    assert_eq!(
        wake.wait(Duration::ZERO),
        Wake::TimedOut,
        "the host is idle"
    );
    assert_eq!(
        harness.control("a", "break_control", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
}

/// `break_control` refuses what it cannot do, with `Bad`: an unknown handle, an unknown session, a session with no worker
/// process yet, `on: false`, and an argument that it does not take.
#[test]
fn break_control_refuses_what_it_cannot_break() {
    let mut harness = TestkitHarness::new(0);
    let bad = |result: Result<Value, ControlError>| matches!(result, Err(ControlError::Bad(_)));
    assert!(bad(harness.control(
        "a",
        "break_control",
        &json!({"session": "s1"})
    )));
    let (_core, _at) = session(&mut harness, false);
    assert!(bad(harness.control(
        "a",
        "break_control",
        &json!({"session": "s1"})
    )));
    assert!(bad(harness.control(
        "a",
        "break_control",
        &json!({"session": "s9"})
    )));
    assert!(bad(harness.control(
        "b",
        "break_control",
        &json!({"session": "s1"})
    )));
    assert!(bad(harness.control(
        "a",
        "break_control",
        &json!({"session": "s1", "on": false})
    )));
    assert!(bad(harness.control(
        "a",
        "break_control",
        &json!({"session": "s1", "seconds": 1})
    )));
}

/// A control that the testkit cannot build yet is known, and gives the typed `Unsupported`, never an unknown-control error.
#[test]
fn the_controls_that_wait_are_registered_as_unsupported() {
    let mut harness = TestkitHarness::new(0);
    for (name, _) in UNSUPPORTED {
        assert!(harness.has_control(name), "{name}");
        assert_eq!(
            harness.control("a", name, &json!({"op": name, "session": "s1"})),
            Err(ControlError::Unsupported),
            "{name}"
        );
    }
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nested {
    session: SessionId,
    script: Value,
}

/// `parse` removes the step's own keys, `op` and `handle`, at the top level only: a control's arguments come out as they are
/// with or without them, a key of an argument's value is kept, and an unknown argument is `Bad`.
#[test]
fn parse_removes_only_the_step_keys_at_the_top_level() {
    let alone = json!({"session": "s1", "script": {"op": "x", "handle": "y"}});
    let with_step_keys =
        json!({"op": "c", "handle": "a", "session": "s1", "script": {"op": "x", "handle": "y"}});
    let expected = Nested {
        session: sid("s1"),
        script: json!({"op": "x", "handle": "y"}),
    };
    assert_eq!(parse::<Nested>(&alone).unwrap(), expected);
    assert_eq!(parse::<Nested>(&with_step_keys).unwrap(), expected);
    let unknown = json!({"session": "s1", "script": 1, "extra": 1});
    assert!(matches!(
        parse::<Nested>(&unknown),
        Err(ControlError::Bad(_))
    ));
    let args = json!({"op": "break_control", "handle": "a", "session": "s1"});
    let args: BreakControl = parse(&args).unwrap();
    assert_eq!(args.session, sid("s1"));
    assert!(args.on);
}
