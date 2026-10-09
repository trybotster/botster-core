//! Core erratum 3 (E3-1): due deadlines and the `pump_events` budget, and the steps that post one event each (9B).

use super::*;
use botster_core_edges::scheduler::ChoicePoint;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A session that runs, through the driver, with a small `pump_events`: it pumps until each step is through.
fn run_session(rig: &mut Rig, name: &str, link: LinkId) {
    to_launch(rig, name, link);
    rig.worker_says(link, launched());
    rig.settle();
}

/// A session whose worker got the launch request and has not answered yet. Returns the `Start` op.
pub(super) fn to_launch(rig: &mut Rig, name: &str, link: LinkId) -> OpId {
    rig.driver.begin(create(name)).unwrap();
    let mut guard = 0;
    while rig.pump().more {
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    let start = rig.driver.begin(Op::Start { id: sid(name) }).unwrap();
    while !rig.mock.lock().unwrap().links.contains_key(&link) {
        rig.pump();
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    // The worker answers after the host sent the launch.
    while !rig.host_frames(link).iter().any(|(k, p)| {
        *k == FrameType::HOST_MSG && matches!(HostMsg::decode(p), Ok(HostMsg::Launch(_)))
    }) {
        rig.pump();
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    start
}

/// A session with a silence deadline `secs` after its last output, through the driver.
fn silent_session(rig: &mut Rig, name: &str, link: LinkId, secs: u64) {
    run_session(rig, name, link);
    rig.worker_says(
        link,
        WorkerMsg::Observed {
            observation: Observation::Output {
                model_rev: ModelRev(2),
            },
        },
    );
    rig.pump();
    rig.driver
        .set_silence_threshold(&sid(name), Some(Duration::from_secs(secs)))
        .unwrap();
}

fn silents(events: &[Event]) -> Vec<SessionId> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Silent { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

/// E3-1 items 2, 3, 6; TM-4: two silences due together with `pump_events = 1`: one is posted with `more`, and the other is
/// carried and posted first in the next pump, before an op that was admitted after.
#[test]
fn e3_1_two_silences_due_together_with_budget_one() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.max_sessions = 4;
        l.mandatory_events = 64;
    }));
    silent_session(&mut rig, "s1", LinkId(1), 3);
    silent_session(&mut rig, "s2", LinkId(2), 3);
    rig.settle();
    rig.now += Duration::from_secs(3);
    rig.unix += 3;
    let first = rig.pump();
    assert_eq!(first.events_posted, 1, "the budget is one");
    assert!(first.more, "the other Silent is carried");
    assert!(rig.wake_set(), "the wake stays signaled (TM-6)");
    let a = silents(&rig.drain_events());
    // E3-1 item 3: the carried step runs before newer work.
    rig.driver
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: Default::default(),
        })
        .unwrap();
    let second = rig.pump();
    assert_eq!(second.events_posted, 1);
    let b = silents(&rig.drain_events());
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1, "the carried Silent came before the op");
    assert_ne!(a, b);
    let mut later = Vec::new();
    for _ in 0..8 {
        rig.pump();
        later.extend(silents(&rig.drain_events()));
    }
    assert!(later.is_empty(), "once per idle period");
}

/// Core TM-5, TM-3: deadlines that are due but not yet pumped are processed in order of due time: two silences that become due
/// in the same pump post their `Silent` events in the order of their due times, whatever the order of the sessions' ids.
#[test]
fn due_deadlines_are_processed_in_order_of_due_time() {
    for (first, second) in [(("s1", 1), ("s2", 3)), (("s2", 1), ("s1", 3))] {
        let mut rig = Rig::new(limits(|l| l.max_sessions = 4));
        let (early, early_secs) = first;
        let (late, late_secs) = second;
        silent_session(
            &mut rig,
            "s1",
            LinkId(1),
            if early == "s1" { early_secs } else { late_secs },
        );
        silent_session(
            &mut rig,
            "s2",
            LinkId(2),
            if early == "s2" { early_secs } else { late_secs },
        );
        rig.drain_events();
        rig.now += Duration::from_secs(late_secs + 1);
        rig.unix += late_secs + 1;
        rig.pump();
        assert_eq!(silents(&rig.drain_events()), vec![sid(early), sid(late)]);
    }
}

