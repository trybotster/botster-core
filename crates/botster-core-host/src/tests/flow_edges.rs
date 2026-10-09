//! The edges of the flows: a start that fails each way, a stop whose row fails, and a remove whose row delete fails (Core
//! AD-7, LC-4, LC-7, LC-12, R-16, A2-1).

use super::*;
use crate::flow::{Flow, RemovePhase};
use crate::session::Admit;
use botster_core_edges::edges::GroupSignal;

/// Core LC-4, A2-1: a launch that the worker refuses ends the start `Exited`, with the exit recorded and the admission state
/// `Exited`; a stop that waited for the start does not run on the failed session.
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
    let s = &w.engine.sessions[&sid("s1")];
    assert_eq!(s.admit, Admit::Exited);
    assert!(s.exit.is_some(), "the exit is recorded");
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

/// Core LC-4: the startup deadline kills the worker and closes its link; the start ends `StartupTimeout`.
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

/// Core LC-7, A2-1: a `Remove` whose row delete fails leaves the session as it was, in the admission state of its shown
/// state, so that the remove can be tried again.
#[test]
fn a_failed_row_delete_restores_the_admission_state() {
    for (name, shown, admit) in [
        ("created", None, Admit::Created),
        ("exited", Some(true), Admit::Exited),
        ("lost", Some(false), Admit::Lost),
    ] {
        let mut w = World::default();
        match shown {
            None => {
                w.ok(create("s1"));
            }
            Some(exit) => {
                w.autopilot = Autopilot::Silent;
                w.running("s1");
                if exit {
                    w.worker_says(
                        "s1",
                        WorkerMsg::Exited {
                            code: Some(0),
                            signal: None,
                        },
                    );
                } else {
                    w.exited("s1");
                }
                w.pump();
                w.engine.poll_events(64);
            }
        }
        let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
        for _ in 0..30 {
            let phase = |w: &World| match &w.engine.sessions[&sid("s1")].flow {
                Flow::Remove(f) => Some(f.phase),
                _ => None,
            };
            if phase(&w) == Some(RemovePhase::AwaitTeardown) {
                if w.engine.sessions[&sid("s1")].worker.link.is_some() {
                    w.worker_says(
                        "s1",
                        WorkerMsg::RemoveResult {
                            uploads: UploadsOutcome::Deleted,
                        },
                    );
                }
                if !w.engine.sessions[&sid("s1")].worker.gone {
                    w.exited("s1");
                }
            }
            if phase(&w) == Some(RemovePhase::DeleteRow) {
                break;
            }
            if !w.step() {
                break;
            }
        }
        w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
        let result = w.complete(remove);
        assert!(
            matches!(&result, OpResult::Err(e) if matches!(e.code, ErrorCode::RegistryFailed { .. })),
            "{name}: {result:?}"
        );
        assert_eq!(w.engine.sessions[&sid("s1")].admit, admit, "{name}");
        assert!(
            w.engine.get(&sid("s1")).is_ok(),
            "{name}: the session stays"
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
        let mut results = BTreeMap::new();
        for _ in 0..10 {
            w.pump();
            for event in w.engine.poll_events(64) {
                if let Event::Completed { op, result } = event {
                    results.insert(op, result);
                }
            }
        }
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
    use crate::flow::Flow;
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
    assert!(matches!(w.engine.sessions[&sid("s1")].flow, Flow::Start(_)));
    assert!(!w.engine.can_accept(&bell), "`Running` waits for room");
    w.complete(start);
    assert!(w.engine.can_accept(&bell), "the start is through");
}

/// Core DP-7, EV-5(b): a failed handoff closes its route once, posts nothing for a route that is gone, and parks only a close
/// that found no room.
#[test]
fn a_failed_handoff_closes_a_known_route_once_and_ignores_an_unknown_one() {
    let attach = |w: &mut World| {
        w.engine
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
            .route
    };
    let mut w = World::new(limits(|l| l.mandatory_events = 2));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = attach(&mut w);
    w.engine.poll_events(64);
    w.feed(Input::HandoffFailed { route });
    let events = w.engine.poll_events(64);
    assert!(
        matches!(
            &events[..],
            [Event::RouteClosed {
                reason: RouteCloseReason::HandoffFailed,
                ..
            }]
        ),
        "{events:?}"
    );
    assert!(!w.engine.parked_work(), "a close that posted is not parked");
    w.feed(Input::HandoffFailed { route });
    w.feed(Input::HandoffFailed { route: RouteId(99) });
    assert!(
        w.engine.poll_events(64).is_empty(),
        "a route that is gone posts nothing"
    );
    assert!(!w.engine.parked_work());
    // With no room the close is parked.
    let route = attach(&mut w);
    w.engine.begin(create("x1")).unwrap();
    w.engine.begin(create("x2")).unwrap();
    w.pump();
    assert!(!w.engine.has_room());
    w.feed(Input::HandoffFailed { route });
    assert!(w.engine.parked_work(), "no room: parked");
}

/// Core LC-7, A6-3: a link that closes while the worker tears down leaves the uploads `OutcomeUnknown`.
#[test]
fn a_link_closing_during_the_remove_teardown_leaves_the_uploads_unknown() {
    use crate::flow::{Flow, RemovePhase};
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
    for _ in 0..10 {
        if matches!(&w.engine.sessions[&sid("s1")].flow, Flow::Remove(f) if f.phase == RemovePhase::AwaitTeardown)
        {
            break;
        }
        w.step();
    }
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
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
    // Only the session's work runs, until the teardown waits for the worker: the write and the detach never run.
    let mut guard = 0;
    while let Some(work) = w
        .engine
        .ready()
        .into_iter()
        .find(|work| matches!(work, Work::Session(_)))
    {
        w.feed(Input::Run(work));
        guard += 1;
        assert!(guard < 50);
    }
    assert!(matches!(
        &w.engine.sessions[&sid("s1")].flow,
        Flow::Remove(f) if f.phase == RemovePhase::AwaitTeardown
    ));
    w.pump();
    let events = w.engine.poll_events(64);
    let result = |op: OpId| {
        events.iter().find_map(|e| match e {
            Event::Completed { op: o, result } if *o == op => Some(result.clone()),
            _ => None,
        })
    };
    assert_eq!(
        result(write),
        Some(OpResult::Ok(OpOutput::Input(InputResult {
            outcome: WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
            payload_bytes_written: 0,
            pty_bytes_written: 0,
            detail: "the session ended".into(),
        }))),
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
    assert!(!w.engine.has_room(), "Running waits");
    w.feed(Input::LinkClosed { link });
    for _ in 0..10 {
        w.engine.poll_events(64);
        w.pump();
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    let _ = start;
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
    assert!(!w.engine.has_room());
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
    let row: serde_json::Value = serde_json::from_slice(&w.rows["session/s1"]).unwrap();
    assert!(row["state"].get("Exited").is_some(), "{row}");
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
    use crate::flow::{Flow, RemovePhase};
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
    for _ in 0..10 {
        if !matches!(&w.engine.sessions.get(&sid("s1")).map(|s| s.flow.clone()), Some(Flow::Remove(f)) if f.phase != RemovePhase::PostReleased)
        {
            break;
        }
        w.feed(Input::Run(crate::io::Work::Session(sid("s1"))));
    }
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

/// Core AD-7: a flow step that asks an edge for something waits for the answer: the session is not ready until the ticket is
/// resolved.
#[test]
fn a_flow_waits_for_the_ticket_of_its_edge_request() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    // One step by hand: the engine asks for random bytes, and the world has not answered.
    w.engine
        .on_input(Input::Run(crate::io::Work::Session(sid("s1"))));
    assert!(
        w.engine.sessions[&sid("s1")].ticket.is_some(),
        "the flow waits for its ticket"
    );
    assert!(
        !w.engine
            .ready()
            .contains(&crate::io::Work::Session(sid("s1"))),
        "a session with a ticket is not ready"
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
    assert!(!w.engine.has_room());
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(3),
            signal: None,
        },
    );
    w.pump();
    let all = w.engine.begin(Op::StopAll).unwrap();
    let mut result = None;
    for _ in 0..12 {
        w.engine.poll_events(64);
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result: r } = e {
                if op == all {
                    result = Some(r);
                }
            }
        }
        if !w.engine.ops.contains_key(&all) {
            break;
        }
    }
    let _ = result;
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "no stop for a payload that ended"
    );
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}
