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
    spec_with(handle, json!({}))
}

fn spec_with(handle: &str, limits: Value) -> OpenSpec {
    OpenSpec {
        handle: handle.into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits,
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
    session_in(harness, &spec("a"), start, program)
}

/// [`session_of`] on a Core opened with `spec`.
fn session_in(
    harness: &mut TestkitHarness,
    spec: &OpenSpec,
    start: bool,
    program: &Value,
) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(spec).expect("open");
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

/// Pumps until the host has no runnable work, so its wake is clear. Returns every event that the pumps posted.
fn settle(core: &mut dyn CoreApi, at: Instant) -> Vec<Event> {
    let mut events = vec![];
    for _ in 0..PUMPS {
        let report = core.pump(Now {
            monotonic: at,
            unix: 1_000_000,
        });
        events.extend(core.poll_events(64));
        if !report.more {
            return events;
        }
    }
    panic!("the host did not settle");
}

fn bad(result: Result<Value, ControlError>) -> bool {
    matches!(result, Err(ControlError::Bad(_)))
}

/// Core ST-1, OU-7: the bytes of `pty_output` are the payload's output, and the worker reads all of them, in reads that
/// the seed sizes, into the session's model. Without the control, a `hold` program writes nothing.
#[test]
fn pty_output_bytes_are_read_by_the_worker() {
    for seed in 0..8 {
        let mut harness = TestkitHarness::new(seed);
        let (mut core, at) = session(&mut harness, true);
        settle(core.as_mut(), at);
        assert_eq!(
            screen_text(core.as_mut(), at).trim(),
            "",
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
        settle(core.as_mut(), at);
        assert_eq!(
            screen_text(core.as_mut(), at).trim(),
            "hello",
            "seed {seed}: the worker read every byte, in order"
        );
    }
}

/// The visible text of `s1`'s model, through Core's `ReadScreen`.
fn screen_text(core: &mut dyn CoreApi, mut at: Instant) -> String {
    let read = core
        .begin(Op::ReadScreen {
            session: sid("s1"),
            history: false,
        })
        .unwrap();
    match complete(core, &mut at, read) {
        OpResult::Ok(OpOutput::Screen(screen)) => screen.text,
        other => panic!("ReadScreen: {other:?}"),
    }
}

/// Steward ruling R-76: `pty_output` ignores `scripted`, the fake's mode script, as Core ignores a `Create` request's `fake`.
/// The bytes are written, and the modes come from the bytes alone: a script that claims bracketed paste changes no mode. Any
/// other unknown field is still `Bad`, with zero effect: no byte reaches the program's output.
#[test]
fn pty_output_ignores_scripted_and_refuses_any_other_unknown_field() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    let modes = core.terminal_state(&sid("s1")).unwrap().modes;
    assert!(!modes.bracketed_paste);
    let scripted = json!({"modes": {"bracketed_paste": true}});
    assert!(bad(harness.control(
        "a",
        "pty_output",
        &json!({"session": "s1", "bytes_hex": "78", "script": scripted})
    )));
    settle(core.as_mut(), at);
    assert!(
        !screen_text(core.as_mut(), at).contains('x'),
        "a refused control writes nothing"
    );
    assert_eq!(
        harness.control(
            "a",
            "pty_output",
            &json!({"session": "s1", "bytes_hex": "78", "scripted": scripted})
        ),
        Ok(Value::Null)
    );
    settle(core.as_mut(), at);
    assert!(
        screen_text(core.as_mut(), at).contains('x'),
        "the accepted byte reaches the session's model"
    );
    assert_eq!(core.terminal_state(&sid("s1")).unwrap().modes, modes);
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

/// `program_ready` (R-60): the testkit's program edge has no terminal setup, so a session that the handle has is ready at
/// once, before and after its start. An unknown handle or session, a removed session, and an argument that it does not take,
/// are `Bad`.
#[test]
fn program_ready_is_true_at_once_for_a_session_of_the_handle() {
    let mut harness = TestkitHarness::new(0);
    let args = json!({"session": "s1"});
    assert!(bad(harness.control("a", "program_ready", &args)));
    let (_core, _at) = session(&mut harness, false);
    assert_eq!(
        harness.control("a", "program_ready", &args),
        Ok(json!({"ready": true})),
        "created"
    );
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, true);
    assert_eq!(
        harness.control("a", "program_ready", &args),
        Ok(json!({"ready": true})),
        "started"
    );
    for bad_args in [
        json!({"session": "s9"}),
        json!({"session": "s1", "on": true}),
    ] {
        assert!(
            bad(harness.control("a", "program_ready", &bad_args)),
            "{bad_args}"
        );
    }
    assert!(bad(harness.control("b", "program_ready", &args)));
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, false);
    let remove = core.begin(Op::Remove { id: sid("s1") }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, remove),
        OpResult::Ok(_)
    ));
    assert!(bad(harness.control("a", "program_ready", &args)), "removed");
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