/// E3-1 item 3, 9B: a pump runs the silences that are due before newer link input, as many as are due and as the budget
/// allows: three due with `pump_events = 2` post two in the first pump and the third first in the next. A silence that is not
/// due yet does not run, and the newer `Bell` waits for budget.
#[test]
fn a_pump_runs_each_due_silence_first_until_the_budget_runs_out() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 2;
        l.max_sessions = 4;
        l.mandatory_events = 64;
    }));
    silent_session(&mut rig, "s1", LinkId(1), 3);
    silent_session(&mut rig, "s2", LinkId(2), 3);
    silent_session(&mut rig, "s3", LinkId(3), 3);
    silent_session(&mut rig, "later", LinkId(4), 30);
    rig.settle();
    rig.now += Duration::from_secs(3);
    rig.unix += 3;
    rig.worker_says(
        LinkId(4),
        WorkerMsg::Observed {
            observation: Observation::Bell,
        },
    );
    let first = rig.pump();
    let events = rig.drain_events();
    assert_eq!(first.events_posted, 2, "{events:?}");
    assert!(first.more, "a silence and the Bell are carried");
    let mut ran = silents(&events);
    assert_eq!(
        ran.len(),
        2,
        "the budget runs two due silences, before the Bell: {events:?}"
    );
    rig.pump();
    let events = rig.drain_events();
    let next = silents(&events);
    assert_eq!(next.len(), 1, "the third due silence: {events:?}");
    ran.extend(next);
    ran.sort();
    assert_eq!(
        ran,
        [sid("s1"), sid("s2"), sid("s3")],
        "each due silence once"
    );
    for _ in 0..4 {
        rig.pump();
        assert!(
            silents(&rig.drain_events()).is_empty(),
            "no silence of a session not due"
        );
    }
}

/// 9B `pump_events`: a worker's `Bell` and the `Done` of an op that arrive together, with `pump_events = 1`, post one event
/// in each pump: the completion is carried to the next pump, and it is not lost.
#[test]
fn a_completion_behind_an_event_of_its_pump_comes_in_the_next_pump() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    let op = rig
        .driver
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: Default::default(),
        })
        .unwrap();
    rig.settle();
    let req = rig
        .host_frames(LinkId(1))
        .iter()
        .rev()
        .find_map(|(k, p)| match HostMsg::decode(p) {
            Ok(HostMsg::Op {
                req,
                op: Op::UpdateMetadata { .. },
            }) if *k == FrameType::HOST_MSG => Some(req),
            _ => None,
        })
        .expect("the setter went to the worker");
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Observed {
            observation: Observation::Bell,
        },
    );
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Unit),
        },
    );
    let first = rig.pump();
    let events = rig.drain_events();
    assert_eq!(first.events_posted, 1, "{events:?}");
    assert!(first.more, "the completion is carried");
    assert!(
        events.iter().any(|e| matches!(e, Event::Bell { .. })),
        "{events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, Event::Completed { .. })),
        "{events:?}"
    );
    rig.pump();
    let events = rig.drain_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Completed { op: o, result: OpResult::Ok(OpOutput::Unit) } if *o == op
        )),
        "{events:?}"
    );
}

/// E3-1 item 1: an effect with no event (the kill at the end of `stop_grace`) runs in its due pump whatever the budget.
#[test]
fn e3_1_due_effect_without_event_runs_in_its_pump_with_no_budget() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.stop_grace = Duration::from_millis(100);
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    for _ in 0..10 {
        rig.pump();
        rig.drain_events();
    }
    let kills = |rig: &Rig| {
        rig.host_frames(LinkId(1))
            .iter()
            .filter(|(k, p)| {
                *k == FrameType::HOST_MSG && matches!(HostMsg::decode(p), Ok(HostMsg::Kill))
            })
            .count()
    };
    assert_eq!(kills(&rig), 0);
    // Link input uses the whole budget of the pump in which the kill is due.
    for _ in 0..4 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: Observation::Bell,
            },
        );
    }
    rig.now += Duration::from_millis(100);
    let report = rig.pump();
    assert_eq!(report.events_posted, 1, "the budget was used by the input");
    assert_eq!(kills(&rig), 1, "the kill ran in its due pump");
}

/// 9B, LC-9: `UpdateMetadata` posts `Completed` and `MetadataChanged` in two steps: a pump with `pump_events = 1` never posts
/// two, and `Completed` comes first.
#[test]
fn a_step_posts_one_event_for_metadata() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    rig.settle();
    rig.driver
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: Default::default(),
        })
        .unwrap();
    let mut seen = Vec::new();
    for _ in 0..10 {
        let r = rig.pump();
        assert!(r.events_posted <= 1, "{r:?}");
        seen.extend(rig.drain_events());
    }
    let kinds: Vec<&str> = seen
        .iter()
        .filter_map(|e| match e {
            Event::Completed { .. } => Some("completed"),
            Event::MetadataChanged { .. } => Some("changed"),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["completed", "changed"], "LC-9 order");
}

