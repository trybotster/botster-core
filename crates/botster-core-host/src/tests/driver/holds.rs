//! `Scheduler::held` (Core A5-2; steward ruling R-88, group E): a step of ready work that the scheduler holds is not runnable
//! work. It does not run, it does not keep `PumpReport.more` true, and no other choice point is asked about it.

use super::deadlines::to_launch;
use super::*;
use botster_core_edges::scheduler::ChoicePoint;
use std::collections::BTreeSet;

/// The production policy, with the ops of `held` held, and every choice that the driver asks recorded in `log`.
struct Holding {
    held: Arc<Mutex<BTreeSet<u64>>>,
    log: Arc<Mutex<Vec<(ChoicePoint, usize)>>>,
    policy: Production,
}

impl Holding {
    fn new() -> (
        Holding,
        Arc<Mutex<BTreeSet<u64>>>,
        Arc<Mutex<Vec<(ChoicePoint, usize)>>>,
    ) {
        let (held, log) = (Arc::default(), Arc::default());
        let policy = Holding {
            held: Arc::clone(&held),
            log: Arc::clone(&log),
            policy: Production::new(),
        };
        (policy, held, log)
    }
}

impl Scheduler for Holding {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        self.log.lock().unwrap().push((point, candidates));
        self.policy.pick(point, candidates)
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        self.log.lock().unwrap().push((point, max));
        self.policy.bound(point, max)
    }

    fn held(&mut self, ops: &[u64]) -> bool {
        let held = self.held.lock().unwrap();
        ops.iter().any(|op| held.contains(op))
    }
}

fn holding_rig() -> (
    Rig,
    Arc<Mutex<BTreeSet<u64>>>,
    Arc<Mutex<Vec<(ChoicePoint, usize)>>>,
) {
    let (policy, held, log) = Holding::new();
    (
        Rig::with_scheduler(CoreLimits::default(), Box::new(policy)),
        held,
        log,
    )
}

fn stopping(events: &[Event], name: &str) -> usize {
    events
        .iter()
        .filter(|e| {
            matches!(e, Event::SessionState { id, state: SessionState::Stopping, .. } if *id == sid(name))
        })
        .count()
}

/// A held `Stop` makes no progress: the session stays `Running`, nothing is posted, and the pump reports no more work, so a
/// host that pumps until `more` is false stops (`pump_idle`). The step that the hold lets go runs in the next pump, once.
#[test]
fn a_held_stop_is_not_runnable_and_runs_once_in_the_pump_after_its_release() {
    let (mut rig, held, _) = holding_rig();
    let link = LinkId(1);
    to_launch(&mut rig, "s1", link);
    rig.worker_says(link, launched());
    rig.settle();
    rig.drain_events();
    let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    held.lock().unwrap().insert(stop.0);
    for _ in 0..4 {
        let report = rig.pump();
        assert!(!report.more, "a held step is not runnable work: {report:?}");
        assert_eq!(rig.drain_events(), []);
    }
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    held.lock().unwrap().remove(&stop.0);
    rig.pump();
    let mut events = rig.drain_events();
    assert_eq!(stopping(&events, "s1"), 1, "{events:?}");
    events.extend(rig.run_out());
    assert_eq!(stopping(&events, "s1"), 1, "the step runs once: {events:?}");
}

/// A deadline is never held: while the `Stop` op waits for its exit and is held, `next_deadline` still gives the
/// `stop_grace` deadline, and the pump at that time sends the kill.
#[test]
fn a_held_op_keeps_its_deadline_and_the_deadline_still_runs() {
    let (mut rig, held, _) = holding_rig();
    let link = LinkId(1);
    to_launch(&mut rig, "s1", link);
    rig.worker_says(link, launched());
    rig.settle();
    let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    let events = rig.run_out();
    assert_eq!(stopping(&events, "s1"), 1, "{events:?}");
    let due = rig.driver.next_deadline().expect("the stop_grace deadline");
    held.lock().unwrap().insert(stop.0);
    assert_eq!(rig.driver.next_deadline(), Some(due));
    let kills = |rig: &Rig| {
        rig.host_frames(link)
            .iter()
            .filter(|(k, p)| {
                *k == FrameType::HOST_MSG && matches!(HostMsg::decode(p), Ok(HostMsg::Kill))
            })
            .count()
    };
    assert_eq!(kills(&rig), 0);
    rig.now = due;
    rig.pump();
    assert_eq!(
        kills(&rig),
        1,
        "the stop_grace kill runs while the op is held"
    );
}

/// Condition 1 of the lead's ruling: a session with no flow serves no op, so a step of it is never held.
#[test]
fn a_session_with_no_flow_serves_no_op() {
    let (mut rig, held, _) = holding_rig();
    let link = LinkId(1);
    let start = to_launch(&mut rig, "s1", link);
    rig.worker_says(link, launched());
    rig.settle();
    // The `Start` op completed, and the session's flow is over: its step serves no op, whatever the scheduler holds.
    held.lock().unwrap().insert(start.0);
    let engine = rig.driver.engine();
    assert_eq!(engine.ops_of(&Work::Session(sid("s1"))), Vec::<u64>::new());
    assert_eq!(engine.ops_of(&Work::Deadline), Vec::<u64>::new());
    assert_eq!(engine.ops_of(&Work::Op(OpId(7))), vec![7]);
}

/// Condition 3: with no hold, the driver asks exactly the choices that it asked before `held` existed, in the same order:
/// a policy that holds an op that never exists sees the same choice sequence as one that holds nothing.
#[test]
fn with_no_hold_the_choice_sequence_is_unchanged() {
    let run = |hold: Option<u64>| {
        let (mut rig, held, log) = holding_rig();
        held.lock().unwrap().extend(hold);
        let link = LinkId(1);
        to_launch(&mut rig, "s1", link);
        rig.worker_says(link, launched());
        rig.settle();
        rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
        rig.run_out();
        let log = log.lock().unwrap().clone();
        log
    };
    let plain = run(None);
    assert!(plain
        .iter()
        .any(|(p, _)| *p == ChoicePoint::OperationDeferral));
    assert_eq!(run(Some(999)), plain);
}