/// Begins a host write of `bytes` to `s1` (Core IN-1).
fn write(core: &mut dyn CoreApi, bytes: &[u8]) -> OpId {
    core.begin(Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Bytes {
            bytes: botster_route_codec::prelude::HexBytes(bytes.to_vec()),
        },
        guard: None,
    })
    .unwrap()
}

/// The input that the program edge of `s1` on `a` took, as `pty_input` reports it.
fn input(harness: &mut TestkitHarness) -> Value {
    harness
        .control("a", "pty_input", &json!({"session": "s1"}))
        .unwrap()["bytes"]["$bytes_hex"]
        .clone()
}

fn outcome(result: OpResult) -> (WriteOutcome, u64) {
    let OpResult::Ok(OpOutput::Input(result)) = result else {
        panic!("a host write completes with an InputResult: {result:?}");
    };
    (result.outcome, result.pty_bytes_written)
}

/// `pty_chunk` (Core AM-2, IN-6): at most the cap reaches the PTY in one pump, and each pump is a new step, so one host
/// write reaches the PTY in pieces and completes with every byte, in order.
#[test]
fn a_pty_chunk_write_reaches_the_pty_in_pieces_across_pumps() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    assert_eq!(
        harness.control("a", "pty_chunk", &json!({"session": "s1", "bytes": 2})),
        Ok(Value::Null)
    );
    let op = write(core.as_mut(), b"abcde");
    let mut more = false;
    for _ in 0..PUMPS {
        more = core
            .pump(Now {
                monotonic: at,
                unix: 1_000_000,
            })
            .more;
        if input(&mut harness) != json!("") {
            break;
        }
    }
    assert_eq!(
        input(&mut harness),
        json!("6162"),
        "one step, one piece of the cap"
    );
    assert!(
        more,
        "the rest of the write is runnable work at the next step"
    );
    assert_eq!(
        outcome(complete(core.as_mut(), &mut at, op)),
        (WriteOutcome::Written, 5)
    );
    assert_eq!(input(&mut harness), json!("6162636465"));
}

/// A program that takes no input now (`pty_blocked`) makes the write wait, not fail: it completes with every byte once
/// the program takes input again (Core AM-2: the program edge's write readiness).
#[test]
fn a_blocked_program_makes_the_write_wait_until_it_takes_input() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1"})),
        Ok(Value::Null)
    );
    let op = write(core.as_mut(), b"ab");
    settle(core.as_mut(), at);
    assert!(
        core.poll_events(64)
            .iter()
            .all(|e| !matches!(e, Event::Completed { op: o, .. } if *o == op)),
        "the write waits while the program takes nothing"
    );
    assert_eq!(input(&mut harness), json!(""));
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null)
    );
    assert_eq!(
        outcome(complete(core.as_mut(), &mut at, op)),
        (WriteOutcome::Written, 2)
    );
    assert_eq!(input(&mut harness), json!("6162"));
}

/// The bytes that `pty_queued` reports for `s1` on `a`.
fn queued(harness: &mut TestkitHarness) -> Value {
    harness
        .control("a", "pty_queued", &json!({"session": "s1"}))
        .unwrap()
}

