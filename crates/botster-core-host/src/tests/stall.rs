//! The host's stall clock of a route (steward ruling R-90 and its follow-up): the host arms `stall_deadline` from the pump
//! that consumes a waiting progress report, sends the due step with its mark, holds one mandatory slot for the answer, and
//! arms `stall_close_after` from the pump that consumes `RouteStalled`. A due step with no mandatory room is parked (E3-1
//! item 4).

use super::*;
use crate::run::DeadlineKind;

const STALL: Duration = Duration::from_secs(1);
const CLOSE_AFTER: Duration = Duration::from_secs(5);

fn world(mandatory: u32) -> World {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = mandatory;
        l.max_sessions = 4;
        l.stall_close_after = CLOSE_AFTER;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    w
}

/// A route of `s1` with a `stall_deadline` of [`STALL`].
fn attach(w: &mut World) -> RouteId {
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
                owner: None,
                query_deadline: Some(Duration::from_secs(1)),
                route_tag: None,
                route_limits: None,
                history: None,
                stall_deadline: Some(STALL),
                answers_queries: true,
                input: true,
            },
        )
        .unwrap()
        .route;
    w.pump();
    w.engine.poll_events(64);
    route
}

fn progress(w: &mut World, route: RouteId, mark: u64, waiting: bool) {
    w.worker_says(
        "s1",
        WorkerMsg::RouteProgress {
            route,
            mark,
            waiting,
        },
    );
}

/// The instant of the route's due step, if one is armed and not parked.
fn due_at(w: &World, route: RouteId) -> Option<Instant> {
    w.engine
        .deadlines()
        .into_iter()
        .find(|(_, kind)| matches!(kind, DeadlineKind::Route(r) if *r == route))
        .map(|(at, _)| at)
}

/// The due messages that the host sent, in order.
fn dues(w: &World) -> Vec<HostMsg> {
    w.sent
        .iter()
        .filter(|(_, m)| {
            matches!(
                m,
                HostMsg::RouteStallDue { .. } | HostMsg::RouteCloseDue { .. }
            )
        })
        .map(|(_, m)| m.clone())
        .collect()
}

/// R-90: a waiting report arms `stall_deadline` from the pump that consumed it. A report of the same mark keeps that
/// instant; a new mark restarts it; `waiting: false` disarms it.
#[test]
fn a_waiting_report_arms_the_stall_deadline_and_a_new_mark_restarts_it() {
    let mut w = world(8);
    let route = attach(&mut w);
    assert_eq!(due_at(&w, route), None, "nothing waits");
    progress(&mut w, route, 3, true);
    let armed = w.now + STALL;
    assert_eq!(due_at(&w, route), Some(armed));
    w.advance(Duration::from_millis(500));
    progress(&mut w, route, 3, true);
    assert_eq!(
        due_at(&w, route),
        Some(armed),
        "the same mark is no progress"
    );
    progress(&mut w, route, 4, true);
    assert_eq!(
        due_at(&w, route),
        Some(w.now + STALL),
        "progress restarts it"
    );
    progress(&mut w, route, 5, false);
    assert_eq!(due_at(&w, route), None, "the queue is written");
}

/// R-90, Q1: the due step sends `RouteStallDue` with the host's mark and holds one mandatory slot. No host-visible fact
/// changes and no event is posted at the due step. When the worker declines (a newer report), the slot is released and
/// the deadline restarts from that report.
#[test]
fn the_due_step_holds_a_slot_and_a_newer_report_releases_it() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.advance(STALL - Duration::from_millis(1));
    w.pump();
    assert!(dues(&w).is_empty(), "not due yet");
    w.advance(Duration::from_millis(1));
    let report = w.pump();
    assert_eq!(dues(&w), [HostMsg::RouteStallDue { route, mark: 3 }]);
    assert_eq!(report.events_posted, 0, "the event is the worker's answer");
    assert!(w.engine.poll_events(64).is_empty());
    assert!(!w.engine.queue.has_mandatory_room(), "the slot is held");
    assert_eq!(due_at(&w, route), None, "the step waits for its answer");
    progress(&mut w, route, 4, true);
    assert!(
        w.engine.queue.has_mandatory_room(),
        "the decline releases it"
    );
    assert_eq!(due_at(&w, route), Some(w.now + STALL));
    assert!(
        w.engine.poll_events(64).is_empty(),
        "a decline posts nothing"
    );
}

