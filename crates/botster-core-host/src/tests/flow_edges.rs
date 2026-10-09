//! The edges of the flows: a start that fails each way, a stop whose row fails, and a remove whose row delete fails (Core
//! AD-7, LC-4, LC-7, LC-12, R-16, A2-1).

use super::*;
use botster_core_edges::edges::{GroupSignal, StorageError};

/// Core LC-4, A2-1: a launch that the worker refuses ends the start `Exited`, with the exit recorded, and the session is
/// not started again; a stop that waited for the start does not run on the failed session.
#[test]
fn a_refused_launch_leaves_an_exited_session_and_no_stop_runs() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::LaunchFailed {
            reason: StartFailReason::ExecFailed { errno: 2 },
        },
    });
    let mut seen = Vec::new();
    for _ in 0..10 {
        w.pump();
        seen.extend(w.engine.poll_events(64));
    }
    assert!(seen
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Err(_) } if *op == start)));
    assert!(seen.iter().any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(OpOutput::End(SessionEnd::Exited(_))) } if *op == stop)));
    let record = w.engine.get(&sid("s1")).unwrap();
    assert!(
        matches!(record.state, SessionState::Exited(_)),
        "{record:?}"
    );
    assert!(record.exit.is_some(), "the exit is recorded");
    assert_eq!(
        w.engine
            .begin(Op::Start { id: sid("s1") })
            .unwrap_err()
            .code,
        ErrorCode::WrongState,
        "A2-1: an exited session is not started again"
    );
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "no stop is sent to a payload that never ran"
    );
}

/// Core AD-7: when the identity row of a start cannot be written, the payload is never launched: the worker is killed, its
/// link is closed, and the session is `Lost(StartInterrupted)` with `RegistryFailed`.
#[test]
fn a_failed_identity_row_kills_the_worker_and_closes_its_link() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    // Run the start until the worker is spawned; the next row write is the identity row.
    for _ in 0..40 {
        if w.trace.iter().any(|t| t == "spawn") {
            break;
        }
        w.feed(Input::Run(crate::io::Work::Session(sid("s1"))));
    }
    assert!(w.trace.iter().any(|t| t == "spawn"));
    w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
    let instance = w.instance_of("s1");
    let token = w.token_of("s1");
    // The worker says hello; the identity row write then fails.
    let link = LinkId(w.next_link);
    w.next_link += 1;
    w.feed(Input::LinkHello {
        link,
        hello: Hello {
            protocol: 1,
            instance: instance.clone(),
            proof: token_proof(&token, &instance, 7),
            host_epoch: 7,
        },
    });
    w.pump();
    let result = w.complete(start);
    assert!(
        matches!(&result, OpResult::Err(e) if matches!(e.code, ErrorCode::RegistryFailed { .. })),
        "{result:?}"
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::StartInterrupted)
    );
    let worker = w.identity_of("s1");
    assert!(
        w.signals.contains(&(worker, GroupSignal::Kill)),
        "the worker is killed"
    );
    assert!(w.closed.contains(&link), "its link is closed");
}

/// Core LC-4: the startup deadline kills the worker and closes its link; the start ends `StartupTimeout`. A read of the
/// ended session then fails `WorkerLinkFailed` (A2-1): no worker can answer it, and it does not wait for a launch.
#[test]
fn the_startup_deadline_kills_the_worker_and_closes_its_link() {
    let mut w = World::new(limits(|l| l.startup = Duration::from_secs(2)));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.advance(Duration::from_secs(2));
    w.pump();
    assert!(matches!(w.complete(start), OpResult::Err(_)));
    let worker = w.identity_of("s1");
    assert!(w.signals.contains(&(worker, GroupSignal::Kill)));
    assert!(w.closed.contains(&link));
    let read = w
        .engine
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    assert!(matches!(
        w.complete(read),
        OpResult::Err(CoreError {
            code: ErrorCode::WorkerLinkFailed,
            ..
        })
    ));
}

/// Core R-16, LC-12: a plain `Stop` and a `StopAll` on one session whose Stopping row fails: the `Stop` fails
/// `RegistryFailed`, and the `StopAll` still stops the session and completes when it ends.
#[test]
fn a_stop_and_a_stop_all_share_a_failed_stopping_row() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let all = w.engine.begin(Op::StopAll).unwrap();
    w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
    w.pump();
    let events = w.engine.poll_events(64);
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Err(err) }
            if *op == stop && matches!(err.code, ErrorCode::RegistryFailed { .. }))
        ),
        "{events:?}"
    );
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the StopAll stops it"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    assert_eq!(w.complete(all), OpResult::Ok(OpOutput::Unit));
}