/// Steward ruling R-74, the held write: `pty_blocked` with `kernel_bytes` lets that many bytes of the write in, unread by the
/// program, and `pty_queued` reports them. The write stays pending. `on: false` delivers the queue first, then the rest.
#[test]
fn a_kernel_queue_holds_the_write_and_pty_queued_reports_it() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    assert_eq!(queued(&mut harness), json!({"bytes": 0}));
    assert_eq!(
        harness.control(
            "a",
            "pty_blocked",
            &json!({"session": "s1", "on": true, "kernel_bytes": 2})
        ),
        Ok(Value::Null)
    );
    let op = write(core.as_mut(), b"abcde");
    // Every event of every pump until idle (#283 HW-TK-2): none completes the write.
    let events = settle(core.as_mut(), at);
    assert_eq!(queued(&mut harness), json!({"bytes": 2}));
    assert_eq!(input(&mut harness), json!(""), "the program read none");
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::Completed { op: o, .. } if *o == op)),
        "the write is held"
    );
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null)
    );
    assert_eq!(queued(&mut harness), json!({"bytes": 0}));
    assert_eq!(
        outcome(complete(core.as_mut(), &mut at, op)),
        (WriteOutcome::Written, 5)
    );
    assert_eq!(input(&mut harness), json!("6162636465"));
}

/// R-74 with `pty_chunk` (#283 HW-TK-1): a spent cap with room left in the kernel queue waits for the next step, so the host
/// keeps pumping until the queue is full, one chunk per step. The full queue holds the write until the release.
#[test]
fn a_chunked_write_fills_the_kernel_queue_before_the_host_settles() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    assert_eq!(
        harness.control("a", "pty_chunk", &json!({"session": "s1", "bytes": 1})),
        Ok(Value::Null)
    );
    assert_eq!(
        harness.control(
            "a",
            "pty_blocked",
            &json!({"session": "s1", "on": true, "kernel_bytes": 3})
        ),
        Ok(Value::Null)
    );
    let op = write(core.as_mut(), b"abcde");
    let events = settle(core.as_mut(), at);
    assert_eq!(queued(&mut harness), json!({"bytes": 3}));
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::Completed { op: o, .. } if *o == op)),
        "the full queue holds the write"
    );
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null)
    );
    assert_eq!(
        outcome(complete(core.as_mut(), &mut at, op)),
        (WriteOutcome::Written, 5)
    );
    assert_eq!(input(&mut harness), json!("6162636465"));
}

/// R-74 with `pty_chunk` and `pty_accept` (#283 HW-TK-1): a spent accept limit refuses the write although the kernel queue
/// has room, so the rest is not work for a later step: the host settles with the one accepted byte queued, and the write
/// is held.
#[test]
fn a_spent_accept_limit_with_room_in_the_kernel_queue_lets_the_host_settle() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    for (op, args) in [
        ("pty_chunk", json!({"session": "s1", "bytes": 1})),
        (
            "pty_blocked",
            json!({"session": "s1", "on": true, "kernel_bytes": 3}),
        ),
        ("pty_accept", json!({"session": "s1", "bytes": 1})),
    ] {
        assert_eq!(harness.control("a", op, &args), Ok(Value::Null), "{op}");
    }
    let op = write(core.as_mut(), b"ab");
    let events = settle(core.as_mut(), at);
    assert_eq!(queued(&mut harness), json!({"bytes": 1}));
    assert_eq!(input(&mut harness), json!(""), "the program read none");
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::Completed { op: o, .. } if *o == op)),
        "the write is held"
    );
}

/// `kernel_bytes` goes with a block only, and `pty_queued` takes a session only: anything else is `Bad`.
#[test]
fn kernel_bytes_and_pty_queued_refuse_what_they_cannot_do() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = session(&mut harness, true);
    for args in [
        json!({"session": "s1", "on": false, "kernel_bytes": 1}),
        json!({"session": "s1", "kernel_bytes": -1}),
        json!({"session": "s1", "kernel_bytes": "1"}),
    ] {
        assert!(bad(harness.control("a", "pty_blocked", &args)), "{args}");
    }
    for args in [
        json!({"session": "s9"}),
        json!({"session": "s1", "bytes": 1}),
        json!({}),
    ] {
        assert!(bad(harness.control("a", "pty_queued", &args)), "{args}");
    }
    assert!(bad(harness.control(
        "b",
        "pty_queued",
        &json!({"session": "s1"})
    )));
}