/// E3-1 item 3: a carried `Silent` runs before link input that arrived later: a newer `Bell` does not take the budget.
#[test]
fn e3_1_a_carried_silent_runs_before_newer_link_input() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    silent_session(&mut rig, "s1", LinkId(1), 3);
    rig.settle();
    rig.now += Duration::from_secs(3);
    rig.unix += 3;
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Observed {
            observation: Observation::Bell,
        },
    );
    let report = rig.pump();
    assert_eq!(report.events_posted, 1);
    let events = rig.drain_events();
    assert_eq!(silents(&events).len(), 1, "{events:?}");
}

/// 9B: two route handoffs that fail post two `RouteClosed` events in two pumps when `pump_events = 1`.
#[test]
fn two_failed_handoffs_post_one_event_per_pump() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
        l.max_sessions = 4;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    let attach = |rig: &mut Rig| {
        rig.driver
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
    };
    attach(&mut rig);
    attach(&mut rig);
    let mut closed = 0;
    for _ in 0..20 {
        let r = rig.pump();
        assert!(r.events_posted <= 1, "{r:?}");
        closed += rig
            .drain_events()
            .iter()
            .filter(|e| matches!(e, Event::RouteClosed { .. }))
            .count();
    }
    assert_eq!(closed, 2);
}

/// A scheduler that defers every second piece of work that it is asked about, and varies nothing else: work that no clause
/// fixes is deferred, and it still progresses.
struct DefersEveryOther(Production, u32);

impl Scheduler for DefersEveryOther {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        match point {
            ChoicePoint::OperationDeferral => {
                self.1 += 1;
                usize::from(self.1 % 2 == 1)
            }
            _ => self.0.pick(point, candidates),
        }
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        self.0.bound(point, max)
    }
}

/// A scheduler that defers every operation while its switch is on, and varies nothing else.
pub(super) struct Switched(pub(super) Production, pub(super) Arc<AtomicBool>);

impl Scheduler for Switched {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        match point {
            ChoicePoint::OperationDeferral => usize::from(self.1.load(Ordering::SeqCst)),
            _ => self.0.pick(point, candidates),
        }
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        self.0.bound(point, max)
    }
}

/// Steward ruling R-20: while the scheduler defers every operation, the fixed-timing ops still complete in the next pump, and an
/// operation that no clause fixes does not.
#[test]
fn r_20_fixed_ops_complete_while_the_scheduler_defers_everything_else() {
    let on = Arc::new(AtomicBool::new(false));
    let mut rig = Rig::with_scheduler(
        CoreLimits::default(),
        Box::new(Switched(Production::new(), Arc::clone(&on))),
    );
    run_session(&mut rig, "s1", LinkId(1));
    rig.driver.begin(create("c")).unwrap();
    for _ in 0..6 {
        rig.pump();
        rig.drain_events();
    }
    on.store(true, Ordering::SeqCst);
    let created_resize = rig
        .driver
        .begin(Op::Resize {
            session: sid("c"),
            size: Size {
                rows: 30,
                cols: 100,
                cell_px: None,
            },
        })
        .unwrap();
    let current = rig.driver.get(&sid("s1")).unwrap().size;
    let same = rig
        .driver
        .begin(Op::Resize {
            session: sid("s1"),
            size: current,
        })
        .unwrap();
    let read = rig
        .driver
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    rig.pump();
    let events = rig.drain_events();
    let done = |op: OpId| {
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
    };
    assert!(done(created_resize), "A2-1 Created Resize");
    assert!(done(same), "SZ-2");
    assert!(!done(read), "an operation that no clause fixes is deferred");
    let forwarded = rig
        .host_frames(LinkId(1))
        .iter()
        .filter(|(k, p)| {
            *k == FrameType::HOST_MSG
                && matches!(
                    HostMsg::decode(p),
                    Ok(HostMsg::Op {
                        op: Op::ReadCursor { .. },
                        ..
                    })
                )
        })
        .count();
    assert_eq!(forwarded, 0, "the deferred read was not sent");
    // A `Resize` to a new size and a `Stop` of a running payload are deferred too: no clause fixes their timing.
    rig.driver
        .begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: current.rows + 1,
                ..current
            },
        })
        .unwrap();
    rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    rig.pump();
    rig.drain_events();
    let sent: Vec<HostMsg> = rig
        .host_frames(LinkId(1))
        .iter()
        .filter(|(k, _)| *k == FrameType::HOST_MSG)
        .filter_map(|(_, p)| HostMsg::decode(p).ok())
        .collect();
    assert!(
        !sent.iter().any(|m| matches!(
            m,
            HostMsg::Op {
                op: Op::Resize { .. },
                ..
            } | HostMsg::Stop
        )),
        "the resize and the stop were deferred: {sent:?}"
    );
    // LC-5: once the payload exited, a `Stop` completes in the next pump while everything else is deferred.
    on.store(false, Ordering::SeqCst);
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    for _ in 0..20 {
        rig.pump();
        rig.drain_events();
    }
    assert!(matches!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
    on.store(true, Ordering::SeqCst);
    let again = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    rig.pump();
    let events = rig.drain_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, .. } if *op == again)),
        "LC-5: {events:?}"
    );
}