/// Core LC-7, A2-1: a `Remove` whose row delete fails completes `RegistryFailed` and leaves the session as it was, so that
/// the remove can be tried again, and the retry removes it.
#[test]
fn a_failed_row_delete_leaves_the_session_and_the_remove_can_be_tried_again() {
    for name in ["created", "exited", "lost"] {
        let mut w = World::default();
        match name {
            "created" => {
                w.ok(create("s1"));
            }
            "exited" => {
                w.running("s1");
                w.ok(Op::Stop { id: sid("s1") });
            }
            _ => {
                w.running("s1");
                w.exited("s1");
                w.pump();
            }
        }
        w.engine.poll_events(64);
        let before = w.engine.get(&sid("s1")).unwrap();
        // The first registry write of a remove is the delete of the row (LC-7 step 4).
        w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
        let result = w.run(Op::Remove { id: sid("s1") });
        assert!(
            matches!(&result, OpResult::Err(e) if e.code == ErrorCode::RegistryFailed { uncertain: false }),
            "{name}: {result:?}"
        );
        assert_eq!(
            w.engine.get(&sid("s1")).unwrap(),
            before,
            "{name}: the session stays"
        );
        assert!(
            matches!(
                w.ok(Op::Remove { id: sid("s1") }),
                OpOutput::RemoveReport(_)
            ),
            "{name}: the retry"
        );
        assert_eq!(
            w.engine.get(&sid("s1")).unwrap_err().code,
            ErrorCode::UnknownSession,
            "{name}"
        );
    }
}

/// Core A2-1 (`Stop`: `RegistryFailed` is its only asynchronous error), AD-7: a `Stop` that waits for a start whose `Starting`
/// row cannot be written completes with the same registry failure as the `Start`, certain or uncertain as the write was.
#[test]
fn a_stop_that_waited_for_a_failed_start_row_has_the_registry_failure() {
    for error in [
        botster_core_edges::edges::StorageError::Failed { errno: 5 },
        botster_core_edges::edges::StorageError::Uncertain { errno: 5 },
    ] {
        let mut w = World::default();
        w.ok(create("s1"));
        let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
        let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
        w.fail_row = Some(error);
        let expected = ErrorCode::RegistryFailed {
            uncertain: matches!(
                error,
                botster_core_edges::edges::StorageError::Uncertain { .. }
            ),
        };
        let results = w.complete_all(&[start, stop]);
        for op in [start, stop] {
            match &results[&op] {
                OpResult::Err(e) => assert_eq!(e.code, expected, "{error:?}"),
                other => panic!("{error:?}: {other:?}"),
            }
        }
    }
}

/// Core OR-2, EV-5, plan 2.5 rule 7: the engine does not take an observation while the start is not through, also while
/// `Running` waits for mandatory room; it takes it once the room is back and `Start` completed. The driver keeps the frame
/// unread on its link meanwhile (`driver::observations`).
#[test]
fn an_observation_is_not_taken_while_the_start_is_not_through() {
    let mut w = World::new(limits(|l| l.mandatory_events = 1));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    let bell = Input::LinkMsg {
        link,
        msg: observation(Observation::Bell),
    };
    assert!(!w.engine.can_accept(&bell), "the start is not through");
    w.engine.poll_events(64);
    // The `Created` of another session fills the mandatory queue (one event; completions use their op's slot, EV-5a), so
    // `Running` waits for room after the launch.
    w.engine.begin(create("x")).unwrap();
    w.pump();
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::new(),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.pump();
    assert!(!w.engine.can_accept(&bell), "`Running` waits for room");
    w.complete(start);
    assert!(w.engine.can_accept(&bell), "the start is through");
}

/// Core DP-7, OU-2, EV-5(b): a failed handoff closes its route once with `HandoffFailed`, posts nothing for a route that is
/// gone, and a close that finds no room waits and posts after a poll, once.
#[test]
fn a_failed_handoff_closes_a_known_route_once_and_ignores_an_unknown_one() {
    let mut w = World::new(limits(|l| l.mandatory_events = 2));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.engine.poll_events(64);
    w.feed(Input::HandoffFailed { route });
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, reason: RouteCloseReason::HandoffFailed, .. }] if *r == route),
        "{events:?}"
    );
    w.feed(Input::HandoffFailed { route });
    w.feed(Input::HandoffFailed { route: RouteId(99) });
    w.pump();
    assert_eq!(
        w.engine.poll_events(64),
        vec![],
        "a route that is gone posts nothing"
    );
    // With no room the close waits; after a poll it posts, once.
    let route = super::losses::attach(&mut w);
    w.engine.begin(create("x1")).unwrap();
    w.engine.begin(create("x2")).unwrap();
    w.pump();
    w.feed(Input::HandoffFailed { route });
    w.pump();
    let closes = |events: &[Event]| {
        events
            .iter()
            .filter(|e| matches!(e, Event::RouteClosed { route: r, reason: RouteCloseReason::HandoffFailed, .. } if *r == route))
            .count()
    };
    // The queue was full when the handoff failed: the first poll has the creates' events, not the close.
    let first = w.engine.poll_events(64);
    assert_eq!(closes(&first), 0, "the close waits for room: {first:?}");
    w.pump();
    let mut later = Vec::new();
    for _ in 0..4 {
        later.extend(w.engine.poll_events(64));
        w.pump();
    }
    assert_eq!(closes(&later), 1, "{later:?}");
}

