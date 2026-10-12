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
    attach_to(w, "s1")
}

/// A route of `session` with a `stall_deadline` of [`STALL`].
fn attach_to(w: &mut World, session: &str) -> RouteId {
    let route = w
        .engine
        .attach(
            ClientId("c".into()),
            sid(session),
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
    w.worker_says(session, WorkerMsg::RouteReceived { route });
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

/// EV-5b, TM-6 (steward ruling R-87, `parked_work`): a due stall step that is parked on room is parked work of the route's
/// session, and work that is ready with room. A due step that did not run yet is ready now, not parked. A poll that frees
/// room ends both.
#[test]
fn a_parked_due_step_is_parked_work_of_the_routes_session() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.engine.begin(create("x1")).unwrap();
    w.pump();
    w.advance(STALL);
    w.feed(Input::Clock(w.unix));
    // Due, and not run yet: it is ready now, not parked.
    assert!(w.engine.ready_at(false).contains(&Work::DueStep));
    assert!(
        w.engine.parked_on_room([]).is_empty(),
        "a step that did not run is not parked"
    );
    w.pump();
    assert!(dues(&w).is_empty(), "the step is parked");
    assert!(!w.engine.ready_at(false).contains(&Work::DueStep));
    assert!(w.engine.ready_at(true).contains(&Work::DueStep));
    assert_eq!(w.engine.parked_on_room([]), BTreeSet::from([sid("s1")]));
    w.engine.poll_events(64);
    assert!(
        w.engine.parked_on_room([]).is_empty(),
        "with room, no work waits for it"
    );
}

/// E3-1 item 4, EV-5b, R-90 follow-up (contracts `main` `1798a65d`): the answer of a due step has the slot that the step
/// holds, and a mandatory frame that Core reads from the same link ahead of the answer may take that slot over. So the full
/// queue takes the answer and every mandatory frame of that worker's link, and no mandatory frame of another worker.
#[test]
fn the_full_queue_takes_the_answer_and_the_frames_ahead_of_it_on_its_link() {
    let mut w = world(1);
    w.running("s2");
    w.engine.poll_events(64);
    let route = attach(&mut w);
    let other = attach(&mut w);
    let elsewhere = attach_to(&mut w, "s2");
    progress(&mut w, route, 3, true);
    w.advance(STALL);
    w.pump();
    assert!(!w.engine.queue.has_mandatory_room(), "the slot is held");
    let takes = |w: &World, session: &str, msg| {
        w.engine.can_accept(&Input::LinkMsg {
            link: w.link_of(session),
            msg,
        })
    };
    assert!(takes(&w, "s1", WorkerMsg::RouteStalled { route, mark: 3 }));
    assert!(takes(
        &w,
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::StallTimeout,
            route_tag: None,
        }
    ));
    assert!(takes(&w, "s1", WorkerMsg::RouteResumed { route, mark: 4 }));
    assert!(takes(
        &w,
        "s1",
        WorkerMsg::RouteStalled {
            route: other,
            mark: 0
        }
    ));
    assert!(
        !takes(
            &w,
            "s2",
            WorkerMsg::RouteStalled {
                route: elsewhere,
                mark: 0
            }
        ),
        "another worker has no slot"
    );
}

/// OU-7, E3-1 item 4: a worker that answers a due step with the close of its ended session gives the slot back at once. The
/// route's close posts later, after the session's state (A2-3).
#[test]
fn a_session_ended_answer_gives_the_slot_back() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.advance(STALL);
    w.pump();
    assert!(!w.engine.queue.has_mandatory_room(), "the slot is held");
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
    assert!(w.engine.queue.has_mandatory_room());
}

/// E3-1 items 2 and 5: a due stop grace is an effect without an event (`Work::Deadline`), and it comes first. A due stall
/// step is atomic with its event (`Work::DueStep`), so it needs event budget.
#[test]
fn a_stall_step_is_atomic_and_a_kill_is_not() {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = 8;
        l.max_sessions = 4;
        l.stop_grace = STALL;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.running("s2");
    w.engine.poll_events(64);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.engine.begin(Op::Stop { id: sid("s2") }).unwrap();
    w.pump();
    w.engine.poll_events(64);
    w.advance(STALL);
    w.feed(Input::Clock(w.unix));
    assert_eq!(w.engine.ready()[..2], [Work::Deadline, Work::DueStep]);
}

