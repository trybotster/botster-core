//! `Scheduler::held` (Core A5-2; steward ruling R-88, group E): a step of ready work that the scheduler holds is not runnable
//! work. It does not run, it does not keep `PumpReport.more` true, and no other choice point is asked about it.

use super::deadlines::to_launch;
use super::*;
use botster_core_edges::scheduler::ChoicePoint;
use std::collections::BTreeSet;

/// The ops that a test holds.
type Held = Arc<Mutex<BTreeSet<u64>>>;
/// Every choice that the driver asks, in order.
type Log = Arc<Mutex<Vec<(ChoicePoint, usize)>>>;
/// Every admission that the driver reports, in order: the op and its fixed timing.
type Admitted = Arc<Mutex<Vec<(u64, bool)>>>;

/// The production policy, with the ops of `held` held, every choice that the driver asks recorded in `log`, and every
/// admission that it reports recorded in `admitted`.
struct Holding {
    held: Held,
    log: Log,
    admitted: Admitted,
    policy: Production,
}

impl Holding {
    fn new() -> (Holding, Held, Log, Admitted) {
        let (held, log, admitted) = (Arc::default(), Arc::default(), Arc::default());
        let policy = Holding {
            held: Arc::clone(&held),
            log: Arc::clone(&log),
            admitted: Arc::clone(&admitted),
            policy: Production::new(),
        };
        (policy, held, log, admitted)
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

    fn admitted(&mut self, op: u64, fixed_timing: bool) {
        self.admitted.lock().unwrap().push((op, fixed_timing));
    }
}

fn holding_rig() -> (Rig, Held, Log, Admitted) {
    let (policy, held, log, admitted) = Holding::new();
    (
        Rig::with_scheduler(CoreLimits::default(), Box::new(policy)),
        held,
        log,
        admitted,
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
    let (mut rig, held, _, _) = holding_rig();
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
    let (mut rig, held, _, _) = holding_rig();
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

/// Condition 1 of the lead's ruling: a session whose flow is over serves no op, so a hold of the ops that it served before
/// does not stop its later steps. Its `Create` and `Start` completed; with both held, the payload's exit still runs to
/// `Exited`.
#[test]
fn a_session_with_no_flow_serves_no_op() {
    let (mut rig, held, _, _) = holding_rig();
    let link = LinkId(1);
    let start = to_launch(&mut rig, "s1", link);
    // `to_launch` begins the `Create` just before the `Start`.
    let create = start.0 - 1;
    rig.worker_says(link, launched());
    rig.settle();
    held.lock().unwrap().extend([create, start.0]);
    rig.worker_says(
        link,
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    rig.run_out();
    assert_eq!(rig.driver.get(&sid("s1")).unwrap().state, exited_normally());
}

/// Condition 3: with no hold, the driver asks exactly the choices that it asked before `held` existed, in the same order:
/// a policy that holds an op that never exists sees the same choice sequence as one that holds nothing.
#[test]
fn with_no_hold_the_choice_sequence_is_unchanged() {
    let run = |hold: Option<u64>| {
        let (mut rig, held, log, _) = holding_rig();
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

/// `Scheduler::admitted`: `begin` reports each op that Core admits once, with whether a clause fixes its timing (R-20), and
/// asks no choice point; a refused `begin` reports nothing.
#[test]
fn begin_reports_each_admission_once_with_its_fixed_timing() {
    let (mut rig, _, log, admitted) = holding_rig();
    let create = rig.driver.begin(create("s1")).unwrap();
    while rig.pump().more {
        rig.drain_events();
    }
    log.lock().unwrap().clear();
    // A2-1, R-20: a `Resize` of a `Created` session has fixed timing.
    let resize = rig
        .driver
        .begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 30,
                cols: 90,
                cell_px: None,
            },
        })
        .unwrap();
    assert!(rig.driver.begin(Op::Start { id: sid("s9") }).is_err());
    assert_eq!(
        *admitted.lock().unwrap(),
        [(create.0, false), (resize.0, true)]
    );
    assert_eq!(*log.lock().unwrap(), [], "begin asks no choice point");
}