/// R-90, OU-2: `RouteStalled` posts in the held slot and arms `stall_close_after` from the pump that consumed it; the close
/// step sends `RouteCloseDue` with the stall's mark, and the worker's `RouteClosed{StallTimeout}` ends the route.
#[test]
fn a_stall_arms_the_close_and_the_close_step_sends_the_stall_mark() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.advance(STALL);
    w.pump();
    w.worker_says("s1", WorkerMsg::RouteStalled { route, mark: 3 });
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteStalled { route: r }] if *r == route),
        "{events:?}"
    );
    assert!(
        w.engine.queue.has_mandatory_room(),
        "the slot became the event"
    );
    assert_eq!(due_at(&w, route), Some(w.now + CLOSE_AFTER));
    w.advance(CLOSE_AFTER);
    w.pump();
    assert_eq!(
        dues(&w),
        [
            HostMsg::RouteStallDue { route, mark: 3 },
            HostMsg::RouteCloseDue { route, mark: 3 }
        ]
    );
    assert!(!w.engine.queue.has_mandatory_room());
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::StallTimeout,
            route_tag: None,
        },
    );
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, reason: RouteCloseReason::StallTimeout, .. }] if *r == route),
        "{events:?}"
    );
    assert!(!w.engine.routes.contains_key(&route));
    assert!(w.engine.queue.has_mandatory_room(), "the close released it");
}

/// OU-2: a resume ends the stall: the close step is disarmed, and a close step that was sent releases its slot.
#[test]
fn a_resume_disarms_the_close() {
    let mut w = world(2);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.advance(STALL);
    w.pump();
    w.worker_says("s1", WorkerMsg::RouteStalled { route, mark: 3 });
    w.engine.poll_events(64);
    w.worker_says("s1", WorkerMsg::RouteResumed { route, mark: 4 });
    w.engine.poll_events(64);
    assert_eq!(due_at(&w, route), None);
    w.advance(CLOSE_AFTER);
    w.pump();
    assert_eq!(dues(&w), [HostMsg::RouteStallDue { route, mark: 3 }]);
    // A resume that answers a close step that was sent releases the slot.
    progress(&mut w, route, 4, true);
    w.advance(STALL);
    w.pump();
    w.worker_says("s1", WorkerMsg::RouteStalled { route, mark: 4 });
    w.engine.poll_events(64);
    w.advance(CLOSE_AFTER);
    w.pump();
    assert!(matches!(
        dues(&w).last(),
        Some(HostMsg::RouteCloseDue { mark: 4, .. })
    ));
    w.worker_says("s1", WorkerMsg::RouteResumed { route, mark: 5 });
    w.engine.poll_events(64);
    assert!(w.engine.queue.has_mandatory_room());
    assert_eq!(due_at(&w, route), None);
}

/// E3-1 item 4, TM-6: a due step that runs and finds no mandatory room is parked: it sends nothing, its instant leaves the
/// deadlines, and the pump reports no `more`. A poll that frees room makes it due again, and the next pump sends it.
#[test]
fn a_due_step_with_no_room_is_parked_until_a_poll_frees_room() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    // `Created` of another session fills the queue of one.
    w.engine.begin(create("x1")).unwrap();
    w.pump();
    w.advance(STALL);
    let report = w.pump();
    assert!(dues(&w).is_empty(), "no room: nothing is sent");
    assert!(!report.more, "a parked step is not runnable work");
    assert_eq!(due_at(&w, route), None, "a parked step has no deadline");
    w.engine.poll_events(64);
    assert_eq!(due_at(&w, route), Some(w.now), "the poll freed room");
    w.pump();
    assert_eq!(dues(&w), [HostMsg::RouteStallDue { route, mark: 3 }]);
}