/// TM-3, E3-1 items 2 and 4 (0181's ST298-R3): new progress replaces a parked step with a new one, which is not parked:
/// only a step that ran can be parked. With the queue still full, the new step's instant is the next deadline, and at it
/// the step runs and parks again.
#[test]
fn new_progress_replaces_a_parked_step_with_a_step_that_is_not_parked() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.engine.begin(create("x1")).unwrap();
    w.pump();
    w.advance(STALL);
    w.pump();
    assert_eq!(w.engine.deadline(), None, "the step is parked");
    progress(&mut w, route, 4, true);
    assert!(
        !w.engine.queue.has_mandatory_room(),
        "the queue is still full"
    );
    let next = w.now + STALL;
    assert_eq!(w.engine.deadline(), Some(next), "the new step");
    w.advance(STALL);
    w.pump();
    assert!(dues(&w).is_empty(), "no room: the new step parks");
    assert_eq!(w.engine.deadline(), None);
    w.engine.poll_events(64);
    w.pump();
    assert_eq!(dues(&w), [HostMsg::RouteStallDue { route, mark: 4 }]);
}

/// OU-2, OU-9 (0181's ST298-R2, host half): a route whose resume failed stays `Stalled` in the worker and sends no resume,
/// so the host keeps its close step. The worker answers that step with the close's own reason: the close posts with it in
/// the step's slot, and the queue has its room again.
#[test]
fn the_close_step_answer_of_a_failed_resume_posts_its_reason_in_the_slot() {
    let mut w = world(1);
    let route = attach(&mut w);
    progress(&mut w, route, 3, true);
    w.advance(STALL);
    w.pump();
    w.worker_says("s1", WorkerMsg::RouteStalled { route, mark: 3 });
    w.engine.poll_events(64);
    // The resume failed in the worker: no report comes.
    w.advance(CLOSE_AFTER);
    w.pump();
    assert!(matches!(
        dues(&w).last(),
        Some(HostMsg::RouteCloseDue { mark: 3, .. })
    ));
    assert!(!w.engine.queue.has_mandatory_room(), "the slot is held");
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None,
        },
    );
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, reason: RouteCloseReason::SnapshotTooLarge, .. }] if *r == route),
        "{events:?}"
    );
    assert!(w.engine.queue.has_mandatory_room());
}

/// ID-2, DP-8 (not built): a route that survived a host restart keeps its id in the worker, and this host numbers its routes
/// from 1. A report with the new route's id is that route's only after the host consumed the route's `RouteReceived` from
/// the session's own worker: before it, or from another session's worker, a report changes no clock and arms nothing.
#[test]
fn a_report_before_the_routes_receipt_or_from_another_worker_arms_nothing() {
    let mut w = world(1);
    w.running("s2");
    w.engine.poll_events(64);
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
    // A surviving route's reports with the same id, still on the link before the worker binds the new route.
    progress(&mut w, route, 7, true);
    w.worker_says("s1", WorkerMsg::RouteStalled { route, mark: 7 });
    assert_eq!(due_at(&w, route), None, "no report before the receipt");
    // Another session's worker binds its own route of the same id: that receipt is not this route's, so the stale report
    // of s1 still arms nothing, and neither does s2's report.
    w.worker_says("s2", WorkerMsg::RouteReceived { route });
    progress(&mut w, route, 7, true);
    progress_of(&mut w, "s2", route, 7, true);
    assert_eq!(due_at(&w, route), None, "another worker's route");
    // The receipt from the session's own worker: its later reports are this route's.
    w.worker_says("s1", WorkerMsg::RouteReceived { route });
    progress(&mut w, route, 1, true);
    let armed = w.now + STALL;
    assert_eq!(due_at(&w, route), Some(armed));
    // A report of s2's route changes this route's clock neither by a new mark nor by an end of waiting.
    w.advance(Duration::from_millis(500));
    progress_of(&mut w, "s2", route, 9, true);
    progress_of(&mut w, "s2", route, 9, false);
    assert_eq!(due_at(&w, route), Some(armed), "another worker's report");
    w.advance(STALL - Duration::from_millis(500));
    w.pump();
    assert_eq!(dues(&w), [HostMsg::RouteStallDue { route, mark: 1 }]);
}

fn progress_of(w: &mut World, session: &str, route: RouteId, mark: u64, waiting: bool) {
    w.worker_says(
        session,
        WorkerMsg::RouteProgress {
            route,
            mark,
            waiting,
        },
    );
}
