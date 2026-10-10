//! The hello, the routes, the captures and the losses of a worker: what each input does and does not change (Core AD-6, DP-2,
//! DP-7, ST-6, LC-5, AD-2).

use super::*;
use botster_core_edges::edges::GroupSignal;

pub(super) fn attach(w: &mut World) -> RouteId {
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
}

/// A start that waits for the launch: the worker said hello by hand and says nothing more.
fn starting(w: &mut World) -> (OpId, LinkId) {
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    (start, link)
}

/// Core AD-6: a hello is accepted only while the start waits for it. A hello with the right proof after the link closed is
/// refused, and nothing is answered.
#[test]
fn a_hello_after_the_link_closed_is_refused() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let instance = w.instance_of("s1");
    let token = w.token_of("s1");
    w.feed(Input::LinkHello {
        link: LinkId(99),
        hello: Hello {
            protocol: 1,
            instance: instance.clone(),
            proof: token_proof(&token, &instance, 7),
            host_epoch: 7,
        },
    });
    assert!(w.closed.contains(&LinkId(99)), "the late link is closed");
    assert!(w.hellos.iter().all(|(l, _)| *l != LinkId(99)));
}

/// Core DP-7, EV-5(b): a stall, a resume and a close of a route become events, in order; an unknown route changes nothing.
/// With no room in the mandatory queue the event waits, and it is posted when the poll frees room.
#[test]
fn route_reports_become_events_and_wait_for_room() {
    let mut w = World::new(limits(|l| l.mandatory_events = 8));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = attach(&mut w);
    w.engine.poll_events(64);
    w.worker_says("s1", WorkerMsg::RouteStalled { route });
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteStalled { route: r }] if *r == route),
        "{events:?}"
    );
    w.worker_says("s1", WorkerMsg::RouteResumed { route });
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteResumed { route: r }] if *r == route),
        "{events:?}"
    );
    w.worker_says("s1", WorkerMsg::RouteStalled { route: RouteId(99) });
    w.worker_says("s1", WorkerMsg::RouteResumed { route: RouteId(99) });
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route: RouteId(99),
            reason: RouteCloseReason::Detached,
            route_tag: None,
        },
    );
    assert!(
        w.engine.poll_events(64).is_empty(),
        "an unknown route posts nothing"
    );
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Detached,
            route_tag: None,
        },
    );
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, .. }] if *r == route),
        "{events:?}"
    );
    assert!(!w.engine.routes.contains_key(&route));
}

/// Core EV-5(b), EV-5(d), EV-6: with the mandatory queue full, the route events that do not fit wait; a pump with no room
/// posts none of them; after a poll they are posted, each exactly once, in the worker's order.
#[test]
fn a_route_event_waits_in_a_full_queue_and_posts_after_a_poll() {
    let mut w = World::new(limits(|l| l.mandatory_events = 3));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = attach(&mut w);
    w.engine.poll_events(64);
    // Three route events fill the queue; the next two wait.
    for _ in 0..3 {
        w.worker_says("s1", WorkerMsg::RouteStalled { route });
    }
    w.worker_says("s1", WorkerMsg::RouteResumed { route });
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Detached,
            route_tag: None,
        },
    );
    w.pump();
    let first = w.engine.poll_events(64);
    assert!(
        first.len() == 3
            && first
                .iter()
                .all(|e| matches!(e, Event::RouteStalled { .. })),
        "{first:?}"
    );
    for _ in 0..4 {
        w.pump();
    }
    let rest = w.engine.poll_events(64);
    assert!(
        matches!(
            &rest[..],
            [Event::RouteResumed { .. }, Event::RouteClosed { .. }]
        ),
        "{rest:?}"
    );
    w.pump();
    assert_eq!(
        w.engine.poll_events(64),
        vec![],
        "each event is posted once"
    );
}

fn page(index: u32, bytes: usize) -> Page {
    Page {
        index,
        last: true,
        bytes: botster_route_codec::prelude::HexBytes(vec![0; bytes]),
    }
}