/// Core LC-7, A6-3: a link that closes while the worker tears down leaves the uploads `OutcomeUnknown`. The worker may still
/// run, so the removal waits for its end (LC-7 step 3) before it completes.
#[test]
fn a_link_closing_during_the_remove_teardown_leaves_the_uploads_unknown() {
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
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)),
        "the worker was asked for its teardown"
    );
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    w.pump();
    assert!(
        !w.engine
            .poll_events(64)
            .iter()
            .any(|e| matches!(e, Event::Completed { op, .. } if *op == remove)),
        "the removal waits for the worker to end"
    );
    assert_eq!(w.engine.list().len(), 1, "the session is not removed yet");
    w.exited("s1");
    match w.complete(remove) {
        OpResult::Ok(OpOutput::RemoveReport(report)) => {
            assert_eq!(
                report.uploads,
                UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
            );
        }
        other => panic!("{other:?}"),
    }
}

/// Core LC-7, A6-3: the worker writes its cleanup result and closes its link before it ends, but the host can see the exit
/// first. The result that it reads after the exit still decides the uploads; with no result before the link's end of file,
/// they are `OutcomeUnknown`. Either way the removal completes.
#[test]
fn a_worker_exit_seen_before_its_remove_result_keeps_the_result() {
    for result in [Some(UploadsOutcome::Deleted), None] {
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
        let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
        w.pump();
        assert!(
            w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)),
            "the worker was asked for its teardown"
        );
        let link = w.link_of("s1");
        w.exited("s1");
        if let Some(uploads) = result.clone() {
            w.worker_says("s1", WorkerMsg::RemoveResult { uploads });
        }
        w.feed(Input::LinkClosed { link });
        let expected = result.unwrap_or(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown));
        match w.complete(remove) {
            OpResult::Ok(OpOutput::RemoveReport(report)) => {
                assert_eq!(report.uploads, expected);
            }
            other => panic!("{other:?}"),
        }
        assert!(w.engine.list().is_empty(), "the session is removed");
    }
}

/// Core AM-3, IN-7, A2-1, A5-2: the scheduler may run a session's work before an op's (OR-3), so a `Remove` can retire ops
/// that never ran. Each keeps the result of its own row: a write that was never forwarded is `NotWritten(SessionEnded)`, and
/// a `Detach` whose route the `Remove` closed completes with unit.
#[test]
fn ops_that_never_ran_before_the_remove_retired_them_keep_their_own_results() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    let write = w
        .engine
        .begin(Op::WriteInput {
            session: sid("s1"),
            payload: InputPayload::Text { text: "x".into() },
            guard: None,
        })
        .unwrap();
    let detach = w
        .engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .unwrap();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    // The session's work runs first, so the remove retires the write and the detach before either runs.
    w.pump_last_first();
    let events = w.engine.poll_events(64);
    let result = |op: OpId| {
        events.iter().find_map(|e| match e {
            Event::Completed { op: o, result } if *o == op => Some(result.clone()),
            _ => None,
        })
    };
    assert!(
        matches!(
            result(write),
            Some(OpResult::Ok(OpOutput::Input(InputResult {
                outcome: WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
                payload_bytes_written: 0,
                pty_bytes_written: 0,
                ..
            })))
        ),
        "{events:?}"
    );
    assert_eq!(
        result(detach),
        Some(OpResult::Ok(OpOutput::Unit)),
        "{events:?}"
    );
}

