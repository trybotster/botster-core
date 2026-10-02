//! The findings of the first review of P1 (F1 to F16), each as a test of the clause that it names.

use super::*;

fn opts() -> AttachOptions {
    AttachOptions {
        file_directory: "/tmp".into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        connect_deadline: None,
        owner: None,
        query_deadline: Some(Duration::from_secs(1)),
        route_tag: None,
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    }
}

/// Core EV-5c (F1): the effects of a stop go out while the mandatory queue is already full: the graceful request, the
/// grace deadline and the kill. Only the state event waits.
#[test]
fn stop_effects_continue_with_a_full_queue() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 4;
        l.mandatory_events = 2;
        l.stop_grace = Duration::from_millis(100);
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    // Fill the queue with unpolled mandatory events, then admit the stop.
    w.engine.begin(create("s2")).unwrap();
    w.engine.begin(create("s3")).unwrap();
    w.pump();
    assert!(!w.engine.has_room_for_test());
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the graceful request went out"
    );
    assert!(
        w.engine.next_deadline().is_some(),
        "the grace deadline runs"
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running,
        "the state event waits"
    );
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Kill)),
        "the kill followed the grace"
    );
}

/// Core EV-5c, LC-7 (F1): `Remove` sends the teardown request while the queue is full.
#[test]
fn remove_sends_its_teardown_while_the_queue_is_full() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 4;
        l.mandatory_events = 2;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    w.engine.begin(create("s2")).unwrap();
    w.engine.begin(create("s3")).unwrap();
    w.pump();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    assert!(w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)));
}

/// Core AM-4 (F4): the lane of a session returns when the host polls the completion, not when the write finishes.
#[test]
fn a_session_lane_is_held_until_the_completion_is_polled() {
    let mut w = World::new(limits(|l| l.input_ops_per_session = 1));
    w.running("s1");
    let write = |b: u8| Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Bytes {
            bytes: botster_route_codec::prelude::HexBytes(vec![b]),
        },
        guard: None,
    };
    let first = w.engine.begin(write(1)).unwrap();
    w.pump();
    assert_eq!(
        w.engine.begin(write(2)).unwrap_err().code,
        ErrorCode::LaneFull,
        "finished, not polled"
    );
    w.engine.poll_events(64);
    assert!(w.engine.begin(write(2)).is_ok());
    let _ = first;
}

/// Core AM-1, AM-3 (F5): a failed `Create` ends the ops that were admitted after it, and none stays attached.
#[test]
fn a_failed_create_completes_the_start_and_the_remove_that_followed() {
    let mut w = World::default();
    w.fail_row = Some(StorageError::Failed { errno: 5 });
    let create_op = w.engine.begin(create("s1")).unwrap();
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let mut done = BTreeMap::new();
    for _ in 0..10 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result } = e {
                done.insert(op, result);
            }
        }
    }
    assert!(
        matches!(done.get(&create_op), Some(OpResult::Err(e)) if e.code == ErrorCode::RegistryFailed { uncertain: false })
    );
    assert!(
        matches!(done.get(&start), Some(OpResult::Err(e)) if e.code == ErrorCode::RegistryFailed { uncertain: false })
    );
    assert!(
        w.engine.ops.is_empty(),
        "no op stays attached to the failed session"
    );
    w.fail_row = Some(StorageError::Failed { errno: 5 });
    let c = w.engine.begin(create("s2")).unwrap();
    let u = w
        .engine
        .begin(Op::UpdateMetadata {
            id: sid("s2"),
            labels: BTreeMap::new(),
        })
        .unwrap();
    let r = w.engine.begin(Op::Remove { id: sid("s2") }).unwrap();
    for _ in 0..10 {
        w.pump();
        w.engine.poll_events(64);
    }
    assert!(w.engine.ops.is_empty(), "{c:?} {r:?} {u:?} still pending");
}

/// Core AM-3, ID-1, LC-7 (F6): `Remove` completes the ops of the old instance, and a late answer never reaches a new instance.
#[test]
fn remove_ends_the_ops_of_the_instance_and_a_new_instance_is_untouched() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    let read = w
        .engine
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    w.pump();
    let req = *w.engine.sessions[&sid("s1")]
        .inflight
        .keys()
        .next()
        .expect("sent");
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
    );
    w.exited("s1");
    let mut done = BTreeMap::new();
    for _ in 0..10 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result } = e {
                done.insert(op, result);
            }
        }
    }
    assert!(
        matches!(done.get(&read), Some(OpResult::Err(e)) if e.code == ErrorCode::WorkerLinkFailed)
    );
    assert!(matches!(
        done.get(&remove),
        Some(OpResult::Ok(OpOutput::RemoveReport(_)))
    ));
    w.ok(create("s1"));
    // A late answer to the old request finds nothing.
    w.feed(Input::LinkMsg {
        link: LinkId(1),
        msg: WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Unit),
        },
    });
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Created
    );
    assert_eq!(w.engine.cancel(read), CancelResult::UnknownOp, "ID-1");
}