/// Core ST-6: a capture is kept when its pages total at most `max_snapshot_bytes`, and `SnapshotTooLarge` over it; each
/// capture has its own id. A `Capture` result of an op that is not a capture passes through unchanged.
#[test]
fn a_capture_is_bounded_by_the_snapshot_bytes_and_has_its_own_id() {
    let mut w = World::new(limits(|l| l.max_snapshot_bytes = 100));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let capture = |w: &mut World, bytes: usize| {
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
                pages: vec![page(0, bytes)],
            },
        );
        w.worker_says(
            "s1",
            WorkerMsg::Done {
                req,
                result: OpResult::Ok(OpOutput::Capture(Capture {
                    capture: CaptureId(0),
                    page_count: 1,
                    total_bytes: bytes as u64,
                    model_rev: ModelRev(5),
                })),
            },
        );
        w.complete(op)
    };
    let first = capture(&mut w, 100);
    let second = capture(&mut w, 60);
    let (OpResult::Ok(OpOutput::Capture(a)), OpResult::Ok(OpOutput::Capture(b))) = (first, second)
    else {
        panic!("both captures are kept");
    };
    assert_eq!(a.total_bytes, 100);
    assert_eq!(b.total_bytes, 60);
    assert_eq!(b.capture.0, a.capture.0 + 1, "each capture has its own id");
    assert!(matches!(
        capture(&mut w, 101),
        OpResult::Err(e) if e.code == ErrorCode::SnapshotTooLarge
    ));
    // A read that answers with a `Capture` is not a capture.
    let read = w
        .engine
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    w.pump();
    let req = w.last_request("s1");
    let odd = Capture {
        capture: CaptureId(77),
        page_count: 0,
        total_bytes: 0,
        model_rev: ModelRev(1),
    };
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Capture(odd.clone())),
        },
    );
    assert_eq!(w.complete(read), OpResult::Ok(OpOutput::Capture(odd)));
}

/// Core LC-4, AD-2: a link that closes while the start waits for the launch fails the start; a link that closes after the
/// session runs leaves its state.
#[test]
fn a_link_closing_before_the_launch_fails_the_start_and_after_it_changes_nothing() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    let (start, link) = starting(&mut w);
    w.feed(Input::LinkClosed { link });
    assert!(matches!(w.complete(start), OpResult::Err(_)));
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
}

/// Core LC-5: when the link closes during a stop, the host signals the verified worker, once; before the stop's row is
/// written, and after the end is known, it signals nothing at that time.
#[test]
fn a_link_closing_during_a_stop_signals_the_worker_only_while_the_stop_waits() {
    let signals = |w: &World| {
        w.signals
            .iter()
            .filter(|(_, s)| *s == GroupSignal::EndPayload)
            .count()
    };
    // The stop waits for the exit: the signal goes out at the close.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert_eq!(signals(&w), 0);
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    assert_eq!(signals(&w), 1);
    // The stop has not written its row yet: the signal follows the send of the stop, not the close.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    assert_eq!(signals(&w), 0, "the stop is still in its row write");
    // The end is known: the exit was reported, and the end waits for room.
    let mut w = World::new(limits(|l| l.mandatory_events = 3));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    assert_eq!(signals(&w), 0, "the payload ended already");
}

/// Core AD-2: the worker process ends while the start waits for the launch: the session is `Lost(WorkerGone)`. After an
/// `Exited` report the same exit changes nothing.
#[test]
fn a_worker_exit_is_lost_before_the_launch_and_nothing_after_an_exited_report() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    let (start, _link) = starting(&mut w);
    w.exited("s1");
    assert!(matches!(w.complete(start), OpResult::Err(_)));
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    ));
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
    let shown = w.engine.get(&sid("s1")).unwrap().state;
    assert!(matches!(shown, SessionState::Exited(_)));
    w.exited("s1");
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        shown,
        "an exit after the end changes nothing"
    );
}

/// Pumps and polls until nothing more comes, and returns the events in order.
fn settle(w: &mut World) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..32 {
        w.pump();
        events.extend(w.engine.poll_events(64));
    }
    events
}

/// A2-3, OU-2b: the routes of a lost session close `SessionLost`, each once and after the session's state. No worker is
/// left to close them, so the host does.
#[test]
fn a_lost_session_closes_its_routes_session_lost_after_its_state() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    settle(&mut w);
    let (one, two) = (attach(&mut w), attach(&mut w));
    settle(&mut w);
    w.exited("s1");
    let events = settle(&mut w);
    let lost = events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::SessionState {
                    state: SessionState::Lost(_),
                    ..
                }
            )
        })
        .expect("the session is lost");
    let closed: Vec<(usize, RouteId)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            Event::RouteClosed {
                route,
                reason: RouteCloseReason::SessionLost,
                ..
            } => Some((i, *route)),
            Event::RouteClosed { reason, .. } => panic!("another reason: {reason:?}"),
            _ => None,
        })
        .collect();
    assert_eq!(
        closed.iter().map(|c| c.1).collect::<Vec<_>>(),
        vec![one, two]
    );
    assert!(closed.iter().all(|(i, _)| *i > lost), "{events:?}");
}

/// The index of the session's `Exited` state in `events`.
fn exited_at(events: &[Event]) -> usize {
    events
        .iter()
        .position(|e| {
            matches!(
                e,
                Event::SessionState {
                    state: SessionState::Exited(_),
                    ..
                }
            )
        })
        .expect("the state")
}

fn route_closes(events: &[Event]) -> Vec<(usize, &Event)> {
    events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, Event::RouteClosed { .. }))
        .collect()
}