/// A write that fails with an OS error (`pty_fail_after`) completes `Failed` with its exact count (Core IN-2), and never
/// waits for a readiness that the error does not give.
#[test]
fn a_failed_pty_write_completes_failed_with_its_exact_count() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    assert_eq!(
        harness.control("a", "pty_fail_after", &json!({"session": "s1", "bytes": 1})),
        Ok(Value::Null)
    );
    let op = write(core.as_mut(), b"abc");
    assert_eq!(
        outcome(complete(core.as_mut(), &mut at, op)),
        (WriteOutcome::Failed, 1)
    );
    assert_eq!(input(&mut harness), json!("61"));
}

/// R-47 item 3: `route_fill` writes as the payload of the route's session does, so a payload that has exited is refused, as
/// with `pty_output`.
#[test]
fn route_fill_refuses_a_payload_that_has_exited() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session_of(&mut harness, true, &json!([{"print": {"bytes_hex": "78"}}]));
    let options: AttachOptions = serde_json::from_value(
        json!({"file_directory": "/tmp", "answers_queries": false, "input": true}),
    )
    .unwrap();
    let (_, mut route) = harness
        .attach_stream(
            "a",
            core.as_mut(),
            ClientId("c".into()),
            &sid("s1"),
            options,
        )
        .expect("attached while the payload runs");
    settle(core.as_mut(), at);
    assert_eq!(
        harness
            .control("a", "payload_alive", &json!({"session": "s1"}))
            .unwrap()["alive"],
        false,
        "the probe program ended after its last step"
    );
    let refused = route.control("route_fill", &json!({})).unwrap_err();
    assert!(refused.contains("has exited"), "{refused}");
}

fn attach(
    harness: &mut TestkitHarness,
    core: &mut dyn CoreApi,
) -> Box<dyn botster_core_conformance::RouteClient> {
    let options: AttachOptions = serde_json::from_value(
        json!({"file_directory": "/tmp", "answers_queries": false, "input": true}),
    )
    .unwrap();
    let (_, route) = harness
        .attach_stream("a", core, ClientId("c".into()), &sid("s1"), options)
        .expect("attached");
    route
}

/// F94 / integration R1-1: a route that attaches while `Starting`, before the worker exists, fills once the payload runs.
/// The fill reads the session's row when it runs: before the spawn there is no worker yet.
#[test]
fn route_fill_after_an_attach_before_the_spawn_writes_once_the_payload_runs() {
    for seed in 0..8 {
        let mut harness = TestkitHarness::new(seed);
        let (mut core, mut at) = session_of(&mut harness, false, &json!([{"hold": {}}]));
        let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
        let mut route = attach(&mut harness, core.as_mut());
        // Start is begun and not pumped: Core admits the attach, and the row names no worker yet.
        assert!(session_row(&harness, "a", &sid("s1"))
            .unwrap()
            .worker
            .is_none());
        let early = route.control("route_fill", &json!({})).unwrap_err();
        assert!(
            early.contains("no worker process yet"),
            "seed {seed}: {early}"
        );
        assert!(matches!(
            complete(core.as_mut(), &mut at, start),
            OpResult::Ok(_)
        ));
        let filled = route.control("route_fill", &json!({})).unwrap();
        assert!(
            filled["bytes"].as_u64().unwrap() > 0,
            "seed {seed}: {filled}"
        );
        settle(core.as_mut(), at);
        assert!(
            screen_text(core.as_mut(), at).contains("abc"),
            "seed {seed}: the running payload printed the fill"
        );
    }
}

/// F94 / integration R1-1: a route belongs to the session instance that it attached to. After that session is removed and a
/// new one with the same id runs, the old route's fill is refused, and the new payload gets no byte from it.
#[test]
fn route_fill_of_a_route_of_a_removed_session_is_refused_after_a_recreate() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, mut at) =
        session_of(&mut harness, true, &json!([{"print": {"bytes_hex": "78"}}]));
    let mut route = attach(&mut harness, core.as_mut());
    settle(core.as_mut(), at);
    assert!(matches!(
        core.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
    let remove = core.begin(Op::Remove { id: sid("s1") }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, remove),
        OpResult::Ok(_)
    ));
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, create),
        OpResult::Ok(_)
    ));
    let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), &mut at, start),
        OpResult::Ok(_)
    ));
    settle(core.as_mut(), at);
    let refused = route.control("route_fill", &json!({})).unwrap_err();
    assert!(refused.contains("instance is gone"), "{refused}");
    settle(core.as_mut(), at);
    assert_eq!(
        screen_text(core.as_mut(), at).trim(),
        "",
        "the new payload gets no byte from the old route"
    );
}