/// Core LC-5, AD-6 (F7): with the link gone, a stop signals only the verified worker (pid and start time), never a bare
/// payload group; the session ends `Lost`.
#[test]
fn a_broken_link_stop_signals_the_verified_worker_only() {
    use botster_core_edges::edges::GroupSignal;
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let worker = w.identity_of("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let op = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert!(w.signals.contains(&(worker, GroupSignal::Term)));
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(w.signals.contains(&(worker, GroupSignal::Kill)));
    assert!(
        w.signals.iter().all(|(id, _)| *id == worker),
        "no process but the verified worker is signalled"
    );
    assert!(matches!(
        w.complete(op),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(
            LostReason::WorkerUnreachable
        )))
    ));
}

/// Core LC-7, A6-3 (F8): the id is not freed before the worker ends, and a worker that does not end within the grace is killed.
#[test]
fn remove_waits_for_the_worker_to_end() {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
    );
    w.pump();
    assert!(
        w.engine.poll_events(64).is_empty(),
        "the worker is still alive: no Released yet"
    );
    assert!(w.rows.contains_key("session/s1"));
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.signals
            .iter()
            .any(|(_, s)| *s == botster_core_edges::edges::GroupSignal::Kill),
        "the stray worker is killed"
    );
    // A signal is not an observed exit: the id stays taken until the exit is seen (A6-3).
    w.pump();
    assert!(w.engine.poll_events(64).is_empty(), "Remove still waits");
    w.exited("s1");
    assert!(
        matches!(w.complete(remove), OpResult::Ok(OpOutput::RemoveReport(r)) if r.uploads == UploadsOutcome::Deleted)
    );
}

/// Core LC-7, A6-3 (F8): a session whose worker cannot be asked has its worker ended, and the outcome is unknown.
#[test]
fn a_remove_without_a_link_kills_the_stray_worker() {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    let worker = w.identity_of("s1");
    assert!(w
        .signals
        .contains(&(worker, botster_core_edges::edges::GroupSignal::Kill)));
    w.exited("s1");
    assert!(
        matches!(w.complete(remove), OpResult::Ok(OpOutput::RemoveReport(r))
        if r.uploads == UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown))
    );
}

/// Core LC-12, AM-3, steward ruling R-16 (F10): the Stopping row of a `StopAll` target is best effort. The write fails, the
/// stop goes on, the target ends, and `StopAll` completes.
#[test]
fn stop_all_goes_on_when_the_stopping_row_write_fails() {
    let mut w = World::default();
    w.running("s1");
    let all = w.engine.begin(Op::StopAll).unwrap();
    w.fail_row = Some(StorageError::Failed { errno: 5 });
    assert_eq!(w.complete(all), OpResult::Ok(OpOutput::Unit));
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the stop was sent"
    );
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_) | SessionState::Lost(_)
    ));
}

/// Core LC-5, R-16 (F10): a plain `Stop` keeps `RegistryFailed` when its row write fails, and the session stays `Running`.
#[test]
fn a_plain_stop_keeps_registry_failed_when_the_row_write_fails() {
    let mut w = World::default();
    w.running("s1");
    let op = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.fail_row = Some(StorageError::Failed { errno: 5 });
    assert!(
        matches!(w.complete(op), OpResult::Err(e) if matches!(e.code, ErrorCode::RegistryFailed { .. }))
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
}

/// Core OR-1, TM-2 (F12): a setter of a `Created` session is visible only after a pump, and a start waits for it.
#[test]
fn a_created_setter_changes_nothing_before_the_pump_and_the_start_sees_it() {
    let mut w = World::default();
    w.ok(create("s1"));
    let new = Size {
        rows: 30,
        cols: 100,
        cell_px: None,
    };
    w.engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: new,
        })
        .unwrap();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().size,
        size(),
        "begin makes no progress"
    );
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    assert_eq!(w.engine.get(&sid("s1")).unwrap().size, new);
    let launch = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Launch(spec) => Some(spec.size),
            _ => None,
        })
        .expect("the start ran");
    assert_eq!(
        launch, new,
        "the launch carries the size of the earlier Resize (AM-1 order)"
    );
}

