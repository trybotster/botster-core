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

fn sid(name: &str) -> SessionId {
    SessionId(name.into())
}

/// Pumps until `op` completes, and returns its result. When a pump leaves no runnable work and no event, the test moves
/// its clock to the deadline that Core armed, as the conformance driver's injected clock does (TM-1, TM-3).
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
    session_of(harness, start, &json!([{"hold": {}}]))
}

/// [`session`] with the payload's probe `program`.
fn session_of(
    harness: &mut TestkitHarness,
    start: bool,
    program: &Value,
) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec("a")).expect("open");
    let mut at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {"program": program}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
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

/// Pumps until the host has no runnable work, so its wake is clear.
fn settle(core: &mut dyn CoreApi, at: Instant) {
    for _ in 0..PUMPS {
        let report = core.pump(Now {
            monotonic: at,
            unix: 1_000_000,
        });
        core.poll_events(64);
        if !report.more {
            return;
        }
    }
    panic!("the host did not settle");
}

/// The output bytes of the payload of `s1` on `a` that the worker has not read.
fn unread(harness: &TestkitHarness) -> usize {
    let row = session_row(harness, "a", &sid("s1")).unwrap();
    let (program, _) = harness
        .workers()
        .program_edge(row.worker.unwrap().identity())
        .unwrap();
    program.output_unread()
}

fn bad(result: Result<Value, ControlError>) -> bool {
    matches!(result, Err(ControlError::Bad(_)))
}

/// Core ST-1, OU-7: the bytes of `pty_output` are the payload's output, and the worker reads all of them, in reads that
/// the seed sizes. Without the control, a `hold` program writes nothing. (The M1 worker reports no output to the host and
/// serves no terminal read, so the testkit cannot see the bytes past the worker's read yet.)
#[test]
fn pty_output_bytes_are_read_by_the_worker() {
    for seed in 0..8 {
        let mut harness = TestkitHarness::new(seed);
        let (mut core, at) = session(&mut harness, true);
        settle(core.as_mut(), at);
        assert_eq!(
            unread(&harness),
            0,
            "seed {seed}: the program printed nothing"
        );
        for part in ["68656c", "6c6f"] {
            assert_eq!(
                harness.control(
                    "a",
                    "pty_output",
                    &json!({"op": "pty_output", "session": "s1", "bytes_hex": part})
                ),
                Ok(Value::Null)
            );
        }
        assert_eq!(
            unread(&harness),
            5,
            "seed {seed}: the five bytes wait for the worker"
        );
        settle(core.as_mut(), at);
        assert_eq!(
            unread(&harness),
            0,
            "seed {seed}: the worker read every byte"
        );
    }
}

/// Core TM-6: the output is an edge event, so it wakes the idle host that owns the worker, as a real PTY's readiness does.
#[test]
fn pty_output_wakes_the_idle_host() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    // #194: a wait on an idle host may give a seeded spurious `Woken` unless the test arms `no_spurious_wakes`.
    assert_eq!(
        harness.control("a", "no_spurious_wakes", &json!({})),
        Ok(Value::Null)
    );
    settle(core.as_mut(), at);
    let wake = core.wake_handle();
    assert_eq!(
        wake.wait(Duration::ZERO),
        Wake::TimedOut,
        "the host is idle"
    );
    assert_eq!(
        harness.control(
            "a",
            "pty_output",
            &json!({"session": "s1", "bytes_hex": "78"})
        ),
        Ok(Value::Null)
    );
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
}

/// `pty_blocked` with `on: false` makes the terminal writable again, so it wakes the idle host; `on: true` makes nothing
/// ready, so it does not. A release without a block is a no-op that still answers. (No input write reaches the program
/// on the testkit yet, so the harness cannot observe a blocked write itself.)
#[test]
fn only_a_release_of_pty_blocked_wakes_the_idle_host() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    // #194: a wait on an idle host may give a seeded spurious `Woken` unless the test arms `no_spurious_wakes`.
    assert_eq!(
        harness.control("a", "no_spurious_wakes", &json!({})),
        Ok(Value::Null)
    );
    settle(core.as_mut(), at);
    let wake = core.wake_handle();
    assert_eq!(
        wake.wait(Duration::ZERO),
        Wake::TimedOut,
        "the host is idle"
    );
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null),
        "a release without a block is a no-op"
    );
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
    settle(core.as_mut(), at);
    assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1"})),
        Ok(Value::Null),
        "`on` is true by default"
    );
    assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null)
    );
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
}