/// F94 / integration R1-1 (round 2): Create and Start begun, neither pumped, then the attach: no row is stored yet, and the
/// route still belongs to that instance. After both complete, the fill writes to its payload. After the session is removed
/// and a new one with the same id runs, the old route's fill is refused, and the new payload gets no byte.
#[test]
fn route_fill_after_an_attach_before_the_first_row_belongs_to_that_instance() {
    for seed in 0..8 {
        let mut harness = TestkitHarness::new(seed);
        let mut core = harness.open(&spec("a")).expect("open");
        let mut at = Instant::now();
        let create_s1 = |harness: &TestkitHarness, core: &mut Box<dyn CoreApi>, program: Value| {
            let mut create = json!({"Create": {"session": "s1", "request": {"program": program}}});
            normalize_op(&mut create, &harness.probe_binary());
            core.begin(serde_json::from_value(create).unwrap()).unwrap()
        };
        let create = create_s1(&harness, &mut core, json!([{"hold": {}}]));
        let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
        assert!(
            harness
                .directories()
                .row("d", &botster_core_host::session::row_key(&sid("s1")))
                .is_none(),
            "seed {seed}: no row is stored before the first pump"
        );
        let mut route = attach(&mut harness, core.as_mut());
        let early = route.control("route_fill", &json!({})).unwrap_err();
        assert!(early.contains("no stored row"), "seed {seed}: {early}");
        assert!(matches!(
            complete(core.as_mut(), &mut at, create),
            OpResult::Ok(_)
        ));
        assert!(matches!(
            complete(core.as_mut(), &mut at, start),
            OpResult::Ok(_)
        ));
        let filled = route.control("route_fill", &json!({})).unwrap();
        assert!(
            filled["bytes"].as_u64().unwrap() > 0,
            "seed {seed}: {filled}"
        );
        settle(core.as_mut(), at);
        assert!(
            screen_text(core.as_mut(), at).contains("abc"),
            "seed {seed}: the running payload printed the fill"
        );
        // The replacement: the first instance ends and is removed, and a new s1 runs. The client closes its end first: the
        // route's unread tail would hold its close, and so the Remove. route_fill does not look at the route's state, so only
        // the instance check refuses the fill below.
        route.control("client_close", &json!({})).unwrap();
        let signal = core
            .begin(Op::Signal {
                id: sid("s1"),
                sig: Signal::Kill,
            })
            .unwrap();
        assert!(matches!(
            complete(core.as_mut(), &mut at, signal),
            OpResult::Ok(_)
        ));
        settle(core.as_mut(), at);
        let remove = core.begin(Op::Remove { id: sid("s1") }).unwrap();
        assert!(
            matches!(complete(core.as_mut(), &mut at, remove), OpResult::Ok(_)),
            "seed {seed}"
        );
        let create = create_s1(&harness, &mut core, json!([{"hold": {}}]));
        assert!(matches!(
            complete(core.as_mut(), &mut at, create),
            OpResult::Ok(_)
        ));
        let start = core.begin(Op::Start { id: sid("s1") }).unwrap();
        assert!(matches!(
            complete(core.as_mut(), &mut at, start),
            OpResult::Ok(_)
        ));
        settle(core.as_mut(), at);
        let refused = route.control("route_fill", &json!({})).unwrap_err();
        assert!(
            refused.contains("instance is gone"),
            "seed {seed}: {refused}"
        );
        settle(core.as_mut(), at);
        assert_eq!(
            screen_text(core.as_mut(), at).trim(),
            "",
            "seed {seed}: the new payload gets no byte from the old route"
        );
    }
}