/// OU-7, LC-5 (#217 R1-3): after `Exited`, an ended session's route closes only when the worker reports that it delivered
/// its queue. Until then the route is held: no `RouteClosed`, and the stop flow waits without busy work. The worker's report
/// then gives exactly one close, `SessionEnded` with the exit that the host shows, after the state.
#[test]
fn an_ended_sessions_route_closes_after_the_worker_delivered_it_with_the_hosts_exit() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    settle(&mut w);
    let route = attach(&mut w);
    settle(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(4),
            signal: None,
        },
    );
    let events = settle(&mut w);
    exited_at(&events);
    assert!(
        route_closes(&events).is_empty(),
        "the route is held: {events:?}"
    );
    assert!(
        w.engine.ready().is_empty(),
        "the held close is no busy work"
    );
    let SessionState::Exited(exit) = w.engine.get(&sid("s1")).unwrap().state else {
        panic!("the session ended");
    };
    // The worker's report carries its own view of the exit; the host posts its own (LC-5).
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::SessionEnded {
                exit: Exit {
                    code: None,
                    signal: None,
                    cause: ExitCause::Other,
                },
            },
            route_tag: None,
        },
    );
    let events = settle(&mut w);
    let closes = route_closes(&events);
    assert_eq!(closes.len(), 1, "{events:?}");
    assert!(matches!(
        closes[0].1,
        Event::RouteClosed { route: r, reason: RouteCloseReason::SessionEnded { exit: e }, .. }
            if *r == route && *e == exit && e.code == Some(4)
    ));
}

/// OU-7: after `Exited`, each route closes at the worker's report for that route. When the worker delivered one route
/// and not the other, only the delivered one closes; the other stays held until its own report.
#[test]
fn after_the_exit_each_route_closes_at_its_own_delivery() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    settle(&mut w);
    let (one, two) = (attach(&mut w), attach(&mut w));
    settle(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(4),
            signal: None,
        },
    );
    settle(&mut w);
    let delivered = |route| WorkerMsg::RouteClosed {
        route,
        reason: RouteCloseReason::SessionEnded {
            exit: Exit {
                code: None,
                signal: None,
                cause: ExitCause::Other,
            },
        },
        route_tag: None,
    };
    w.worker_says("s1", delivered(two));
    let events = settle(&mut w);
    let closes = route_closes(&events);
    assert_eq!(closes.len(), 1, "{events:?}");
    assert!(matches!(
        closes[0].1,
        Event::RouteClosed { route: r, reason: RouteCloseReason::SessionEnded { .. }, .. } if *r == two
    ));
    assert!(
        w.engine.ready().is_empty(),
        "the held close is no busy work"
    );
    w.worker_says("s1", delivered(one));
    let events = settle(&mut w);
    let closes = route_closes(&events);
    assert_eq!(closes.len(), 1, "{events:?}");
    assert!(matches!(
        closes[0].1,
        Event::RouteClosed { route: r, reason: RouteCloseReason::SessionEnded { .. }, .. } if *r == one
    ));
}

/// OU-7, DP-7 (#217 R1-3): a `Detach` that waits across the payload's exit completes only when the worker reports the
/// route's close, and the close keeps the detach's reason (the first one).
#[test]
fn a_detach_that_waits_across_the_exit_completes_at_the_workers_close_with_its_reason() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    settle(&mut w);
    let route = attach(&mut w);
    settle(&mut w);
    let detach = w
        .engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Revoked,
        })
        .unwrap();
    settle(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(4),
            signal: None,
        },
    );
    let events = settle(&mut w);
    exited_at(&events);
    assert!(route_closes(&events).is_empty(), "{events:?}");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, .. } if *op == detach)),
        "the detach waits for the worker: {events:?}"
    );
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Revoked,
            route_tag: None,
        },
    );
    let events = settle(&mut w);
    let closes = route_closes(&events);
    assert_eq!(closes.len(), 1, "{events:?}");
    assert!(matches!(
        closes[0].1,
        Event::RouteClosed { route: r, reason: RouteCloseReason::Revoked, .. } if *r == route
    ));
    let completed = events
        .iter()
        .position(|e| matches!(e, Event::Completed { op, .. } if *op == detach))
        .expect("the detach completes");
    assert!(completed > closes[0].0, "{events:?}");
}

/// OU-7: when the worker's link is lost after the exit, a route that the worker did not report closed closes
/// `SessionLost`, once.
#[test]
fn a_held_route_closes_session_lost_when_the_link_is_lost_after_the_exit() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    settle(&mut w);
    let route = attach(&mut w);
    settle(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(4),
            signal: None,
        },
    );
    let events = settle(&mut w);
    assert!(route_closes(&events).is_empty(), "{events:?}");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let events = settle(&mut w);
    let closes = route_closes(&events);
    assert_eq!(closes.len(), 1, "{events:?}");
    assert!(matches!(
        closes[0].1,
        Event::RouteClosed { route: r, reason: RouteCloseReason::SessionLost, .. } if *r == route
    ));
}
