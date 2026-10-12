//! Unit tests of FakeCore: the choice points are reachable (design 7.2 rule 3), and the branches that apply a clause to a value that only
//! the script supplies (design 7.2 rule 6, "script-dependent"), boundary included.

use botster_core_contract::prelude::*;
use botster_fake_core::choice::Choice;
use botster_fake_core::{FakeCore, FakeStore};
use proptest::prelude::*;
use serde_json::json;
use std::time::Instant;

fn open(limits: serde_json::Value, seed: u64) -> FakeCore {
    let limits: CoreLimits = serde_json::from_value(limits).unwrap();
    let config = OpenConfig {
        data_dir: "d".into(),
        worker_path: Some("w".into()),
        limits,
    };
    FakeCore::open(&FakeStore::new(), config, seed).unwrap()
}

fn now() -> Now {
    Now {
        monotonic: Instant::now(),
        unix: 1,
    }
}

/// Creates and starts `s1` with the given fake script, and pumps until it runs.
fn running(core: &mut FakeCore, script: serde_json::Value) -> SessionId {
    let argv = vec![
        "botster-conformance-probe".to_string(),
        json!({ "program": [{ "hold": {} }], "fake": script }).to_string(),
    ];
    let request: SpawnRequest = serde_json::from_value(json!({
        "argv": argv, "env": {}, "cwd": "/", "size": { "rows": 24, "cols": 80 }
    }))
    .unwrap();
    let id = SessionId("s1".into());
    core.begin(Op::Create {
        session: id.clone(),
        request,
    })
    .unwrap();
    core.begin(Op::Start { id: id.clone() }).unwrap();
    for _ in 0..20 {
        core.pump(now());
        core.poll_events(64);
    }
    id
}

fn key(repeat: Option<u16>) -> Op {
    serde_json::from_value(
        json!({ "WriteInput": { "session": "s1", "payload": { "key": {
        "key": { "char": "a" }, "mods": [], "event": "press", "repeat": repeat } } } }),
    )
    .unwrap()
}

fn wheel(notches: u32) -> Op {
    serde_json::from_value(json!({ "WriteInput": { "session": "s1", "payload": { "mouse": {
        "action": "wheel", "button": "wheel_up", "row": 0, "col": 0, "mods": [], "notches": notches } } } }))
    .unwrap()
}

/// Core IN-9: the worst-case size of a key write is `repeat` times the scripted size, and the bound is `max_paste_bytes`. The boundary is exact.
#[test]
fn key_repeat_bound_is_repeat_times_the_scripted_size() {
    let limits =
        json!({ "max_paste_bytes": 1024, "max_route_frame_bytes": 1048576, "max_key_repeat": 64 });
    let mut core = open(limits, 0);
    running(&mut core, json!({ "admission": { "key": 16 } }));
    // 64 * 16 = 1024 is within the bound; 16 is the size of one press.
    assert!(core.begin(key(Some(64))).is_ok());
    let mut core = open(
        json!({ "max_paste_bytes": 1024, "max_route_frame_bytes": 1048576, "max_key_repeat": 64 }),
        0,
    );
    running(&mut core, json!({ "admission": { "key": 17 } }));
    let refused = core.begin(key(Some(64))).unwrap_err();
    assert_eq!(refused.code, ErrorCode::PayloadTooLarge);
}

/// Core IN-9: a wheel write counts `notches` times the scripted size, whatever the mode.
#[test]
fn wheel_bound_is_notches_times_the_scripted_size() {
    let mut core = open(
        json!({ "max_paste_bytes": 1024, "max_route_frame_bytes": 1048576 }),
        0,
    );
    running(&mut core, json!({ "admission": { "mouse": 8 } }));
    assert!(core.begin(wheel(128)).is_ok());
    assert_eq!(
        core.begin(wheel(129)).unwrap_err().code,
        ErrorCode::PayloadTooLarge
    );
}

/// Core IN-9: a script outcome of `unsupported` becomes the typed zero with that `what`.
#[test]
fn scripted_unsupported_outcome_is_a_typed_zero() {
    let mut core = open(json!({}), 0);
    running(
        &mut core,
        json!({ "writes": { "key": { "not_written": { "unsupported": "named_key" } } }, "admission": { "key": 4 } }),
    );
    let op = core.begin(key(None)).unwrap();
    for _ in 0..10 {
        core.pump(now());
    }
    let done = core.poll_events(64).into_iter().find_map(|e| match e {
        Event::Completed { op: o, result } if o == op => Some(result),
        _ => None,
    });
    let Some(OpResult::Ok(OpOutput::Input(r))) = done else {
        panic!("no completion")
    };
    assert_eq!(
        r.outcome,
        WriteOutcome::NotWritten(NotWrittenReason::Unsupported {
            what: UnsupportedWhat::NamedKey
        })
    );
    assert_eq!((r.payload_bytes_written, r.pty_bytes_written), (0, 0));
}

