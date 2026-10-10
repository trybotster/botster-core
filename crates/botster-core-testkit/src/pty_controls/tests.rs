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
        assert!(
            unread(&harness) > 0,
            "seed {seed}: the running payload's edge has the fill"
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
    let before = unread(&harness);
    let refused = route.control("route_fill", &json!({})).unwrap_err();
    assert!(refused.contains("instance is gone"), "{refused}");
    assert_eq!(
        unread(&harness),
        before,
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
        assert!(
            unread(&harness) > 0,
            "seed {seed}: the running payload's edge has the fill"
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
        let before = unread(&harness);
        let refused = route.control("route_fill", &json!({})).unwrap_err();
        assert!(
            refused.contains("instance is gone"),
            "seed {seed}: {refused}"
        );
        assert_eq!(
            unread(&harness),
            before,
            "seed {seed}: the new payload gets no byte from the old route"
        );
    }
}
