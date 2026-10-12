//! The stall clock at the driver (steward ruling R-90 and its follow-ups): the slot that a due step holds, and the mandatory
//! frames that the worker sent ahead of the step's answer on the same link (0181's ST298-R1; contracts `main` `1798a65d`).
//! Frames come through `link_recv` and `can_accept`, as on a real link, so a frame that the engine cannot take stays unread.

use super::*;
use botster_core_link::msg::HostMsg;

const STALL: Duration = Duration::from_secs(1);
const CLOSE_AFTER: Duration = Duration::from_secs(5);

fn rig(mandatory: u32) -> Rig {
    Rig::new(limits(|l| {
        l.mandatory_events = mandatory;
        l.max_sessions = 4;
        l.stall_close_after = CLOSE_AFTER;
    }))
}

/// `start_session` with a poll after each step: the start's own events need the room of a small queue.
fn start(rig: &mut Rig, name: &str, link: LinkId) {
    rig.driver.begin(create(name)).unwrap();
    rig.run_out();
    rig.driver.begin(Op::Start { id: sid(name) }).unwrap();
    rig.run_out();
    rig.worker_says(link, launched());
    rig.run_out();
}

/// A route of `session`, whose worker on `link` bound it.
fn attach(rig: &mut Rig, session: &str, link: LinkId) -> RouteId {
    let route = rig
        .driver
        .attach(
            ClientId("c".into()),
            sid(session),
            RouteTransport::Stream(StreamEndpoint::new(Arc::new(0u32))),
            AttachOptions {
                file_directory: "/tmp".into(),
                file_permissions: None,
                route_features: vec![],
                terminal_formats: vec![],
                owner: None,
                query_deadline: None,
                route_tag: None,
                route_limits: None,
                history: None,
                stall_deadline: Some(STALL),
                answers_queries: false,
                input: true,
            },
        )
        .unwrap()
        .route;
    rig.pump();
    rig.worker_says(link, WorkerMsg::RouteReceived { route });
    rig.pump();
    route
}

fn waiting(rig: &mut Rig, link: LinkId, route: RouteId, mark: u64) {
    rig.worker_says(
        link,
        WorkerMsg::RouteProgress {
            route,
            mark,
            waiting: true,
        },
    );
    rig.pump();
}

/// The due messages that the host wrote on `link`, in order.
fn dues(rig: &Rig, link: LinkId) -> Vec<HostMsg> {
    rig.host_frames(link)
        .into_iter()
        .filter(|(kind, _)| *kind == FrameType::HOST_MSG)
        .filter_map(|(_, payload)| HostMsg::decode(&payload).ok())
        .filter(|m| {
            matches!(
                m,
                HostMsg::RouteStallDue { .. } | HostMsg::RouteCloseDue { .. }
            )
        })
        .collect()
}

fn closed(route: RouteId) -> WorkerMsg {
    WorkerMsg::RouteClosed {
        route,
        reason: RouteCloseReason::WriteFailed,
        route_tag: None,
    }
}

fn parked_work(rig: &Rig) -> serde_json::Value {
    rig.driver.diagnostics()["parked_work"].clone()
}

