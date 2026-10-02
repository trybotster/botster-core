//! Core erratum 3 (E3-1): due deadlines and the `pump_events` budget, and the steps that post one event each (9B).

use super::*;

/// A session that runs, through the driver, with a small `pump_events`: it pumps until each step is through.
fn run_session(rig: &mut Rig, name: &str, link: LinkId) {
    rig.driver.begin(create(name)).unwrap();
    let mut guard = 0;
    while rig.pump().more {
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    rig.driver.begin(Op::Start { id: sid(name) }).unwrap();
    while !rig.mock.lock().unwrap().links.contains_key(&link) {
        rig.pump();
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    rig.worker_says(link, launched());
    while rig.pump().more {
        rig.drain_events();
        guard += 1;
        assert!(guard < 200);
    }
    rig.drain_events();
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
    while rig.pump().more {
        rig.drain_events();
    }
    rig.drain_events();
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
    while rig.pump().more {
        rig.drain_events();
    }
    rig.drain_events();
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
