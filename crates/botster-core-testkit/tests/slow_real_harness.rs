//! `RealCoreHarness` and its guarded launch wrappers with real processes (plan 4.2; lead ruling 2026-10-04 on RealCoreHarness).
//! Slow tier: real `Core::open`, the prebuilt `botster-worker`, `botster-conformance-probe` and `botster-test-anchor`.
//!
//! Clause: Core A5-4 (the suite runs on real Core with real worker processes); BUILD.md testing rule 10 (every real-process
//! test owns its groups on every exit path); Core LC-4, LC-5, LC-7 and AD-2 through the wrappers (the wrapped worker and
//! program keep their pid, group, exit status and signals).
#![cfg(feature = "slow")]

mod common;

use botster_core_contract::prelude::*;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_testkit::anchor::{Line, Report};
use botster_core_testkit::process_group::OwnedGroup;
use botster_core_testkit::real::AnchorGuard;
use botster_probe_script::Step;
use common::{completed, pump_until, sid, state, HOST_DEADLINE};
use rustix::process::{getpgid, kill_process, Pid, Signal};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

const FIXTURE: &str = env!("CARGO_BIN_EXE_botster-test-fixture");

fn pid(raw: u32) -> Pid {
    Pid::from_raw(i32::try_from(raw).unwrap()).unwrap()
}

fn pumped_to(
    core: &mut dyn CoreApi,
    mut events: Vec<Event>,
    id: &SessionId,
    reached: impl Fn(SessionState) -> bool,
) -> SessionState {
    if !state(&events, id).is_some_and(&reached) {
        events.extend(pump_until(core, |e| state(e, id).is_some_and(&reached)));
    }
    state(&events, id).expect("a state")
}

fn run(core: &mut dyn CoreApi, op: Op) -> OpResult {
    let op = core.begin(op).expect("admitted");
    let events = pump_until(core, |e| completed(e, op).is_some());
    completed(&events, op).expect("completed")
}

/// The reports that appeared since `before`.
fn new_reports(all: Vec<Report>, before: &[Report]) -> Vec<Report> {
    all.into_iter().filter(|r| !before.contains(r)).collect()
}

/// Every process that a report names has ended: the anchor and the real binary.
fn all_ended(reports: &[Report]) {
    for report in reports {
        for identity in [report.leader, report.anchor] {
            common::wait_exit(identity, common::cleanup_deadline())
                .unwrap_or_else(|why| panic!("{why}; report {report:?}"));
        }
    }
}

