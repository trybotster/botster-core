use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
const PUMPS: usize = 50;

fn spec(handle: &str) -> OpenSpec {
    OpenSpec {
        handle: handle.into(),
        data_dir: DataDirRef("adopt-dir".into()),
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

/// Pumps until `op` completes, and returns its result and the events up to it. When a pump leaves no runnable work and no
/// event, the clock moves to the deadline that Core armed, as the conformance driver's injected clock does (TM-1, TM-3).
fn complete(core: &mut dyn CoreApi, at: &mut Instant, op: OpId) -> (OpResult, Vec<Event>) {
    let mut seen = Vec::new();
    for _ in 0..PUMPS {
        let report = core.pump(now(*at));
        let events = core.poll_events(64);
        let idle = !report.more && events.is_empty();
        for event in events {
            if let Event::Completed { op: o, result } = &event {
                if *o == op {
                    let result = result.clone();
                    seen.push(event);
                    return (result, seen);
                }
            }
            seen.push(event);
        }
        if let Some(due) = core.next_deadline().filter(|due| idle && *due > *at) {
            *at = due;
        }
    }
    panic!("the op {op:?} did not complete");
}

/// Begins `op` and completes it.
fn run(core: &mut dyn CoreApi, at: &mut Instant, op: Value) -> (OpResult, Vec<Event>) {
    let mut op = op;
    normalize_op(&mut op, "probe");
    let id = core.begin(serde_json::from_value(op).unwrap()).unwrap();
    complete(core, at, id)
}

/// A harness and a Core on handle `a` with the sessions `names` running; their programs hold.
fn running(names: &[&str]) -> (TestkitHarness, Box<dyn CoreApi>, Instant) {
    let mut harness = TestkitHarness::new(0);
    let mut core = harness.open(&spec("a")).expect("open");
    let mut at = Instant::now();
    for name in names {
        let mut create =
            json!({"Create": {"session": name, "request": {"program": [{"hold": {}}]}}});
        normalize_op(&mut create, &harness.probe_binary());
        let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
        assert!(matches!(
            complete(core.as_mut(), &mut at, create).0,
            OpResult::Ok(_)
        ));
        let (start, _) = run(core.as_mut(), &mut at, json!({"Start": {"id": name}}));
        assert!(matches!(start, OpResult::Ok(_)), "{start:?}");
    }
    (harness, core, at)
}

/// Drops handle `a` and adopts on a new handle `b` over the same directory.
fn adopt_on_b(
    harness: &mut TestkitHarness,
    core: Box<dyn CoreApi>,
    at: &mut Instant,
) -> (Box<dyn CoreApi>, Vec<Event>) {
    drop(core);
    harness.drop_handle("a");
    let mut b = harness.open(&spec("b")).expect("open b");
    let (result, events) = run(b.as_mut(), at, json!("AdoptAll"));
    assert!(matches!(result, OpResult::Ok(_)), "{result:?}");
    (b, events)
}

fn state(core: &dyn CoreApi, name: &str) -> SessionState {
    core.get(&sid(name)).unwrap().state
}

/// Core A10-1, AD-6: an impostor answers the adoption with a wrong token, or a wrong `InstanceId`. Core's own check refuses
/// it: the session is `Lost(WorkerGone)`, and Core sends no signal to the refused identity. A later handle posts the row,
/// and its `Remove` sends none either (review A2-F1): the upload outcome is unknown. The real worker is not touched: its
/// payload still runs.
#[test]
fn an_impostor_is_refused_and_never_signalled() {
    for field in ["token", "instance"] {
        let (mut harness, core, mut at) = running(&["s1"]);
        assert_eq!(
            harness.control(
                "a",
                "impostor_worker",
                &json!({"session": "s1", "field": field})
            ),
            Ok(Value::Null)
        );
        let (b, _) = adopt_on_b(&mut harness, core, &mut at);
        assert_eq!(
            state(b.as_ref(), "s1"),
            SessionState::Lost(LostReason::WorkerGone),
            "{field}"
        );
        let signals = || json!({"signals": []});
        assert_eq!(
            harness.control("b", "signals_received", &json!({"session": "s1"})),
            Ok(signals())
        );
        assert_eq!(
            harness.control("b", "payload_alive", &json!({"session": "s1"})),
            Ok(json!({"alive": true})),
            "the real worker runs on"
        );
        drop(b);
        harness.drop_handle("b");
        let mut c = harness.open(&spec("c")).expect("open c");
        let (adopted, _) = run(c.as_mut(), &mut at, json!("AdoptAll"));
        assert!(matches!(adopted, OpResult::Ok(_)), "{adopted:?}");
        assert_eq!(
            state(c.as_ref(), "s1"),
            SessionState::Lost(LostReason::WorkerGone)
        );
        assert_eq!(
            harness.control("c", "payload_alive", &json!({"session": "s1"})),
            Ok(json!({"alive": true})),
            "the real worker runs on at the later handle"
        );
        let (removed, _) = run(c.as_mut(), &mut at, json!({"Remove": {"id": "s1"}}));
        assert!(
            matches!(
                &removed,
                OpResult::Ok(OpOutput::RemoveReport(r))
                    if r.uploads == UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
            ),
            "{removed:?}"
        );
        // `payload_alive` reads the row, which the `Remove` deletes: the signal count is the proof after it.
        assert_eq!(
            harness.control("c", "signals_received", &json!({"session": "s1"})),
            Ok(signals())
        );
    }
}

/// Core A11-1, steward ruling R-42: the impostor's scripted frames, which Core meets after it refused the handshake, change
/// nothing. The state stays
/// `Lost(WorkerGone)`, and no notification or state event of the session follows.
#[test]
fn an_impostors_frames_after_the_refusal_change_nothing() {
    let (mut harness, core, mut at) = running(&["s1", "sb"]);
    assert_eq!(
        harness.control(
            "a",
            "impostor_worker",
            &json!({"session": "s1", "field": "token",
                    "script": ["state_exited", "notification", "cleanup_deleted"]})
        ),
        Ok(Value::Null)
    );
    let (mut b, mut events) = adopt_on_b(&mut harness, core, &mut at);
    // A later operation of another session lets the impostor's frames arrive if any could.
    let (labels, later) = run(
        b.as_mut(),
        &mut at,
        json!({"UpdateMetadata": {"id": "sb", "labels": {"b": "1"}}}),
    );
    assert!(matches!(labels, OpResult::Ok(_)), "{labels:?}");
    events.extend(later);
    let states: Vec<&SessionState> = events
        .iter()
        .filter_map(|e| match e {
            Event::SessionState { id, state, .. } if *id == sid("s1") => Some(state),
            _ => None,
        })
        .collect();
    assert_eq!(states, [&SessionState::Lost(LostReason::WorkerGone)]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Notification { id, .. } if *id == sid("s1"))),
        "{events:?}"
    );
    assert_eq!(
        state(b.as_ref(), "s1"),
        SessionState::Lost(LostReason::WorkerGone)
    );
}