/// 0181's deadlock (ST298-R1): a stalled route whose worker resumed before the close deadline. Its `RouteResumed` is on
/// the link, unread, when the pump runs the close step, and the step reserves the only slot of an empty queue. The resume
/// is a frame ahead of the step's answer: it takes the slot over, and its event posts in the same pump, with no poll.
#[test]
fn a_resume_ahead_of_a_due_close_takes_its_slot_over() {
    let mut rig = rig(1);
    start(&mut rig, "s1", LinkId(1));
    let route = attach(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    waiting(&mut rig, LinkId(1), route, 3);
    rig.now += STALL;
    rig.pump();
    rig.worker_says(LinkId(1), WorkerMsg::RouteStalled { route, mark: 3 });
    rig.pump();
    assert_eq!(rig.drain_events(), [Event::RouteStalled { route }]);
    // The worker resumed; the host reads it only after the close step.
    rig.worker_says(LinkId(1), WorkerMsg::RouteResumed { route, mark: 4 });
    rig.now += CLOSE_AFTER;
    let report = rig.pump();
    assert_eq!(
        dues(&rig, LinkId(1)),
        [
            HostMsg::RouteStallDue { route, mark: 3 },
            HostMsg::RouteCloseDue { route, mark: 3 }
        ]
    );
    assert_eq!(rig.drain_events(), [Event::RouteResumed { route }]);
    assert!(!report.more);
    assert_eq!(parked_work(&rig), serde_json::json!([]));
    // The step ended with the resume: the queue has its room again.
    waiting(&mut rig, LinkId(1), route, 5);
    rig.now += STALL;
    rig.pump();
    assert_eq!(
        dues(&rig, LinkId(1)).last(),
        Some(&HostMsg::RouteStallDue { route, mark: 5 })
    );
}

/// The steward's form (R-90 follow-up, contracts `main` `1798a65d`): an empty queue of one slot, which a due stall step
/// reserves. The worker's close of another route is ahead of the step's answer on the link: it takes the slot over and
/// posts. The answer is then an ordinary route report: with no room it is parked, `parked_work` names its session, and a
/// poll frees room, signals the wake, and the next pump posts it.
#[test]
fn a_frame_ahead_of_the_answer_takes_the_slot_and_the_answer_waits_for_a_poll() {
    let mut rig = rig(1);
    start(&mut rig, "s1", LinkId(1));
    let route = attach(&mut rig, "s1", LinkId(1));
    let other = attach(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    waiting(&mut rig, LinkId(1), route, 3);
    rig.now += STALL;
    rig.worker_says(LinkId(1), closed(other));
    rig.worker_says(LinkId(1), WorkerMsg::RouteStalled { route, mark: 3 });
    let report = rig.pump();
    assert_eq!(
        dues(&rig, LinkId(1)),
        [HostMsg::RouteStallDue { route, mark: 3 }]
    );
    assert!(!report.more, "the answer waits for room only");
    assert_eq!(parked_work(&rig), serde_json::json!(["s1"]));
    let events = rig.drain_events();
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, .. }] if *r == other),
        "{events:?}"
    );
    assert!(rig.wake_set(), "the poll freed room (EV-5d)");
    rig.pump();
    assert_eq!(rig.drain_events(), [Event::RouteStalled { route }]);
    assert_eq!(parked_work(&rig), serde_json::json!([]));
}

/// Several routes due at once: their steps reserve every slot of an empty queue. A frame ahead of one answer takes that
/// slot over; the other answer posts in its own slot; the first answer posts after a poll. A poll never returns more than
/// the queue's bound.
#[test]
fn several_due_routes_reserve_every_slot_and_a_frame_ahead_still_passes() {
    let mut rig = rig(2);
    start(&mut rig, "s1", LinkId(1));
    start(&mut rig, "s2", LinkId(2));
    let a = attach(&mut rig, "s1", LinkId(1));
    let a2 = attach(&mut rig, "s1", LinkId(1));
    let b = attach(&mut rig, "s2", LinkId(2));
    rig.drain_events();
    waiting(&mut rig, LinkId(1), a, 3);
    waiting(&mut rig, LinkId(2), b, 7);
    rig.now += STALL;
    rig.pump();
    assert_eq!(
        dues(&rig, LinkId(1)),
        [HostMsg::RouteStallDue { route: a, mark: 3 }]
    );
    assert_eq!(
        dues(&rig, LinkId(2)),
        [HostMsg::RouteStallDue { route: b, mark: 7 }]
    );
    rig.worker_says(LinkId(1), closed(a2));
    rig.worker_says(LinkId(1), WorkerMsg::RouteStalled { route: a, mark: 3 });
    rig.worker_says(LinkId(2), WorkerMsg::RouteStalled { route: b, mark: 7 });
    rig.pump();
    let first = rig.drain_events();
    assert!(first.len() <= 2, "{first:?}");
    assert!(
        first.contains(&Event::RouteStalled { route: b }),
        "{first:?}"
    );
    assert!(
        first
            .iter()
            .any(|e| matches!(e, Event::RouteClosed { route, .. } if *route == a2)),
        "{first:?}"
    );
    rig.pump();
    assert_eq!(rig.drain_events(), [Event::RouteStalled { route: a }]);
}