/// Lead ruling item 15 and Core LC-4, LC-5, LC-7, AD-2: Core runs the real worker and the real probe through the wrappers and
/// observes their real exit paths. The wrapped worker and payload each lead their own group, as Core and the worker start
/// them. A payload's exit code reaches Core; `Stop` ends a payload with `TERM`; the end of a worker process, killed by its
/// reported pid, is `Lost(WorkerGone)`; `Remove` completes for each. Then every anchor ends its group with `KILL`, and every
/// process of the run ends.
#[test]
fn core_observes_the_real_worker_and_probe_through_the_wrappers() {
    let candidate = common::candidate();
    let mut harness = common::harness();
    let mut core = common::open(&mut harness);
    let code = 7;

    let events = common::start(&harness, core.as_mut(), "a", vec![Step::Exit { code }]);
    let reports_a = harness.await_anchors(2).unwrap();
    let ended = pumped_to(core.as_mut(), events, &sid("a"), |s| {
        matches!(s, SessionState::Exited(_))
    });
    assert_eq!(
        ended,
        SessionState::Exited(Exit {
            code: Some(code),
            signal: None,
            cause: ExitCause::Normal,
        })
    );
    let mut binaries: Vec<&Path> = reports_a.iter().map(|r| r.binary.as_path()).collect();
    binaries.sort();
    let mut expected = vec![candidate.worker.as_path(), candidate.probe.as_path()];
    expected.sort();
    assert_eq!(
        binaries, expected,
        "one worker and one payload: {reports_a:?}"
    );
    for report in &reports_a {
        assert_eq!(
            report.group, report.leader.pid,
            "a wrapped process leads its group: {report:?}"
        );
    }

    common::start(&harness, core.as_mut(), "b", vec![Step::Hold {}]);
    let stop = run(core.as_mut(), Op::Stop { id: sid("b") });
    let OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) = stop else {
        panic!("{stop:?}");
    };
    assert_eq!(exit.signal, Some(Signal::TERM.as_raw()), "{exit:?}");

    let before = harness.await_anchors(4).unwrap();
    let events = common::start(&harness, core.as_mut(), "c", vec![Step::Hold {}]);
    let reports_c = new_reports(harness.await_anchors(6).unwrap(), &before);
    let worker = reports_c
        .iter()
        .find(|r| r.binary == candidate.worker)
        .expect("the worker of c");
    // The anchor holds the worker's group, whose id is the worker's pid: that pid cannot name another process meanwhile.
    kill_process(pid(worker.leader.pid), Signal::KILL).unwrap();
    let lost = pumped_to(core.as_mut(), events, &sid("c"), |s| {
        matches!(s, SessionState::Lost(_))
    });
    assert_eq!(lost, SessionState::Lost(LostReason::WorkerGone));

    for id in ["a", "b", "c"] {
        let removed = run(core.as_mut(), Op::Remove { id: sid(id) });
        assert!(matches!(removed, OpResult::Ok(_)), "{id}: {removed:?}");
    }
    drop(core);
    let reports = harness.await_anchors(6).unwrap();
    assert_eq!(reports.len(), 6, "a worker and a payload for each session");
    let finished = harness.finish().unwrap();
    assert_eq!(finished.len(), reports.len());
    for f in &finished {
        assert!(f.killed(), "{f:?}");
    }
    all_ended(&reports);
}

/// BUILD.md testing rule 10: a test that panics while its session runs leaves no process. The payload and its child ignore
/// `TERM`, so only the group `KILL` ends them. The harness's drop runs during the unwind.
#[test]
fn a_panic_ends_every_group_of_the_harness() {
    let mut reports = Vec::new();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut harness = common::harness();
        let mut core = common::open(&mut harness);
        common::start(&harness, core.as_mut(), "s", common::stubborn(None));
        reports = harness.await_anchors(2).unwrap();
        panic!("the test failed while its session ran");
    }));
    assert!(unwound.is_err());
    assert_eq!(reports.len(), 2, "{reports:?}");
    all_ended(&reports);
}

