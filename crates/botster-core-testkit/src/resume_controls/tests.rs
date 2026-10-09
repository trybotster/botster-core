use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::hex_encode;
use serde_json::json;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
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

/// Pumps until `op` completes, and returns its result.
fn complete(core: &mut dyn CoreApi, at: Instant, op: OpId) -> OpResult {
    for _ in 0..PUMPS {
        core.pump(now(at));
        for event in core.poll_events(64) {
            if let Event::Completed { op: o, result } = event {
                if o == op {
                    return result;
                }
            }
        }
    }
    panic!("the op {op:?} did not complete");
}

/// Pumps until Core has no runnable work (the fence of `await_quiet`).
fn settle(core: &mut dyn CoreApi, at: Instant) {
    for _ in 0..PUMPS {
        let more = core.pump(now(at)).more;
        if !more && core.poll_events(64).is_empty() {
            return;
        }
    }
    panic!("Core did not settle");
}

/// A Core on handle `a` with the session `s1` running a payload that holds.
fn running(harness: &mut TestkitHarness) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec()).expect("open");
    let at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert!(matches!(
        complete(core.as_mut(), at, create),
        OpResult::Ok(_)
    ));
    let start = core.begin(Op::Start { id: sid() }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), at, start),
        OpResult::Ok(_)
    ));
    settle(core.as_mut(), at);
    (core, at)
}

fn sid() -> SessionId {
    SessionId("s1".into())
}

fn output(harness: &mut TestkitHarness, core: &mut dyn CoreApi, at: Instant, bytes: &[u8]) {
    harness
        .control(
            "a",
            "pty_output",
            &json!({"session": "s1", "bytes_hex": hex_encode(bytes)}),
        )
        .expect("pty_output");
    settle(core, at);
}

fn capture(core: &mut dyn CoreApi, at: Instant) -> Capture {
    let op = core
        .begin(Op::CaptureSnapshot {
            session: sid(),
            owner: ClientId("c1".into()),
        })
        .unwrap();
    match complete(core, at, op) {
        OpResult::Ok(OpOutput::Capture(capture)) => capture,
        other => panic!("a capture, got {other:?}"),
    }
}

fn resume_of(harness: &mut TestkitHarness, capture: CaptureId) -> Result<Value, ControlError> {
    harness.control(
        "a",
        "oracle_resume",
        &json!({"session": "s1", "capture": capture}),
    )
}

/// ST-6b: Core's pages of a capture taken inside an SGR sequence, with the output after it, give the session's model. The
/// output before the next capture is consumed after the first one only, so each capture has its own cut.
#[test]
fn cores_pages_with_the_output_after_them_give_the_sessions_model() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"pre \x1b[1;3");
    let first = capture(core.as_mut(), at);
    output(&mut harness, core.as_mut(), at, b"1mX\x1b[0m");
    let second = capture(core.as_mut(), at);
    assert_ne!(first.model_rev, second.model_rev);
    output(&mut harness, core.as_mut(), at, b"\x1b]0;T\x1b\\tail");
    for capture in [first.capture, second.capture] {
        assert_eq!(
            resume_of(&mut harness, capture).unwrap(),
            json!({"equal": true})
        );
    }
}

/// The control compares: pages that are not the state at the capture's cut are not equal.
#[test]
fn pages_of_another_state_are_not_equal() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"abc");
    let taken = capture(core.as_mut(), at);
    output(&mut harness, core.as_mut(), at, b"def");
    let log = harness.workers().model_log(worker(&harness)).unwrap();
    let mut other = Terminal::new(log.size.as_ref().unwrap(), History::On).unwrap();
    other.vt_write(b"xyz");
    harness.captures_of("a").unwrap().insert(
        taken.capture,
        CaptureRecord {
            model_rev: taken.model_rev,
            bytes: other.snapshot().unwrap(),
        },
    );
    assert_eq!(
        resume_of(&mut harness, taken.capture).unwrap(),
        json!({"equal": false})
    );
}

/// The other side is the live model: with Core's capture and the logged suffix unchanged, a model that stepped other
/// output is not equal.
#[test]
fn a_live_model_that_diverged_is_not_equal() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"abc");
    let taken = capture(core.as_mut(), at);
    output(&mut harness, core.as_mut(), at, b"def");
    assert_eq!(
        resume_of(&mut harness, taken.capture).unwrap(),
        json!({"equal": true})
    );
    harness
        .workers()
        .apply_unlogged_output(worker(&harness), b"xyz", at);
    assert_eq!(
        resume_of(&mut harness, taken.capture).unwrap(),
        json!({"equal": false})
    );
}

/// A worker that stops stepping after a valid capture: the edge logged output that its model never applied, so the
/// capture with that suffix is not the live model.
#[test]
fn a_worker_that_stopped_stepping_is_not_equal() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"abc");
    let taken = capture(core.as_mut(), at);
    harness
        .workers()
        .log_unapplied_output(worker(&harness), b"def");
    assert_eq!(
        resume_of(&mut harness, taken.capture).unwrap(),
        json!({"equal": false})
    );
}

fn worker(harness: &TestkitHarness) -> botster_core_edges::edges::ProcessIdentity {
    session_row(harness, "a", &sid())
        .unwrap()
        .worker
        .expect("a worker")
        .identity()
}

/// A capture that the host did not complete, or a revision that the worker never had, is `Bad`, never a verdict.
#[test]
fn an_unknown_capture_or_revision_is_bad() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    let taken = capture(core.as_mut(), at);
    assert!(matches!(
        resume_of(&mut harness, CaptureId(taken.capture.0 + 1)),
        Err(ControlError::Bad(_))
    ));
    harness.captures_of("a").unwrap().insert(
        taken.capture,
        CaptureRecord {
            model_rev: ModelRev(taken.model_rev.0 + 100),
            bytes: Vec::new(),
        },
    );
    assert!(matches!(
        resume_of(&mut harness, taken.capture),
        Err(ControlError::Bad(_))
    ));
}

/// The log keeps the first position of each revision, and a repeated revision adds nothing.
#[test]
fn the_log_keeps_the_first_position_of_each_revision() {
    let mut log = ModelLog::new(Size {
        rows: 2,
        cols: 4,
        cell_px: None,
    });
    log.rev(ModelRev(0));
    log.read(b"ab");
    log.rev(ModelRev(0));
    log.rev(ModelRev(1));
    log.read(b"c");
    log.rev(ModelRev(1));
    log.rev(ModelRev(2));
    assert_eq!(
        log.revs,
        [(ModelRev(0), 0), (ModelRev(1), 2), (ModelRev(2), 3)]
    );
    assert_eq!(log.read_at(ModelRev(1)), Some(2));
    assert_eq!(log.read_at(ModelRev(3)), None);
}

/// The replay keeps the suffix that the step rule does not consume: what an independent terminal consumes of the same
/// bytes in one step.
#[test]
fn the_replay_keeps_the_unconsumed_suffix() {
    let size = Size {
        rows: 2,
        cols: 8,
        cell_px: None,
    };
    let bytes = b"a\x1b]2;t\x1b";
    let mut fresh = Terminal::new(&size, History::On).unwrap();
    let step = fresh.vt_write_until_query(bytes).unwrap();
    assert!(step.consumed < bytes.len(), "the model keeps the ESC");
    let consumed = replay(&size, bytes).unwrap();
    assert_eq!(consumed, step.consumed);
}