/// Core LC-4, AD-2: a link that closes while `Running` is still being posted does not fail the start, and a worker exit that
/// comes after a failed start does not change how it failed.
#[test]
fn a_closed_link_after_the_launch_and_an_exit_after_a_failure_change_nothing() {
    // The launch is through, `Running` waits for room, the link closes: the session is `Running`.
    let mut w = World::new(limits(|l| l.mandatory_events = 2));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.engine.begin(create("x")).unwrap();
    w.pump();
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::new(),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.pump();
    w.feed(Input::LinkClosed { link });
    let mut events = Vec::new();
    for _ in 0..10 {
        events.extend(w.engine.poll_events(64));
        w.pump();
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == start)),
        "the start completed: {events:?}"
    );
    // The link closes before the launch: the start fails `Exited`; a worker exit while that failure waits for room does not
    // turn it into `Lost`.
    let mut w = World::new(limits(|l| l.mandatory_events = 2));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.engine.begin(create("x")).unwrap();
    w.pump();
    w.feed(Input::LinkClosed { link });
    w.pump();
    w.exited("s1");
    for _ in 0..10 {
        w.engine.poll_events(64);
        w.pump();
    }
    assert!(
        matches!(
            w.engine.get(&sid("s1")).unwrap().state,
            SessionState::Exited(_)
        ),
        "{:?}",
        w.engine.get(&sid("s1")).unwrap().state
    );
}

/// Core LC-5: with the link alive, the kill after `stop_grace` goes to the worker as a message and the host signals nothing;
/// the final state row is written when the session ends.
#[test]
fn the_grace_kill_goes_over_the_link_and_the_end_is_written_to_the_registry() {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Kill)),
        "the kill is a message"
    );
    assert!(
        w.signals.is_empty(),
        "the host signals no process while the link is alive"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    let row = crate::session::Row::decode(&sid("s1"), &w.rows["session/s1"])
        .expect("Core decodes its row");
    assert!(matches!(row.state, SessionState::Exited(_)), "{row:?}");
}

/// Core DP-7: a local `Detach` closes the route with the reason that was asked for.
#[test]
fn a_local_detach_closes_with_the_reason_asked_for() {
    for (asked, expected) in [
        (DetachReason::Detached, RouteCloseReason::Detached),
        (DetachReason::Replaced, RouteCloseReason::Replaced),
        (DetachReason::Revoked, RouteCloseReason::Revoked),
    ] {
        let mut w = World::default();
        w.autopilot = Autopilot::Silent;
        w.running("s1");
        let route = w
            .engine
            .attach(
                ClientId("c".into()),
                sid("s1"),
                RouteTransport::Stream(StreamEndpoint::new(())),
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
                },
            )
            .unwrap()
            .route;
        w.engine.poll_events(64);
        let link = w.link_of("s1");
        w.feed(Input::LinkClosed { link });
        let detach = w
            .engine
            .begin(Op::Detach {
                route,
                reason: asked,
            })
            .unwrap();
        let mut closed = None;
        for _ in 0..6 {
            w.pump();
            for e in w.engine.poll_events(64) {
                if let Event::RouteClosed { reason, .. } = e {
                    closed = Some(reason);
                }
            }
        }
        let _ = detach;
        assert_eq!(closed, Some(expected));
    }
}

/// Core AM-3, A2-1: ops that are still pending when a session is removed end by their own row: a setter of a `Created` session
/// that never ran ends `RegistryFailed` for a policy and `SessionEnded` for a size or a size policy.
#[test]
fn pending_setters_of_a_removed_created_session_end_by_their_own_row() {
    let mut w = World::default();
    w.ok(create("s1"));
    let resize = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 9,
                cols: 9,
                cell_px: None,
            },
        })
        .unwrap();
    let policy = w
        .engine
        .begin(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        })
        .unwrap();
    let size_policy = w
        .engine
        .begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Latest,
        })
        .unwrap();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    // The flow of the remove runs before the setters do.
    w.pump_last_first();
    let mut done = BTreeMap::new();
    for _ in 0..10 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result } = e {
                done.insert(op, result);
            }
        }
    }
    let code = |op: OpId| match done.get(&op) {
        Some(OpResult::Err(e)) => Some(e.code.clone()),
        _ => None,
    };
    assert_eq!(code(resize), Some(ErrorCode::SessionEnded), "{done:?}");
    assert_eq!(code(size_policy), Some(ErrorCode::SessionEnded));
    assert_eq!(
        code(policy),
        Some(ErrorCode::RegistryFailed { uncertain: false })
    );
}

/// Core LC-12: a `StopAll` that finds the end of a payload in flight waits for it and does not send a stop.
#[test]
fn stop_all_joins_an_end_in_flight() {
    let mut w = World::new(limits(|l| l.mandatory_events = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.begin(create("x")).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(3),
            signal: None,
        },
    );
    w.pump();
    let all = w.engine.begin(Op::StopAll).unwrap();
    assert_eq!(w.complete(all), OpResult::Ok(OpOutput::Unit), "LC-12");
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "no stop for a payload that ended"
    );
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}

