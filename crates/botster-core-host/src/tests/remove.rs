//! `Remove` (Core LC-7, A6-3, R-15): the teardown request, the end of the worker, the release of the captures and the routes,
//! and the ops of the instance.

use super::*;

fn opts() -> AttachOptions {
    AttachOptions {
        file_directory: "/tmp".into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
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

/// Core EV-5c, LC-7: `Remove` sends the teardown request while the queue is full.
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

/// Core AM-3, ID-1, LC-7: `Remove` completes the ops of the old instance, and a late answer never reaches a new instance.
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
    let req = w.last_request("s1");
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

/// Core LC-7, A6-3: the id is not freed before the worker ends, and a worker that does not end within the grace is killed.
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
    let unlink = format!("unlink {}", w.instance_of("s1").0);
    assert!(
        !w.trace.contains(&unlink),
        "the endpoint stays while the worker may live"
    );
    w.exited("s1");
    assert!(
        matches!(w.complete(remove), OpResult::Ok(OpOutput::RemoveReport(r)) if r.uploads == UploadsOutcome::Deleted)
    );
    // DESIGN.md "Adoption (P5)" part 1, SV-9: after the worker's end is verified, the host removes its endpoint, then the
    // row (LC-7 step 4).
    let at = w
        .trace
        .iter()
        .position(|e| *e == unlink)
        .expect("the endpoint is removed");
    assert_eq!(w.trace[at + 1], "delete session/s1");
}

/// Core LC-7, A6-3: a session whose worker cannot be asked has its worker ended, and the outcome is unknown.
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
    let req = w.last_request("s1");
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
    // The exit closes the routes that are bound then (A2-3), so the route that the remove closes attaches after it.
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    w.engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            opts(),
        )
        .unwrap();
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
