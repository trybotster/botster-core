//! Worker observations around the start of a session: they follow `Running` and the completion of `Start` (OR-2, EV-5), keep
//! the worker's order (ST-4, EV-6), and wait unread on their link, not in the host (plan 2.5 rule 7).

use super::deadlines::{to_launch, Switched};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn observed(observation: Observation) -> WorkerMsg {
    WorkerMsg::Observed { observation }
}

fn modes(alt_screen: bool, rev: u64) -> (ModeFlags, WorkerMsg) {
    let mut flags = terminal_state().modes.clone();
    flags.alt_screen = alt_screen;
    let msg = observed(Observation::Modes {
        flags: flags.clone(),
        model_rev: ModelRev(rev),
    });
    (flags, msg)
}

/// Core ST-4, EV-6, OR-2: an observation that waits for the end of the start stays before the observations that arrive after
/// it. The older `Modes` goes in first, so the cache, the revision and the last `ModesChanged` are the newer ones; two class D
/// events keep their order across the end of the start.
#[test]
fn observations_keep_the_worker_order_across_the_end_of_the_start() {
    let mut rig = Rig::new(limits(|l| l.pump_events = 1));
    let start = to_launch(&mut rig, "s1", LinkId(1));
    let alt = terminal_state().modes.alt_screen;
    let (_, older) = modes(!alt, 5);
    rig.worker_says(LinkId(1), launched());
    rig.worker_says(LinkId(1), older);
    rig.worker_says(LinkId(1), observed(Observation::Bell));
    let mut events = Vec::new();
    let mut guard = 0;
    while !events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, .. } if *op == start))
    {
        rig.pump();
        events.extend(rig.drain_events());
        guard += 1;
        assert!(guard < 100);
    }
    // The start just ended: newer observations arrive while the older ones may still wait.
    let (newer_flags, newer) = modes(alt, 6);
    rig.worker_says(LinkId(1), newer);
    rig.worker_says(
        LinkId(1),
        observed(Observation::PromptMark {
            mark: PromptMarkKind::PromptStart,
            exit_code: None,
        }),
    );
    for _ in 0..20 {
        rig.pump();
        events.extend(rig.drain_events());
    }
    let state = rig.driver.terminal_state(&sid("s1")).unwrap();
    assert_eq!(state.modes, newer_flags, "the cache has the newer modes");
    assert_eq!(state.model_rev, ModelRev(6));
    let last_modes = events.iter().rev().find_map(|e| match e {
        Event::ModesChanged { flags, .. } => Some(flags.clone()),
        _ => None,
    });
    assert_eq!(last_modes, Some(newer_flags), "{events:?}");
    let position = |wanted: fn(&Event) -> bool| events.iter().position(wanted).unwrap();
    let running = position(|e| {
        matches!(
            e,
            Event::SessionState {
                state: SessionState::Running,
                ..
            }
        )
    });
    let bell = position(|e| matches!(e, Event::Bell { .. }));
    let mark = position(|e| matches!(e, Event::PromptMark { .. }));
    assert!(running < bell && bell < mark, "{events:?}");
}

/// Core EV-2, EV-5, plan 2.5 rules 7 and 8: while the start's progress is deferred, the observations behind `Launched` stay
/// unread on the link (the host holds one frame and reads no more), and they all apply, in order, when the start ends.
#[test]
fn observations_wait_unread_on_the_link_while_the_start_is_deferred() {
    let on = Arc::new(AtomicBool::new(false));
    let mut rig = Rig::with_scheduler(
        CoreLimits::default(),
        Box::new(Switched(Production::new(), Arc::clone(&on))),
    );
    to_launch(&mut rig, "s1", LinkId(1));
    on.store(true, Ordering::SeqCst);
    rig.worker_says(LinkId(1), launched());
    for n in 0..1000 {
        rig.worker_says(
            LinkId(1),
            observed(Observation::Title {
                title: format!("t{n}"),
                model_rev: ModelRev(n + 1),
            }),
        );
    }
    for _ in 0..10 {
        rig.pump();
        rig.drain_events();
        let mock = rig.mock.lock().unwrap();
        let link = &mock.links[&LinkId(1)];
        assert!(
            !link.read_interest,
            "the link is not read while the start runs"
        );
        assert!(
            !link.to_host.is_empty(),
            "the observations stay on the link: the host keeps one read chunk and one held frame"
        );
    }
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Starting
    );
    on.store(false, Ordering::SeqCst);
    let mut guard = 0;
    while rig.pump().more {
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    assert!(rig.mock.lock().unwrap().links[&LinkId(1)].read_interest);
    let state = rig.driver.terminal_state(&sid("s1")).unwrap();
    assert_eq!(state.title.as_deref(), Some("t999"));
    assert_eq!(state.model_rev, ModelRev(1000));
}