/// Core AM-1, AM-3: a failed `Create` ends the ops that were admitted after it, each with the registry failure, and none
/// stays attached to the session that never existed: a `Start`, and a `Remove`.
#[test]
fn a_failed_create_completes_the_ops_admitted_after_it() {
    // Both wait behind the create's flow, so the create's row is the first registry write (AM-1).
    for after in [Op::Start { id: sid("s1") }, Op::Remove { id: sid("s1") }] {
        let mut w = World::default();
        w.fail_row = Some(StorageError::Failed { errno: 5 });
        let ops = [
            w.engine.begin(create("s1")).unwrap(),
            w.engine.begin(after).unwrap(),
        ];
        let results = w.complete_all(&ops);
        for op in &ops {
            assert!(
                matches!(&results[op], OpResult::Err(e) if e.code == ErrorCode::RegistryFailed { uncertain: false }),
                "{op:?}: {results:?}"
            );
        }
        assert_eq!(
            w.engine.get(&sid("s1")).unwrap_err().code,
            ErrorCode::UnknownSession
        );
    }
}

/// Core AM-1, AM-3, LC-3: a row write of an op admitted after `Create` (`UpdateMetadata`, and `SetNotificationPolicy` in
/// `Created`) waits for the create's own row. When the create's write fails, the session never existed: each op ends with
/// the registry failure, and no row of the session is left.
#[test]
fn a_row_write_admitted_after_a_failed_create_leaves_no_row() {
    let later = [
        Op::UpdateMetadata {
            id: sid("s1"),
            labels: BTreeMap::from([("k".to_string(), "v".to_string())]),
        },
        Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        },
    ];
    for after in later {
        let mut w = World::default();
        w.fail_row = Some(StorageError::Failed { errno: 5 });
        let ops = [
            w.engine.begin(create("s1")).unwrap(),
            w.engine.begin(after).unwrap(),
        ];
        let results = w.complete_all(&ops);
        for op in &ops {
            assert!(
                matches!(&results[op], OpResult::Err(e) if e.code == ErrorCode::RegistryFailed { uncertain: false }),
                "{op:?}: {results:?}"
            );
        }
        assert!(
            w.rows.is_empty(),
            "no row of a session that never existed: {:?}",
            w.rows.keys()
        );
    }
}

/// Core LC-5, AD-6: with the link gone, a stop signals only the verified worker (pid and start time), never a bare
/// payload group; the session ends `Lost`.
#[test]
fn a_broken_link_stop_signals_the_verified_worker_only() {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let worker = w.identity_of("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let op = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert!(w.signals.contains(&(worker, GroupSignal::EndPayload)));
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(w.signals.contains(&(worker, GroupSignal::EndPayload)));
    assert!(
        w.signals.iter().all(|(_, s)| *s == GroupSignal::EndPayload),
        "the worker is never killed: it keeps the final model (LC-5)"
    );
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

/// Core LC-5, R-16: a plain `Stop` keeps `RegistryFailed` when its row write fails, and the session stays `Running`.
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

/// Core AD-7, LC-10 (audit A48): the final row of a session that ended is written with no operation waiting for it. A write
/// that fails changes nothing that the session reached (the stop completes with the exit), the registry keeps the earlier
/// row, and the failure is recorded: `diagnostics()` counts it, since no completion can carry it. A final row that is
/// written is not counted.
#[test]
fn a_failed_final_row_is_counted_and_changes_no_end() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    let stopping = crate::session::Row::decode(&sid("s1"), &w.rows["session/s1"])
        .expect("Core decodes its row");
    assert_eq!(stopping.state, SessionState::Stopping);
    w.fail_row = Some(StorageError::Failed { errno: 28 });
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    assert!(matches!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Exited(_)))
    ));
    let kept = crate::session::Row::decode(&sid("s1"), &w.rows["session/s1"])
        .expect("Core decodes its row");
    assert_eq!(
        kept.state,
        SessionState::Stopping,
        "the failed write left the row"
    );
    assert_eq!(w.engine.diagnostics()["final_row_failures"], 1);
    w.running("s2");
    let stop = w.engine.begin(Op::Stop { id: sid("s2") }).unwrap();
    w.pump();
    w.worker_says(
        "s2",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.complete(stop);
    let written = crate::session::Row::decode(&sid("s2"), &w.rows["session/s2"])
        .expect("Core decodes its row");
    assert!(
        matches!(written.state, SessionState::Exited(_)),
        "{written:?}"
    );
    assert_eq!(
        w.engine.diagnostics()["final_row_failures"],
        1,
        "a written final row is not counted"
    );
}