/// DP-5 (#227 F99), the worker with the testkit binding: the binding spends each read allowance read by read. A view-only
/// route's client sends refused frames one at a time, many more bytes than the route's first allowance, and reads the
/// refusal of each before it sends the next. So each read takes one whole frame and the route holds nothing after it: the
/// worker's own value of the allowance does not change, and it still sends a new one when the binding's rest is lower.
/// Every frame is refused, in order, and none stalls in the stream. (The binding's reads are seeded and most end inside a
/// frame, which changes the worker's value and sends a new allowance anyway; the worker test
/// `a_driver_that_spends_its_allowance_on_refused_frames_gets_a_new_one` reads whole frames and so fails without the fix.)
#[test]
fn a_route_reads_past_its_first_allowance_while_its_frames_are_refused() {
    use botster_hub_conformance::route::RouteRead;
    use botster_route_codec::prelude::{
        stream_wrap, Decoded, FrameBounds, Op as WireOp, StreamReader, ToClient, ToWorker,
    };
    const QUEUE: u64 = 2048;
    const FRAMES: u64 = 600;
    let mut harness = TestkitHarness::new(0);
    let spec = spec_with("a", json!({"route_input_queue_bytes": QUEUE}));
    let (mut core, at) = session_in(&mut harness, &spec, true, &json!([{"hold": {}}]));
    let options: AttachOptions = serde_json::from_value(json!({
        "file_directory": "/tmp",
        "answers_queries": false,
        "input": false,
        "route_limits": {"max_frame_bytes": 1024},
    }))
    .unwrap();
    let (_, mut route) = harness
        .attach_stream(
            "a",
            core.as_mut(),
            ClientId("c".into()),
            &sid("s1"),
            options,
        )
        .expect("attached");
    settle(core.as_mut(), at);
    let frame = |k: u64| {
        stream_wrap(
            &ToWorker::Bytes {
                op: WireOp(k),
                bytes: b"x".to_vec().into(),
            }
            .encode(),
        )
    };
    assert!(FRAMES * frame(1).len() as u64 > 4 * QUEUE);
    let bounds = FrameBounds {
        max_frame: 1024,
        max_screen: u64::MAX,
        max_history: u64::MAX,
    };
    let (mut reader, mut ops) = (StreamReader::default(), Vec::new());
    for k in 1..=FRAMES {
        route.write(&frame(k));
        let mut idle = 0;
        while ops.last() != Some(&k) && idle < 4 {
            match route.read(64 * 1024, &botster_conformance::Deadline::none()) {
                RouteRead::Bytes(bytes) => {
                    reader.push(&bytes);
                    while let Some(frame) = reader.next_frame(&bounds).unwrap() {
                        if let Decoded::Frame(ToClient::InputRefused(r)) =
                            ToClient::decode(&frame, &bounds).unwrap()
                        {
                            ops.push(r.op.expect("each frame has its op").0);
                        }
                    }
                }
                _ => idle += 1,
            }
        }
        assert_eq!(ops.last(), Some(&k), "frame {k} stalled in the stream");
    }
    assert_eq!(
        ops,
        (1..=FRAMES).collect::<Vec<_>>(),
        "every frame is refused"
    );
}

/// `input_blocked` (Core DP-5, docs/core-testkit-controls.md): after the workers' ready work, it reports whether the route's
/// stream takes no more bytes from the client. With the PTY blocked, the worker stops reading the route at its input bound
/// (a small `route_input_queue_bytes`), and the stream fills: blocked. Released, the worker writes every byte, reads the rest,
/// and the stream takes bytes again: not blocked. No byte is lost.
#[test]
fn input_blocked_reports_a_stream_that_takes_no_more_client_bytes() {
    use botster_route_codec::prelude::{stream_wrap, Op as WireOp, ToWorker};
    const QUEUE: u64 = 2048;
    const PAYLOAD: usize = 200;
    const FRAMES: usize = 400;
    let mut harness = TestkitHarness::new(0);
    let spec = spec_with("a", json!({"route_input_queue_bytes": QUEUE}));
    let (mut core, at) = session_in(&mut harness, &spec, true, &json!([{"hold": {}}]));
    let options: AttachOptions = serde_json::from_value(json!({
        "file_directory": "/tmp",
        "answers_queries": false,
        "input": true,
        "route_limits": {"max_frame_bytes": 1024},
    }))
    .unwrap();
    let (_, mut route) = harness
        .attach_stream(
            "a",
            core.as_mut(),
            ClientId("c".into()),
            &sid("s1"),
            options,
        )
        .expect("attached");
    settle(core.as_mut(), at);
    let blocked = |route: &mut Box<dyn botster_core_conformance::RouteClient>| {
        route.control("input_blocked", &json!({}))
    };
    assert_eq!(blocked(&mut route), Ok(json!({"blocked": false})));
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": true})),
        Ok(Value::Null)
    );
    let frame = |k: usize| {
        stream_wrap(
            &ToWorker::Bytes {
                op: WireOp(k as u64),
                bytes: vec![b'a' + (k % 26) as u8; PAYLOAD].into(),
            }
            .encode(),
        )
    };
    assert!(FRAMES * frame(1).len() > crate::route_client::ROUTE_STREAM_BYTES + 2 * QUEUE as usize);
    for k in 1..=FRAMES {
        route.write(&frame(k));
    }
    assert_eq!(blocked(&mut route), Ok(json!({"blocked": true})));
    assert_eq!(
        harness.control("a", "pty_blocked", &json!({"session": "s1", "on": false})),
        Ok(Value::Null)
    );
    for _ in 0..PUMPS {
        settle(core.as_mut(), at);
        if blocked(&mut route) == Ok(json!({"blocked": false}))
            && input(&mut harness).as_str().map_or(0, str::len) == 2 * FRAMES * PAYLOAD
        {
            break;
        }
    }
    assert_eq!(blocked(&mut route), Ok(json!({"blocked": false})));
    let expected: String = (1..=FRAMES)
        .map(|k| botster_route_codec::prelude::hex_encode(&[b'a' + (k % 26) as u8; PAYLOAD]))
        .collect();
    assert_eq!(input(&mut harness), json!(expected), "every byte, in order");
}