/// Pumps until `start` completes, and returns the events.
fn start_completed(rig: &mut Rig, start: OpId) -> Vec<Event> {
    let mut events = Vec::new();
    let mut guard = 0;
    while !events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, .. } if *op == start))
    {
        rig.pump();
        events.extend(rig.drain_events());
        guard += 1;
        assert!(guard < 100);
    }
    events
}

fn title(text: &str, rev: u64) -> WorkerMsg {
    observed(Observation::Title {
        title: text.into(),
        model_rev: ModelRev(rev),
    })
}

/// Pumps without polling the queue, as the `await_title` step does (steward ruling R-57), until `terminal_state` shows
/// `sentinel`. Returns whether a pump ended with the earlier observation posted and the sentinel not yet applied: the budget
/// ran out between them.
fn pump_until_title(rig: &mut Rig, sentinel: &str) -> bool {
    let mut split = false;
    for _ in 0..20 {
        let report = rig.pump();
        assert!(
            report.events_posted <= 1,
            "a pump over its budget: {report:?}"
        );
        let state = rig.driver.terminal_state(&sid("s1")).unwrap();
        if state.title.as_deref() == Some(sentinel) {
            return split;
        }
        split |= report.events_posted == 1;
    }
    panic!("the title never showed {sentinel:?}");
}

/// Core ST-4, EV-1 class K, E3-1 (steward ruling R-57 condition 3): the title of `terminal_state` never runs ahead of the event
/// queue. The worker observes a bell and then a title. With `pump_events: 1`, one pump posts the bell and stops before the
/// title; the cached title is still the old one then. When the read shows the new title, the bell and the title's
/// `TitleChanged` have both been posted, in that order. Nothing polls the queue until then.
#[test]
fn the_title_never_runs_ahead_of_the_queue() {
    let mut rig = Rig::new(limits(|l| l.pump_events = 1));
    let start = to_launch(&mut rig, "s1", LinkId(1));
    rig.worker_says(LinkId(1), launched());
    start_completed(&mut rig, start);
    rig.worker_says(LinkId(1), observed(Observation::Bell));
    rig.worker_says(LinkId(1), title("sentinel", 7));
    assert!(
        pump_until_title(&mut rig, "sentinel"),
        "no pump ended between the bell and the title"
    );
    let events = rig.drain_events();
    let bell = events.iter().position(|e| matches!(e, Event::Bell { .. }));
    let changed = events.iter().position(
        |e| matches!(e, Event::TitleChanged { title, .. } if title.as_str() == "sentinel"),
    );
    assert!(
        matches!((bell, changed), (Some(b), Some(t)) if b < t),
        "{events:?}"
    );
}

/// Core EV-2 with R-57 condition 3, the shape of `conf::ev_2_one_marker_per_session`: the class D queue is full, so the last
/// bell drops the oldest unpolled notification. When the read shows the sentinel title that the worker observed after that bell,
/// the loss marker already names both kinds, though the queue was never polled.
#[test]
fn the_title_never_runs_ahead_of_a_full_queue() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.event_queue = 1;
    }));
    let start = to_launch(&mut rig, "s1", LinkId(1));
    rig.worker_says(LinkId(1), launched());
    start_completed(&mut rig, start);
    rig.worker_says(LinkId(1), observed(Observation::Bell));
    rig.worker_says(
        LinkId(1),
        observed(Observation::Notification {
            source: NotificationSource::Osc9,
            title: None,
            body: "n".into(),
            truncated: false,
        }),
    );
    rig.worker_says(LinkId(1), observed(Observation::Bell));
    rig.worker_says(LinkId(1), title("sentinel", 9));
    assert!(
        pump_until_title(&mut rig, "sentinel"),
        "no pump ended between the bell and the title"
    );
    let events = rig.drain_events();
    let lost = events.iter().find_map(|e| match e {
        Event::EventsLost { kinds, .. } => Some(kinds.clone()),
        _ => None,
    });
    assert_eq!(
        lost,
        Some(BTreeSet::from([LostKind::Bell, LostKind::Notification])),
        "{events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, Event::TitleChanged { title, .. } if title.as_str() == "sentinel")
        ),
        "{events:?}"
    );
}
