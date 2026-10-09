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