/// AD-2: an exit that the process edge reports ends the session `Lost(WorkerGone)`, and one exit per call of the edge is taken
/// while the pump has budget.
#[test]
fn the_driver_takes_the_exits_of_the_process_edge() {
    let mut rig = Rig::new(CoreLimits::default());
    run_session(&mut rig, "s1", LinkId(1));
    let worker = ProcessIdentity {
        pid: 500,
        start_time: 1,
    };
    rig.mock
        .lock()
        .unwrap()
        .exits
        .push((worker, ExitStatus::Code(0)));
    for _ in 0..6 {
        rig.pump();
        rig.drain_events();
    }
    assert!(
        rig.mock.lock().unwrap().exits.is_empty(),
        "the exit was taken"
    );
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
}

/// Core AM-2: the writes of one session reach the worker in `begin` order, whatever the scheduler defers: a deferred write
/// holds the later writes of its session.
#[test]
fn writes_of_one_session_reach_the_worker_in_begin_order() {
    let mut rig = Rig::with_scheduler(
        CoreLimits::default(),
        Box::new(DefersEveryOther(Production::new(), 0)),
    );
    run_session(&mut rig, "s1", LinkId(1));
    for _ in 0..60 {
        if rig.driver.get(&sid("s1")).unwrap().state == SessionState::Running {
            break;
        }
        rig.pump();
        rig.drain_events();
    }
    let write = |text: &str| Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Text { text: text.into() },
        guard: None,
    };
    for text in ["a", "b", "c", "d"] {
        rig.driver.begin(write(text)).unwrap();
    }
    for _ in 0..20 {
        rig.pump();
        rig.drain_events();
    }
    let sent: Vec<String> = rig
        .host_frames(LinkId(1))
        .iter()
        .filter_map(|(k, p)| match HostMsg::decode(p) {
            Ok(HostMsg::Op {
                op:
                    Op::WriteInput {
                        payload: InputPayload::Text { text },
                        ..
                    },
                ..
            }) if *k == FrameType::HOST_MSG => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(sent, ["a", "b", "c", "d"]);
}

/// The sizes of the `Resize` ops that the host sent to the worker, in the order sent.
fn sent_resizes(rig: &Rig, link: LinkId) -> Vec<Size> {
    rig.host_frames(link)
        .iter()
        .filter_map(|(k, p)| match HostMsg::decode(p) {
            Ok(HostMsg::Op {
                op: Op::Resize { size, .. },
                ..
            }) if *k == FrameType::HOST_MSG => Some(size),
            _ => None,
        })
        .collect()
}

fn rows(rows: u32) -> Size {
    Size {
        rows,
        cols: 80,
        cell_px: None,
    }
}

/// Core SZ-3, OR-1: the resizes of one session reach the worker in `begin` order, whatever the scheduler defers, so the last
/// resize begun is the one applied.
#[test]
fn resizes_of_one_session_reach_the_worker_in_begin_order() {
    let mut rig = Rig::with_scheduler(
        CoreLimits::default(),
        Box::new(DefersEveryOther(Production::new(), 0)),
    );
    run_session(&mut rig, "s1", LinkId(1));
    let sizes = [rows(30), rows(31), rows(32)];
    for size in sizes {
        rig.driver
            .begin(Op::Resize {
                session: sid("s1"),
                size,
            })
            .unwrap();
    }
    for _ in 0..20 {
        rig.pump();
        rig.drain_events();
    }
    assert_eq!(sent_resizes(&rig, LinkId(1)), sizes);
}

/// Core SZ-2, SZ-3: a `Resize` back to the current size while another `Resize` is in flight is not the same-size case: the
/// in-flight one applies later, so this one goes to the worker after it and does not complete at once.
#[test]
fn a_resize_to_the_current_size_waits_for_a_resize_in_flight() {
    let mut rig = Rig::new(CoreLimits::default());
    run_session(&mut rig, "s1", LinkId(1));
    let current = rig.driver.get(&sid("s1")).unwrap().size;
    let other = Size {
        rows: current.rows + 1,
        ..current
    };
    rig.driver
        .begin(Op::Resize {
            session: sid("s1"),
            size: other,
        })
        .unwrap();
    rig.pump();
    rig.drain_events();
    let back = rig
        .driver
        .begin(Op::Resize {
            session: sid("s1"),
            size: current,
        })
        .unwrap();
    let mut completed = false;
    for _ in 0..10 {
        rig.pump();
        completed |= rig
            .drain_events()
            .iter()
            .any(|e| matches!(e, Event::Completed { op, .. } if *op == back));
    }
    assert_eq!(sent_resizes(&rig, LinkId(1)), [other, current]);
    assert!(
        !completed,
        "the resize back completes only after the worker answers"
    );
}

/// A5-2: the scheduler defers the progress of an operation, not the transition that an edge report causes: when the process
/// edge reports the exit of a worker, the session is `Lost` in the pump that takes the report, whatever the scheduler defers.
#[test]
fn a_reported_exit_is_posted_in_its_pump_whatever_the_scheduler_defers() {
    let on = Arc::new(AtomicBool::new(false));
    let mut rig = Rig::with_scheduler(
        CoreLimits::default(),
        Box::new(Switched(Production::new(), Arc::clone(&on))),
    );
    run_session(&mut rig, "s1", LinkId(1));
    on.store(true, Ordering::SeqCst);
    rig.mock.lock().unwrap().exits.push((
        ProcessIdentity {
            pid: 500,
            start_time: 1,
        },
        ExitStatus::Code(0),
    ));
    rig.pump();
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
}

/// TM-6, plan 2.5: a pump that leaves work or unread input reports `more` and keeps the wake signaled, and a pump that finds
/// work in the readiness that it settles at its end is not quiet: `more` is true.
#[test]
fn a_pump_that_leaves_work_keeps_the_wake_and_looks_again_after_settling() {
    // Work remains.
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.max_sessions = 4;
    }));
    rig.driver.begin(create("a")).unwrap();
    rig.driver.begin(create("b")).unwrap();
    assert!(rig.pump().more);
    assert!(rig.wake_set(), "work remains: the wake stays signaled");
    // Input remains: the same.
    let mut rig = Rig::new(limits(|l| l.pump_bytes = 64));
    run_session(&mut rig, "s1", LinkId(1));
    for _ in 0..40 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: Observation::Bell,
            },
        );
    }
    assert!(rig.pump().more);
    assert!(rig.wake_set(), "input remains: the wake stays signaled");
    // The settle reports an exit: the pump looks again and has work.
    let mut rig = Rig::new(CoreLimits::default());
    run_session(&mut rig, "s1", LinkId(1));
    let mut payload = Vec::new();
    WorkerMsg::Exited {
        code: Some(0),
        signal: None,
    }
    .encode(&mut payload);
    rig.mock
        .lock()
        .unwrap()
        .late
        .push((LinkId(1), frame(FrameType::WORKER_MSG, &payload)));
    let report = rig.pump();
    assert!(report.more, "the late exit is work");
}

/// TM-6, AD-2: an exit that the process edge queues while the pump ends, and whose wake the settle consumes, is taken in that
/// pump, which then reports more work with the wake set: nothing waits in the edge without a wake, and the next pump ends the
/// session `Lost(WorkerGone)`.
#[test]
fn an_exit_reaped_while_the_pump_settles_is_taken_in_that_pump() {
    let mut rig = Rig::new(CoreLimits::default());
    run_session(&mut rig, "s1", LinkId(1));
    rig.mock.lock().unwrap().late_exits.push((
        ProcessIdentity {
            pid: 500,
            start_time: 1,
        },
        ExitStatus::Code(0),
    ));
    let report = rig.pump();
    assert!(
        rig.mock.lock().unwrap().exits.is_empty(),
        "the exit was taken"
    );
    assert!(report.more && rig.wake_set(), "the host pumps again");
    rig.pump();
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
}