/// Both controls refuse what they cannot do, with `Bad`: an unknown handle, an unknown session, a session with no worker
/// process yet, an argument that they do not take, and (`pty_output`) bytes that are not one or more hex pairs.
#[test]
fn the_program_controls_refuse_what_they_cannot_do() {
    let mut harness = TestkitHarness::new(0);
    let output = |hex: &str| json!({"session": "s1", "bytes_hex": hex});
    let blocked = json!({"session": "s1"});
    assert!(bad(harness.control("a", "pty_output", &output("78"))));
    assert!(bad(harness.control("a", "pty_blocked", &blocked)));
    let (_core, _at) = session(&mut harness, false);
    assert!(
        bad(harness.control("a", "pty_output", &output("78"))),
        "no worker yet"
    );
    assert!(
        bad(harness.control("a", "pty_blocked", &blocked)),
        "no worker yet"
    );
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, true);
    for args in [
        json!({"session": "s9", "bytes_hex": "78"}),
        json!({"session": "s1", "bytes_hex": "78", "atomic": true}),
        json!({"session": "s1"}),
        output(""),
        output("7"),
        output("zz"),
    ] {
        assert!(bad(harness.control("a", "pty_output", &args)), "{args}");
    }
    assert!(bad(harness.control("b", "pty_output", &output("78"))));
    for args in [
        json!({"session": "s9"}),
        json!({"session": "s1", "on": true, "bytes": 1}),
        json!({"on": true}),
    ] {
        assert!(bad(harness.control("a", "pty_blocked", &args)), "{args}");
    }
    assert!(bad(harness.control("b", "pty_blocked", &blocked)));
    assert_eq!(
        harness.control("a", "pty_output", &output("78")),
        Ok(Value::Null)
    );
}

/// A payload that has exited writes nothing more: its PTY stays readable until the reap, but `pty_output` is `Bad`.
#[test]
fn pty_output_refuses_a_payload_that_has_exited() {
    for seed in 0..8 {
        let mut harness = TestkitHarness::new(seed);
        let (mut core, at) =
            session_of(&mut harness, true, &json!([{"print": {"bytes_hex": "78"}}]));
        settle(core.as_mut(), at);
        assert_eq!(
            harness
                .control("a", "payload_alive", &json!({"session": "s1"}))
                .unwrap()["alive"],
            false,
            "seed {seed}: the probe program ended after its last step"
        );
        let row = session_row(&harness, "a", &sid("s1")).unwrap();
        assert!(
            harness
                .workers()
                .program_edge(row.worker.unwrap().identity())
                .is_ok(),
            "seed {seed}: the payload is not reaped yet"
        );
        assert!(
            bad(harness.control(
                "a",
                "pty_output",
                &json!({"session": "s1", "bytes_hex": "78"})
            )),
            "seed {seed}"
        );
    }
}

/// The input controls refuse what they cannot do, with `Bad`: an unknown handle or session, a session with no worker yet,
/// an argument that they do not take, a missing `bytes`, and (`pty_chunk`) a cap of zero, which would let no input through.
#[test]
fn the_input_controls_refuse_what_they_cannot_do() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, false);
    for op in ["pty_input", "pty_chunk", "pty_accept", "pty_fail_after"] {
        let args = if op == "pty_input" {
            json!({"session": "s1"})
        } else {
            json!({"session": "s1", "bytes": 1})
        };
        assert!(bad(harness.control("a", op, &args)), "{op}: no worker yet");
    }
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, true);
    for op in ["pty_chunk", "pty_accept", "pty_fail_after"] {
        for args in [
            json!({"session": "s9", "bytes": 1}),
            json!({"session": "s1"}),
            json!({"session": "s1", "bytes": 1, "on": true}),
            json!({"session": "s1", "bytes": -1}),
        ] {
            assert!(bad(harness.control("a", op, &args)), "{op} {args}");
        }
        assert!(bad(harness.control(
            "b",
            op,
            &json!({"session": "s1", "bytes": 1})
        )));
        assert_eq!(
            harness.control("a", op, &json!({"session": "s1", "bytes": 1})),
            Ok(Value::Null),
            "{op}"
        );
    }
    assert!(bad(harness.control(
        "a",
        "pty_chunk",
        &json!({"session": "s1", "bytes": 0})
    )));
    for args in [
        json!({"session": "s9"}),
        json!({"session": "s1", "bytes": 1}),
    ] {
        assert!(bad(harness.control("a", "pty_input", &args)), "{args}");
    }
}

/// `pty_input` is the observer of the input controls: the bytes that the program edge took, in order, as hex. A `hold`
/// program that took no input gives an empty log.
#[test]
fn pty_input_reports_the_input_that_the_program_took() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, true);
    assert_eq!(
        harness.control("a", "pty_input", &json!({"session": "s1"})),
        Ok(json!({"bytes": {"$bytes_hex": ""}}))
    );
}
