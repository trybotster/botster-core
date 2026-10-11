//! Steward ruling R-78 item 4 in the production driver: a send error closes only the link's write side. The link is read on,
//! so the frames that the worker wrote before it ended are handled; its end of file closes it.

use super::deadlines::run_session;
use super::*;

const GRACE: Duration = Duration::from_millis(100);

fn link(rig: &Rig) -> std::sync::MutexGuard<'_, Mock> {
    rig.mock.lock().unwrap()
}

fn fail_next_send(rig: &Rig) {
    link(rig)
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .fail_send
        .push(io::ErrorKind::BrokenPipe);
}

fn peer_closes(rig: &Rig) {
    link(rig).links.get_mut(&LinkId(1)).unwrap().peer_closed = true;
}

fn exited(code: i32) -> WorkerMsg {
    WorkerMsg::Exited {
        code: Some(code),
        signal: None,
    }
}

/// The request number of the first op that the host sent on link 1.
fn first_request(rig: &Rig) -> u64 {
    rig.host_frames(LinkId(1))
        .iter()
        .find_map(|(k, p)| match HostMsg::decode(p) {
            Ok(HostMsg::Op { req, .. }) if *k == FrameType::HOST_MSG => Some(req),
            _ => None,
        })
        .expect("an op was sent")
}

fn modes_done(req: u64) -> WorkerMsg {
    WorkerMsg::Done {
        req,
        result: OpResult::Ok(OpOutput::Modes(Modes {
            flags: crate::tests::terminal_state().modes,
            model_rev: ModelRev(1),
        })),
    }
}

/// Pumps until `ops` completed, and returns their results.
fn results(rig: &mut Rig, ops: &[OpId], seen: &mut Vec<Event>) -> BTreeMap<OpId, OpResult> {
    for _ in 0..50 {
        rig.pump();
        seen.extend(rig.drain_events());
        let found: BTreeMap<OpId, OpResult> = seen
            .iter()
            .filter_map(|e| match e {
                Event::Completed { op, result } if ops.contains(op) => Some((*op, result.clone())),
                _ => None,
            })
            .collect();
        if found.len() == ops.len() {
            return found;
        }
    }
    panic!("not every op completed: {seen:?}");
}

fn is_exit(result: &OpResult, code: i32) -> bool {
    matches!(result, OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) if exit.code == Some(code))
}

/// R-78 test 10: the send of the `Stop` fails, then the worker's `Exited` and its end of file arrive. The frame is read and
/// decides the end; the link closes once, at the end of file.
#[test]
fn a_frame_after_a_send_error_is_read_before_the_end_of_file() {
    let mut rig = Rig::new(CoreLimits::default());
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    fail_next_send(&rig);
    let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    rig.pump();
    assert!(!link(&rig).links[&LinkId(1)].closed_by_host);
    rig.worker_says(LinkId(1), exited(7));
    peer_closes(&rig);
    let mut seen = Vec::new();
    let done = results(&mut rig, &[stop], &mut seen);
    assert!(is_exit(&done[&stop], 7), "{:?}", done[&stop]);
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 test 11: test 10 with a pump bound of one event. The frame is read in a later pump, and each pump reports more work
/// until it is read.
#[test]
fn a_frame_after_a_send_error_is_read_within_the_pump_bound() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    fail_next_send(&rig);
    let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    rig.pump();
    for _ in 0..3 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: Observation::Bell,
            },
        );
    }
    rig.worker_says(LinkId(1), exited(7));
    peer_closes(&rig);
    let report = rig.pump();
    assert!(report.more, "the frames wait for later pumps");
    assert!(!link(&rig).links[&LinkId(1)].closed_by_host);
    let mut seen = Vec::new();
    let done = results(&mut rig, &[stop], &mut seen);
    assert!(is_exit(&done[&stop], 7), "{:?}", done[&stop]);
}

/// R-78 test 15: a live worker whose send fails and whose link stays open with nothing to read. At the grace the identity
/// check finds it alive: the stop completes `Lost(WorkerUnreachable)` under the existing grace, with no end of file.
#[test]
fn a_live_peer_with_a_failed_send_is_unreachable_at_the_stop_grace() {
    let mut rig = Rig::new(limits(|l| l.stop_grace = GRACE));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    fail_next_send(&rig);
    let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    for _ in 0..4 {
        rig.pump();
    }
    rig.now += GRACE;
    let mut seen = Vec::new();
    let done = results(&mut rig, &[stop], &mut seen);
    assert_eq!(
        done[&stop],
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(
            LostReason::WorkerUnreachable
        )))
    );
    assert!(!link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 tests 16 and 17: the exit report and a failed send, in both orders, then an `Exited` and a `Done` that the pump
/// bound delays, and the stop's grace before the end of file. Nothing is classified at the grace and no request fails
/// early; the drained frames decide the results.
#[test]
fn an_exit_and_a_failed_send_keep_the_drain_past_the_grace_in_either_order() {
    for exit_first in [true, false] {
        let mut rig = Rig::new(limits(|l| {
            l.pump_events = 1;
            l.stop_grace = GRACE;
            l.mandatory_events = 64;
        }));
        run_session(&mut rig, "s1", LinkId(1));
        rig.drain_events();
        let read = rig
            .driver
            .begin(Op::ReadModeFlags { session: sid("s1") })
            .unwrap();
        let stop = rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
        let mut seen = Vec::new();
        for _ in 0..6 {
            rig.pump();
            seen.extend(rig.drain_events());
        }
        let req = first_request(&rig);
        let exit = (
            ProcessIdentity {
                pid: 500,
                start_time: 1,
            },
            ExitStatus::Code(0),
        );
        // The send that fails: a second read, which fails at once.
        let failing = |rig: &mut Rig, seen: &mut Vec<Event>| {
            fail_next_send(rig);
            rig.driver
                .begin(Op::ReadModeFlags { session: sid("s1") })
                .unwrap();
            rig.pump();
            seen.extend(rig.drain_events());
        };
        if exit_first {
            link(&rig).exits.push(exit);
            rig.pump();
            seen.extend(rig.drain_events());
            failing(&mut rig, &mut seen);
        } else {
            failing(&mut rig, &mut seen);
            link(&rig).exits.push(exit);
            rig.pump();
            seen.extend(rig.drain_events());
        }
        for _ in 0..3 {
            rig.worker_says(
                LinkId(1),
                WorkerMsg::Observed {
                    observation: Observation::Bell,
                },
            );
        }
        rig.worker_says(LinkId(1), modes_done(req));
        rig.worker_says(LinkId(1), exited(8));
        peer_closes(&rig);
        rig.now += GRACE;
        rig.pump();
        seen.extend(rig.drain_events());
        assert_eq!(
            rig.driver.get(&sid("s1")).unwrap().state,
            SessionState::Stopping,
            "exit first {exit_first}: nothing is classified at the grace"
        );
        assert!(
            !seen
                .iter()
                .any(|e| matches!(e, Event::Completed { op, .. } if *op == read)),
            "exit first {exit_first}: no early request failure"
        );
        let done = results(&mut rig, &[read, stop], &mut seen);
        assert!(
            matches!(done[&read], OpResult::Ok(OpOutput::Modes(_))),
            "exit first {exit_first}: {:?}",
            done[&read]
        );
        assert!(
            is_exit(&done[&stop], 8),
            "exit first {exit_first}: {:?}",
            done[&stop]
        );
        assert!(link(&rig).links[&LinkId(1)].closed_by_host);
    }
}