/// Spawns a fixture role in a group that this test owns, and reads its lines up to `ready`. The returned stdin keeps the
/// fixture waiting.
fn fixture(role: &str) -> (OwnedGroup, std::process::ChildStdin, Vec<String>) {
    let mut command = Command::new(FIXTURE);
    command
        .arg(role)
        .arg(common::candidate_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut group = OwnedGroup::spawn(command).unwrap();
    let stdin = group.take_stdin().unwrap();
    let mut lines = common::lines_until(
        group.take_stdout().unwrap(),
        |l| l == "ready",
        HOST_DEADLINE,
    );
    lines.pop();
    (group, stdin, lines)
}

/// BUILD.md testing rule 10: a test process that is killed, so that none of its cleanup runs, leaves no process. The kernel
/// closes its guard, and each anchor ends its group.
#[test]
fn a_killed_test_leaves_no_process() {
    let (mut group, _stdin, lines) = fixture("own-harness");
    let reports: Vec<Report> = lines
        .iter()
        .map(|l| match Line::decode(l) {
            Ok(Line::Anchor(report)) => report,
            other => panic!("{l}: {other:?}"),
        })
        .collect();
    assert_eq!(reports.len(), 2, "{reports:?}");
    group.kill();
    all_ended(&reports);
}

/// A guard that dies before it accepted its anchor's connection still ends the group: the connection is reset when the
/// listener closes, and the anchor takes the reset as the end of its guard.
#[test]
fn an_owner_that_dies_before_it_registers_its_anchor_still_ends_the_group() {
    let (mut group, _stdin, lines) = fixture("own-guard");
    let [line] = &lines[..] else {
        panic!("{lines:?}");
    };
    let value: serde_json::Value = serde_json::from_str(line).unwrap();
    let leader = ProcessIdentity {
        pid: u32::try_from(value["pid"].as_u64().unwrap()).unwrap(),
        start_time: value["start_time"].as_u64().unwrap(),
    };
    group.kill();
    // The probe and its child ignore `TERM`: the anchor's `KILL` ends them.
    common::wait_exit(leader, common::cleanup_deadline()).unwrap();
}

/// Lead ruling item 13: when the real binary has moved to another group, the anchor reports it and signals nothing. The
/// wrapper runs inside a group that a holder process of this test leads; the fixture then moves itself out.
#[test]
fn an_anchor_refuses_a_group_that_its_real_binary_left() {
    let candidate = common::candidate();
    let mut guard = AnchorGuard::new(&candidate.anchor).unwrap();
    let wrapper = guard
        .wrapper(
            "fixture",
            Path::new(FIXTURE),
            CoreLimits::default().stop_grace,
        )
        .unwrap();
    let mut holder = OwnedGroup::spawn({
        let mut cat = Command::new("/bin/cat");
        cat.stdin(Stdio::piped()).stdout(Stdio::null());
        cat
    })
    .unwrap();
    let _holder_input = holder.take_stdin();
    let holder_group = i32::try_from(holder.pid()).unwrap();
    let mut moved = OwnedChild(
        Command::new(&wrapper)
            .arg("move-group")
            .process_group(holder_group)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    common::lines_until(
        moved.0.stdout.take().unwrap(),
        |l| l == "moved",
        HOST_DEADLINE,
    );
    let finished = guard.finish().unwrap();
    let [only] = &finished[..] else {
        panic!("{finished:?}");
    };
    assert_eq!(only.report.as_ref().map(|r| r.group), Some(holder.pid()));
    assert!(
        matches!(only.lines.last(), Some(Line::Refused { .. }))
            && !only.lines.contains(&Line::Term),
        "{finished:?}"
    );
    assert_eq!(
        moved.0.try_wait().unwrap(),
        None,
        "the moved process was signalled"
    );
    assert!(
        !holder.leader_exited().unwrap(),
        "the holder's group was signalled"
    );
}

/// A child that this test kills and reaps on every exit path.
struct OwnedChild(std::process::Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The first line that a fixture role prints, from a group that this test owns.
fn first_line(command: Command) -> (OwnedGroup, String) {
    let mut group = OwnedGroup::spawn(command).unwrap();
    let mut lines = common::lines_until(group.take_stdout().unwrap(), |_| true, HOST_DEADLINE);
    (group, lines.remove(0))
}

fn fds_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .arg("fds")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

/// Lead ruling items 2 and 5: the real binary runs only after its anchor holds the group, and it has the descriptors that
/// it has without the wrapper: the guard connection and the anchor's pipe do not reach it.
#[test]
fn the_real_binary_runs_after_its_anchor_with_its_own_descriptors() {
    let candidate = common::candidate();
    let mut guard = AnchorGuard::new(&candidate.anchor).unwrap();
    let wrapper = guard
        .wrapper(
            "fixture",
            Path::new(FIXTURE),
            CoreLimits::default().stop_grace,
        )
        .unwrap();
    let (_direct, without) = first_line(fds_command(Path::new(FIXTURE)));
    let (wrapped, with) = first_line(fds_command(&wrapper));
    assert_eq!(with, without);
    let reports = guard.reports().unwrap();
    let [report] = &reports[..] else {
        panic!("{reports:?}");
    };
    assert_eq!(report.leader.pid, wrapped.pid());
    assert_eq!(report.group, wrapped.pid());
    assert_eq!(report.binary, Path::new(FIXTURE));
    let anchor_group = getpgid(Some(pid(report.anchor.pid))).unwrap();
    assert_eq!(
        anchor_group,
        pid(report.group),
        "the anchor is a member of the group"
    );
    let finished = guard.finish().unwrap();
    assert!(finished.iter().all(|f| f.killed()), "{finished:?}");
}