/// Core ID-1, IN-6 (F13): the identity of an op is exact: an id that was never minted is `UnknownOp`, and an old op stays
/// `TooLate` or `UnknownOp` by its instance after any number of ops.
#[test]
fn cancel_identity_is_exact_for_the_life_of_the_handle() {
    let mut w = World::default();
    assert_eq!(w.engine.cancel(OpId(0)), CancelResult::UnknownOp);
    assert_eq!(
        w.engine.cancel(OpId(1)),
        CancelResult::UnknownOp,
        "never minted"
    );
    w.ok(create("s1"));
    let first = OpId(1);
    for _ in 0..5000 {
        let op = w
            .engine
            .begin(Op::UpdateMetadata {
                id: sid("s1"),
                labels: BTreeMap::new(),
            })
            .unwrap();
        w.complete(op);
    }
    assert_eq!(w.engine.cancel(first), CancelResult::TooLate);
    w.ok(Op::Remove { id: sid("s1") });
    assert_eq!(
        w.engine.cancel(first),
        CancelResult::UnknownOp,
        "the instance is gone"
    );
}

/// Core OU-1, EV-8, A7-1 (F16): a query deadline below 1 ms is refused before a route is reserved; 1 ms and the maximum attach.
#[test]
fn a_query_deadline_below_one_millisecond_is_refused() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let attach = |w: &mut World, d: Duration| {
        let mut o = opts();
        o.query_deadline = Some(d);
        w.engine.attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            o,
        )
    };
    assert_eq!(
        attach(&mut w, Duration::from_nanos(1)).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        attach(&mut w, Duration::from_micros(999)).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert!(attach(&mut w, Duration::from_millis(1)).is_ok());
    let max = w.engine.limits().max_query_deadline;
    assert!(attach(&mut w, max).is_ok());
    assert!(
        w.engine.routes.len() == 2,
        "a refused attach reserved nothing"
    );
}

/// Core TM-3, TM-5 (F3): a due deadline is the first ready work, before any step or op.
#[test]
fn a_due_deadline_is_listed_before_other_work() {
    let mut w = World::new(limits(|l| l.capture_ttl = Duration::from_secs(1)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = w
        .engine
        .begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId("c".into()),
        })
        .unwrap();
    w.pump();
    let req = *w.engine.sessions[&sid("s1")]
        .inflight
        .keys()
        .next()
        .unwrap();
    w.worker_says("s1", WorkerMsg::Pages { req, pages: vec![] });
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Capture(Capture {
                capture: CaptureId(0),
                page_count: 0,
                total_bytes: 0,
                model_rev: ModelRev(1),
            })),
        },
    );
    w.complete(op);
    w.engine.begin(create("s2")).unwrap();
    w.advance(Duration::from_secs(2));
    w.feed(Input::Clock(w.unix));
    assert_eq!(w.engine.ready().first(), Some(&Work::Deadline));
}

impl HostEngine {
    /// Whether one more mandatory event fits, for tests.
    pub(crate) fn has_room_for_test(&self) -> bool {
        self.has_room()
    }
}

/// Core LC-7, steward ruling R-15: with the queue full, step 1 (close each bound route, with its `RouteClosed`) parks, and
/// steps 2 to 5 wait for it: the open capture stays readable and no teardown request goes out until the poll frees room and
/// `RouteClosed` is posted. Then the capture is released and the teardown request is sent.
#[test]
fn remove_with_a_bound_route_waits_for_route_closed_before_releasing_captures() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 4;
        l.mandatory_events = 2;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = w
        .engine
        .begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId("c".into()),
        })
        .unwrap();
    w.pump();
    let req = *w.engine.sessions[&sid("s1")]
        .inflight
        .keys()
        .next()
        .unwrap();
    w.worker_says(
        "s1",
        WorkerMsg::Pages {
            req,
            pages: vec![Page {
                index: 0,
                bytes: botster_route_codec::prelude::HexBytes(vec![1]),
                last: true,
            }],
        },
    );
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Capture(Capture {
                capture: CaptureId(0),
                page_count: 1,
                total_bytes: 1,
                model_rev: ModelRev(1),
            })),
        },
    );
    let capture = match w.complete(op) {
        OpResult::Ok(OpOutput::Capture(c)) => c.capture,
        other => panic!("{other:?}"),
    };
    w.engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            opts(),
        )
        .unwrap();
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    // Fill the queue, then remove.
    w.engine.begin(create("s2")).unwrap();
    w.engine.begin(create("s3")).unwrap();
    w.pump();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    assert!(
        w.engine.read_page(capture, 0).is_ok(),
        "step 2 waits for step 1"
    );
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)),
        "no teardown request while a route is still bound (R-15)"
    );
    let freed = w.engine.poll_events(64);
    assert!(!freed.is_empty());
    w.pump();
    let events = w.engine.poll_events(64);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::RouteClosed {
            reason: RouteCloseReason::SessionRemoved,
            ..
        }
    )));
    assert_eq!(
        w.engine.read_page(capture, 0).unwrap_err().code,
        ErrorCode::UnknownCapture,
        "the capture is released after RouteClosed"
    );
    assert!(w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)));
}