/// The `more` of each of `n` pumps after `controls` and a host write of `bytes` on a settled session, with what the program
/// read after each. The first pump hands the write to the worker; the second performs it.
fn more_after_write(
    controls: &[(&str, Value)],
    bytes: &[u8],
    n: usize,
) -> (TestkitHarness, Vec<bool>) {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = session(&mut harness, true);
    settle(core.as_mut(), at);
    for (op, args) in controls {
        assert_eq!(harness.control("a", op, args), Ok(Value::Null), "{op}");
    }
    write(core.as_mut(), bytes);
    let more = (0..n)
        .map(|_| {
            let report = core.pump(Now {
                monotonic: at,
                unix: 1_000_000,
            });
            core.poll_events(64);
            report.more
        })
        .collect();
    (harness, more)
}

/// `pty_chunk` with `pty_accept` (F53, TM-6): the write spends the step's cap and the accept limit together. The rest is
/// refused by `pty_accept`, which a new step does not restore, so the pump that writes reports no more work. (Without
/// `accept != Some(0)` in the edge's refusal, that pump reported `more: true` with nothing runnable.)
#[test]
fn a_write_that_spends_the_cap_and_the_accept_limit_reports_no_more_work() {
    let (mut harness, more) = more_after_write(
        &[
            ("pty_chunk", json!({"session": "s1", "bytes": 2})),
            ("pty_accept", json!({"session": "s1", "bytes": 2})),
        ],
        b"abc",
        3,
    );
    assert_eq!(more, [true, false, false]);
    assert_eq!(input(&mut harness), json!("6162"));
}

/// `pty_chunk` with a kernel queue (R-74, TM-6): the write spends the step's cap and fills the queue together. Only the
/// release continues it, so the pump that writes reports no more work.
#[test]
fn a_write_that_spends_the_cap_and_fills_the_kernel_queue_reports_no_more_work() {
    let (mut harness, more) = more_after_write(
        &[
            ("pty_chunk", json!({"session": "s1", "bytes": 2})),
            (
                "pty_blocked",
                json!({"session": "s1", "on": true, "kernel_bytes": 2}),
            ),
        ],
        b"abc",
        3,
    );
    assert_eq!(more, [true, false, false]);
    assert_eq!(queued(&mut harness), json!({"bytes": 2}));
    assert_eq!(input(&mut harness), json!(""), "the program read none");
}

/// `pty_accept` with no `pty_chunk` cap (TM-6): a refused write waits for no step.
#[test]
fn a_write_that_pty_accept_refuses_with_no_cap_reports_no_more_work() {
    let (mut harness, more) = more_after_write(
        &[("pty_accept", json!({"session": "s1", "bytes": 0}))],
        b"a",
        3,
    );
    assert_eq!(more, [true, false, false]);
    assert_eq!(input(&mut harness), json!(""));
}