/// Core A6-1, AD-2: a live worker that withholds its control link is `Lost(WorkerUnreachable)` at Core's deadline, not
/// `WorkerGone`. The worker and its payload run on.
#[test]
fn a_withheld_control_link_is_unreachable_at_the_deadline() {
    let (mut harness, core, mut at) = running(&["s1"]);
    assert_eq!(
        harness.control("a", "withhold_control_link", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let started = at;
    let (b, _) = adopt_on_b(&mut harness, core, &mut at);
    assert!(at > started, "the adoption ended at Core's deadline");
    assert_eq!(
        state(b.as_ref(), "s1"),
        SessionState::Lost(LostReason::WorkerUnreachable)
    );
    assert_eq!(
        harness.control("b", "payload_alive", &json!({"session": "s1"})),
        Ok(json!({"alive": true}))
    );
}

/// Core A10-2, AD-2: a damaged row is `Lost(RegistryCorrupt)` at the adoption, and the other session is adopted.
#[test]
fn a_damaged_row_is_registry_corrupt_and_the_others_are_adopted() {
    let (mut harness, core, mut at) = running(&["s1", "s2"]);
    assert_eq!(
        harness.control("a", "corrupt_registry_row", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let (b, _) = adopt_on_b(&mut harness, core, &mut at);
    assert_eq!(
        state(b.as_ref(), "s1"),
        SessionState::Lost(LostReason::RegistryCorrupt)
    );
    assert_eq!(state(b.as_ref(), "s2"), SessionState::Running);
}

/// Core A10-2: `corrupt_registry_row` keeps the first half of the bytes that Core's encoder wrote.
#[test]
fn a_damaged_row_is_the_first_half_of_the_written_row() {
    let (mut harness, _core, _at) = running(&["s1"]);
    let key = row_key(&sid("s1"));
    let dir = harness.directory_of("a").expect("handle a").to_string();
    let written = harness.directories().row(&dir, &key).expect("the row");
    assert!(written.len() > 2);
    assert_eq!(
        harness.control("a", "corrupt_registry_row", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    assert_eq!(
        harness.directories().row(&dir, &key),
        Some(written[..written.len() / 2].to_vec())
    );
}

/// Each control refuses what it cannot do, with `Bad`.
#[test]
fn the_adoption_controls_refuse_what_they_cannot_do() {
    let (mut harness, _core, _at) = running(&["s1"]);
    let bad = |result: Result<Value, ControlError>| matches!(result, Err(ControlError::Bad(_)));
    let mut control = |op: &str, args: Value| harness.control("a", op, &args);
    assert!(bad(control(
        "impostor_worker",
        json!({"session": "s1", "field": "epoch"})
    )));
    assert!(bad(control(
        "impostor_worker",
        json!({"session": "s1", "field": "token", "script": ["launch"]})
    )));
    assert!(bad(control(
        "impostor_worker",
        json!({"session": "nope", "field": "token"})
    )));
    assert!(bad(control("signals_received", json!({"session": "s1"}))));
    assert_eq!(
        control(
            "impostor_worker",
            json!({"session": "s1", "field": "token"})
        ),
        Ok(Value::Null)
    );
    assert!(bad(control(
        "impostor_worker",
        json!({"session": "s1", "field": "instance"})
    )));
    assert_eq!(
        control("withhold_control_link", json!({"session": "s1"})),
        Ok(Value::Null)
    );
    assert!(bad(control(
        "withhold_control_link",
        json!({"session": "s1"})
    )));
    assert!(bad(control(
        "withhold_control_link",
        json!({"session": "s1", "extra": 1})
    )));
    assert!(bad(control(
        "corrupt_registry_row",
        json!({"session": "nope"})
    )));
}

/// Core A6-2, AD-4: a hello that proves the token and the instance but announces a protocol outside `{T, T-1}` is
/// `Lost(WorkerVersion)`. Core sends no signal to the identity, and the real worker runs on.
#[test]
fn an_announced_protocol_outside_the_set_is_worker_version() {
    let current = botster_worker_core::WORKER_PROTOCOL;
    for protocol in [current + 1, current + 2] {
        let (mut harness, core, mut at) = running(&["s1", "s2"]);
        assert_eq!(
            harness.control(
                "a",
                "announce_protocol",
                &json!({"session": "s1", "protocol": protocol})
            ),
            Ok(Value::Null)
        );
        let (b, _) = adopt_on_b(&mut harness, core, &mut at);
        assert_eq!(
            state(b.as_ref(), "s1"),
            SessionState::Lost(LostReason::WorkerVersion),
            "{protocol}"
        );
        assert_eq!(state(b.as_ref(), "s2"), SessionState::Running);
        assert_eq!(
            harness.control("b", "signals_received", &json!({"session": "s1"})),
            Ok(json!({"signals": []}))
        );
        assert_eq!(
            harness.control("b", "payload_alive", &json!({"session": "s1"})),
            Ok(json!({"alive": true}))
        );
    }
}

/// `announce_protocol` gives `Unsupported` for a protocol that Core adopts (`T`, and `T-1` when `T > 1`): only a real worker
/// of that protocol can stand for it. Protocol 0 is never adoptable, so the control takes it. It refuses an unknown session,
/// a protocol that is not a `u8` and an unknown argument with `Bad`.
#[test]
fn announce_protocol_refuses_an_adoptable_protocol() {
    let (mut harness, _core, _at) = running(&["s1"]);
    let current = botster_worker_core::WORKER_PROTOCOL;
    let mut control = |args: Value| harness.control("a", "announce_protocol", &args);
    assert_eq!(
        control(json!({"session": "s1", "protocol": current})),
        Err(ControlError::Unsupported)
    );
    if current > 1 {
        assert_eq!(
            control(json!({"session": "s1", "protocol": current - 1})),
            Err(ControlError::Unsupported)
        );
    }
    let bad = |result: Result<Value, ControlError>| matches!(result, Err(ControlError::Bad(_)));
    assert!(bad(control(
        json!({"session": "nope", "protocol": current + 1})
    )));
    assert!(bad(control(json!({"session": "s1", "protocol": 256}))));
    assert!(bad(control(
        json!({"session": "s1", "protocol": current + 1, "extra": 1})
    )));
    assert_eq!(
        control(json!({"session": "s1", "protocol": 0})),
        Ok(Value::Null)
    );
}