/// Design 7.2 rule 3: every choice point that FakeCore names is reached, with more than one outcome, under the CI seeds.
#[test]
fn every_choice_point_is_reachable() {
    let mut seen: std::collections::BTreeMap<Choice, std::collections::BTreeSet<usize>> =
        Default::default();
    for seed in 0..32 {
        let mut core = open(json!({}), seed);
        running(&mut core, json!({}));
        core.begin(Op::UpdateMetadata {
            id: SessionId("s1".into()),
            labels: Default::default(),
        })
        .unwrap();
        let size = Size {
            rows: 30,
            cols: 90,
            cell_px: None,
        };
        core.begin(Op::Resize {
            session: SessionId("s1".into()),
            size,
        })
        .unwrap();
        core.begin(Op::Stop {
            id: SessionId("s1".into()),
        })
        .unwrap();
        for _ in 0..40 {
            core.pump(now());
            core.poll_events(64);
        }
        for (k, v) in core.choices_taken() {
            seen.entry(k).or_default().extend(v);
        }
    }
    for point in [
        Choice::SessionOrder,
        Choice::PumpsPerOp,
        Choice::StartPumps,
        Choice::StopPumps,
        Choice::PollBatch,
        Choice::SpuriousWake,
        Choice::KeyedDefer,
    ] {
        assert!(
            seen.get(&point).is_some_and(|s| s.len() > 1),
            "{point:?} took fewer than two outcomes"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Core IN-9, IN-5: `begin` refuses a key write exactly when its worst-case size is over `max_paste_bytes`.
    #[test]
    fn key_admission_matches_the_bound(repeat in 1u16..=64, size in 1u64..=64, max in 1024u64..=4096) {
        let mut core = open(json!({ "max_paste_bytes": max, "max_route_frame_bytes": 1048576, "max_key_repeat": 64 }), 1);
        running(&mut core, json!({ "admission": { "key": size } }));
        let result = core.begin(key(Some(repeat)));
        prop_assert_eq!(result.is_err(), u64::from(repeat) * size > max);
    }

    /// Core 9B: a zero in any one numeric limit makes `open` fail `InvalidConfig`.
    #[test]
    fn a_zero_limit_is_invalid_config(field in 0usize..45) {
        let defaults = serde_json::to_value(CoreLimits::default()).unwrap();
        let name = defaults.as_object().unwrap().iter().filter(|(_, v)| v.is_number()).nth(field).map(|(k, _)| k.clone());
        let Some(name) = name else { return Ok(()); };
        let mut limits = defaults;
        limits[&name] = json!(0);
        let limits: CoreLimits = serde_json::from_value(limits).unwrap();
        let config = OpenConfig { data_dir: "d".into(), worker_path: Some("w".into()), limits };
        let err = FakeCore::open(&FakeStore::new(), config, 0).err().expect("a zero limit is refused");
        let refused = matches!(err.code, ErrorCode::InvalidConfig { .. });
        prop_assert!(refused);
    }
}

/// Core EV-1, EV-6: a class K event that is replaced moves to the position of the newer observation, and carries the latest value.
#[test]
fn a_replaced_keyed_event_moves_to_the_latest_position() {
    use botster_fake_core::queue::Queue;
    let (id, instance) = (SessionId("s1".into()), InstanceId("i".into()));
    let title = |t: &str| Event::TitleChanged {
        id: id.clone(),
        instance: instance.clone(),
        title: t.into(),
    };
    let bell = Event::Bell {
        id: id.clone(),
        instance: instance.clone(),
        at: 1,
    };
    let mut q = Queue::default();
    q.post(title("one"));
    q.post(bell.clone());
    q.post(title("three"));
    assert_eq!(q.take(10), vec![bell, title("three")]);
}

fn bytes_write(hex: &str) -> Op {
    serde_json::from_value(json!({ "WriteInput": { "session": "s1", "payload": { "bytes": { "bytes": { "$bytes_hex": hex } } } } })).unwrap()
}

/// Core AM-4, ID-1: the capacity charge of an old instance is not given back to a new instance of the same id when the old completion is polled.
#[test]
fn polling_an_old_instance_completion_does_not_release_the_new_instance_charge() {
    let mut core = open(json!({ "input_ops_per_session": 1 }), 0);
    let id = running(&mut core, json!({}));
    let old = core.begin(bytes_write("61")).unwrap();
    for _ in 0..10 {
        core.pump(now());
    }
    core.begin(Op::Stop { id: id.clone() }).unwrap();
    for _ in 0..20 {
        core.pump(now());
    }
    core.begin(Op::Remove { id: id.clone() }).unwrap();
    for _ in 0..20 {
        core.pump(now());
    }
    assert!(core.get(&id).is_err(), "the old instance is gone");
    // The old completion is still unpolled at the front of the queue. The new instance runs and its write completes unpolled.
    let request: SpawnRequest = serde_json::from_value(
        json!({ "argv": ["x"], "env": {}, "cwd": "/", "size": { "rows": 24, "cols": 80 } }),
    )
    .unwrap();
    core.begin(Op::Create {
        session: id.clone(),
        request,
    })
    .unwrap();
    core.begin(Op::Start { id: id.clone() }).unwrap();
    for _ in 0..20 {
        core.pump(now());
    }
    core.begin(bytes_write("62")).unwrap();
    for _ in 0..10 {
        core.pump(now());
    }
    let front = core.poll_events(1);
    assert!(
        matches!(front.as_slice(), [Event::Completed { op, .. }] if *op == old),
        "the old write completion comes first"
    );
    assert_eq!(
        core.begin(bytes_write("63")).unwrap_err().code,
        ErrorCode::LaneFull
    );
}
