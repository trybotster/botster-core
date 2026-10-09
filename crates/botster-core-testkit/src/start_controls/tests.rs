use super::*;
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
        data_dir: DataDirRef("start-dir".into()),
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

/// Pumps until `op` completes, and returns its result. When a pump leaves no runnable work and no event, the test moves its
/// clock to the deadline that Core armed, as the conformance driver's injected clock does (TM-1, TM-3).
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

/// Pumps at `at` until Core has no runnable work and the edges are quiet, as `await_quiet` does: the clock stays. Returns
/// the events, so a test can check that an op did not complete.
fn settle(harness: &mut TestkitHarness, core: &mut dyn CoreApi, at: Instant) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..PUMPS {
        let report = core.pump(now(at));
        events.extend(core.poll_events(64));
        let quiet = harness.control("a", "edges_quiet", &json!({})).unwrap();
        if !report.more && quiet["quiet"] == json!(true) {
            return events;
        }
    }
    panic!("Core did not settle");
}

/// A harness and a Core on handle `a` with the session `s1` created; its program holds.
fn created(seed: u64) -> (TestkitHarness, Box<dyn CoreApi>, Instant) {
    let mut harness = TestkitHarness::new(seed);
    let mut core = harness.open(&spec()).expect("open");
    let mut at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, create),
        OpResult::Ok(_)
    ));
    (harness, core, at)
}

fn control(harness: &mut TestkitHarness, op: &str, args: Value) -> Result<Value, ControlError> {
    harness.control("a", op, &args)
}

fn alive(harness: &mut TestkitHarness) -> Value {
    control(harness, "payload_alive", json!({"session": "s1"})).unwrap()["alive"].clone()
}

fn hold(harness: &mut TestkitHarness) {
    assert_eq!(
        control(
            harness,
            "hold_start_at",
            json!({"op": "hold_start_at", "session": "s1", "before": "payload"})
        ),
        Ok(Value::Null)
    );
}

fn completes(events: &[Event], op: OpId) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
}

/// Core AD-7: a start held before the payload's launch stops after step 3. The row is `Starting` with the worker identity
/// recorded, no payload runs, and `Start` does not complete. After the release, the payload runs and `Start` completes
/// `Running`. `registry_row` follows the stored row through `Created`, `Starting` and `Running`.
#[test]
fn a_held_start_launches_its_payload_only_after_the_release() {
    let (mut harness, mut core, mut at) = created(0);
    let row = |harness: &mut TestkitHarness| {
        control(harness, "registry_row", json!({"session": "s1"})).unwrap()
    };
    assert_eq!(
        row(&mut harness),
        json!({"state": "Created", "identity_recorded": false, "labels": {}})
    );
    assert_eq!(alive(&mut harness), json!(false), "no worker, no payload");
    hold(&mut harness);
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    let events = settle(&mut harness, core.as_mut(), at);
    assert!(
        !completes(&events, start),
        "the held start does not complete"
    );
    assert_eq!(
        row(&mut harness),
        json!({"state": "Starting", "identity_recorded": true, "labels": {}})
    );
    assert_eq!(
        alive(&mut harness),
        json!(false),
        "no payload runs while held"
    );
    assert_eq!(
        control(&mut harness, "release_start_at", json!({"session": "s1"})),
        Ok(Value::Null)
    );
    match complete(core.as_mut(), &mut at, start) {
        OpResult::Ok(OpOutput::Record(record)) => assert_eq!(record.state, SessionState::Running),
        other => panic!("{other:?}"),
    }
    assert_eq!(alive(&mut harness), json!(true));
    assert_eq!(row(&mut harness)["state"], json!("Running"));
}

/// A stop of a held start ends the session, and the kept spawn goes with the worker: no payload runs later, and no hold
/// remains to release.
#[test]
fn a_stop_of_a_held_start_drops_the_kept_spawn() {
    let (mut harness, mut core, mut at) = created(0);
    hold(&mut harness);
    let _start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    settle(&mut harness, core.as_mut(), at);
    let stop = core.begin(Op::Stop { id: sid("s1") }).unwrap();
    complete(core.as_mut(), &mut at, stop);
    let state = core.get(&sid("s1")).unwrap().state;
    assert!(
        matches!(state, SessionState::Exited(_) | SessionState::Lost(_)),
        "{state:?}"
    );
    assert_eq!(alive(&mut harness), json!(false));
    assert!(matches!(
        control(&mut harness, "release_start_at", json!({"session": "s1"})),
        Err(ControlError::Bad(_))
    ));
    settle(&mut harness, core.as_mut(), at);
    assert_eq!(alive(&mut harness), json!(false), "no payload spawns later");
}

/// A hold that is left at the end of a run goes with the run: a new run of the same seed starts the session at once.
#[test]
fn a_hold_left_at_the_end_of_a_run_does_not_reach_the_next_run() {
    {
        let (mut harness, _core, _at) = created(3);
        hold(&mut harness);
    }
    let (mut harness, mut core, mut at) = created(3);
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    assert_eq!(alive(&mut harness), json!(true));
}

/// The start controls refuse what they cannot do, with `Bad`: an unknown handle or session, an argument that they do not
/// take, a second hold, a hold of a start that has begun, and a release with no hold. `before` `identity` and `running` are
/// known and give the typed `Unsupported`.
#[test]
fn the_start_controls_refuse_what_they_cannot_do() {
    let bad = |result: Result<Value, ControlError>| matches!(result, Err(ControlError::Bad(_)));
    let mut harness = TestkitHarness::new(0);
    for op in ["registry_row", "payload_alive", "release_start_at"] {
        assert!(
            bad(control(&mut harness, op, json!({"session": "s1"}))),
            "{op}"
        );
    }
    assert!(bad(control(
        &mut harness,
        "hold_start_at",
        json!({"session": "s1", "before": "payload"})
    )));
    let (mut harness, mut core, mut at) = created(0);
    for op in ["registry_row", "payload_alive", "release_start_at"] {
        assert!(
            bad(control(&mut harness, op, json!({"session": "s2"}))),
            "{op}"
        );
        assert!(
            bad(control(&mut harness, op, json!({"session": "s1", "at": 1}))),
            "{op}"
        );
    }
    assert!(bad(control(
        &mut harness,
        "hold_start_at",
        json!({"session": "s2", "before": "payload"})
    )));
    for before in [json!("payload"), json!("later"), json!(1)] {
        let args = json!({"session": "s1", "before": before, "at": 1});
        assert!(bad(control(&mut harness, "hold_start_at", args)));
    }
    assert!(bad(control(
        &mut harness,
        "hold_start_at",
        json!({"session": "s1", "before": "later"})
    )));
    for before in ["identity", "running"] {
        assert_eq!(
            control(
                &mut harness,
                "hold_start_at",
                json!({"session": "s1", "before": before})
            ),
            Err(ControlError::Unsupported),
            "{before}"
        );
    }
    assert!(bad(control(
        &mut harness,
        "release_start_at",
        json!({"session": "s1"})
    )));
    hold(&mut harness);
    assert!(bad(control(
        &mut harness,
        "hold_start_at",
        json!({"session": "s1", "before": "payload"})
    )));
    assert_eq!(
        control(&mut harness, "release_start_at", json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    assert!(bad(control(
        &mut harness,
        "hold_start_at",
        json!({"session": "s1", "before": "payload"})
    )));
}
