//! Steward ruling R-78 item 4 in the production driver: a send error closes only the link's write side. The link is read on,
//! so the frames that the worker wrote before it ended are handled; its end of file closes it. After the worker's process
//! ended (the steward's follow-up), the driver reads the link without waiting and closes it at the first would-block.

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

/// R-78 tests 16 and 17: the worker wrote a `Done` and an `Exited` behind three observations, then ended. The exit report and
/// a failed send come in both orders, and the stop's grace comes while the pump bound still delays the frames. No end of
/// file comes. Nothing is classified at the grace and no request fails early; the drained frames decide the results, and
/// the host closes the drained link.
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
        let exit = ProcessIdentity {
            pid: 500,
            start_time: 1,
        };
        // The send that fails: a second read, which fails at once.
        let failing = |rig: &mut Rig, seen: &mut Vec<Event>| {
            fail_next_send(rig);
            rig.driver
                .begin(Op::ReadModeFlags { session: sid("s1") })
                .unwrap();
            rig.pump();
            seen.extend(rig.drain_events());
        };
        // The worker wrote its last frames before it ended: a process writes nothing after its exit. No end of file comes.
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

/// R-78 item 4 (0179): the end is chosen before the exit report, and frames still wait behind it. The worker wrote `Exited`,
/// then the `Done` of a read that it served from its final model (LC-5), then the route's `RouteClosed` (`report_exit` queues
/// `Exited` before `routes_session_ended` queues the closes). With a pump bound of one event, `Exited` is handled first and
/// the exit report comes next. No end of file comes. The host still reads what the link holds without waiting: the read
/// completes, the route closes `SessionEnded`, the end stays `Exited`, and the host closes the drained link.
#[test]
fn frames_after_a_chosen_end_are_read_without_an_end_of_file() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    let attached = super::handoff::attach(&mut rig, Arc::new(1));
    let read = rig
        .driver
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    for _ in 0..6 {
        rig.pump();
    }
    rig.worker_says(
        LinkId(1),
        WorkerMsg::RouteReceived {
            route: attached.route,
        },
    );
    let req = first_request(&rig);
    let mut seen = rig.drain_events();
    rig.worker_says(LinkId(1), exited(7));
    rig.worker_says(LinkId(1), modes_done(req));
    rig.worker_says(
        LinkId(1),
        WorkerMsg::RouteClosed {
            route: attached.route,
            reason: RouteCloseReason::SessionEnded {
                exit: Exit {
                    code: Some(7),
                    signal: None,
                    cause: ExitCause::Normal,
                },
            },
            route_tag: None,
        },
    );
    rig.pump();
    seen.extend(rig.drain_events());
    link(&rig).exits.push(ProcessIdentity {
        pid: 500,
        start_time: 1,
    });
    rig.pump();
    seen.extend(rig.drain_events());
    let done = results(&mut rig, &[read], &mut seen);
    assert!(
        matches!(done[&read], OpResult::Ok(OpOutput::Modes(_))),
        "the Done behind Exited is read: {:?}",
        done[&read]
    );
    for _ in 0..10 {
        rig.pump();
        seen.extend(rig.drain_events());
    }
    let closes: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            Event::RouteClosed { route, reason, .. } if *route == attached.route => Some(*reason),
            _ => None,
        })
        .collect();
    assert!(
        matches!(closes[..], [RouteCloseReason::SessionEnded { .. }]),
        "the RouteClosed behind Exited is read, once: {closes:?}"
    );
    assert!(
        matches!(rig.driver.get(&sid("s1")).unwrap().state, SessionState::Exited(exit) if exit.code == Some(7)),
        "{:?}",
        rig.driver.get(&sid("s1")).unwrap().state
    );
    assert!(
        link(&rig).links[&LinkId(1)].closed_by_host,
        "the drain ends at the would-block: the host closes the link"
    );
}

fn worker_exits(rig: &Rig) {
    link(rig).exits.push(ProcessIdentity {
        pid: 500,
        start_time: 1,
    });
}

/// R-78 item 4, steward follow-up: a running worker wrote `Exited` and ended, and its link gives no end of file (a peer may
/// hold a copy of it). The exit report drains the link without waiting: the session ends `Exited`, and the host closes the
/// link.
#[test]
fn an_ended_worker_s_exited_frame_is_read_without_an_end_of_file() {
    let mut rig = Rig::new(limits(|_| {}));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig.worker_says(LinkId(1), exited(3));
    worker_exits(&rig);
    let report = rig.pump();
    assert!(
        matches!(rig.driver.get(&sid("s1")).unwrap().state, SessionState::Exited(exit) if exit.code == Some(3)),
        "{:?}",
        rig.driver.get(&sid("s1")).unwrap().state
    );
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
    assert!(!report.more, "nothing is left to drain");
}

/// R-78 item 4, steward follow-up: a running worker ended with nothing written, and its link gives no end of file. The
/// first read would block. The pump of the exit report marks the link for its drain before it reads the links, so that
/// pump's read closes the link and its work loop loses the session `WorkerGone`: nothing waits for a later pump.
#[test]
fn an_ended_worker_with_nothing_written_is_gone_at_once() {
    let mut rig = Rig::new(limits(|_| {}));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    worker_exits(&rig);
    rig.pump();
    assert!(
        link(&rig).links[&LinkId(1)].closed_by_host,
        "the drain does not wait"
    );
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 item 4, steward follow-up: a trailing partial frame is a truncated link. The complete `Done` before it is handled;
/// the part of an `Exited` decides nothing, so the session is lost `WorkerGone`. No end of file comes.
#[test]
fn a_partial_frame_at_the_exit_is_a_truncated_link() {
    let mut rig = Rig::new(limits(|_| {}));
    run_session(&mut rig, "s1", LinkId(1));
    let read = rig
        .driver
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    for _ in 0..4 {
        rig.pump();
    }
    let req = first_request(&rig);
    let mut seen = rig.drain_events();
    rig.worker_says(LinkId(1), modes_done(req));
    let mut payload = Vec::new();
    exited(5).encode(&mut payload);
    let whole = frame(FrameType::WORKER_MSG, &payload);
    link(&rig)
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .to_host
        .extend(&whole[..whole.len() - 1]);
    worker_exits(&rig);
    let done = results(&mut rig, &[read], &mut seen);
    assert!(
        matches!(done[&read], OpResult::Ok(OpOutput::Modes(_))),
        "{:?}",
        done[&read]
    );
    rig.settle();
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 item 4 and Core EV-5b (Sol, 0179): a frame of the exit drain that needs mandatory room waits while the queue is
/// full. The drain does not spin: with only that frame blocked, the pump reports no more work. The poll that frees room
/// restores the link, and the next pumps deliver the held frame and the `Exited` behind it, then close the drained link.
#[test]
fn a_drain_blocked_on_mandatory_room_waits_for_the_poll() {
    let mut rig = Rig::new(limits(|l| {
        l.mandatory_events = 3;
        l.max_sessions = 4;
    }));
    // `Created`, `Starting`, `Running` fill the queue of three; the route event behind them needs room.
    super::start_session(&mut rig, "s1", LinkId(1));
    rig.worker_says(LinkId(1), WorkerMsg::RouteStalled { route: RouteId(1) });
    rig.worker_says(LinkId(1), exited(3));
    worker_exits(&rig);
    let report = rig.pump();
    assert!(
        !report.more,
        "a drain blocked on mandatory room is not runnable work (TM-6)"
    );
    assert!(
        !link(&rig).links[&LinkId(1)].closed_by_host,
        "the drain waits for room"
    );
    assert_eq!(
        rig.driver.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    rig.drain_events();
    assert!(
        rig.wake_set(),
        "the poll that freed room signalled the wake"
    );
    rig.settle();
    assert!(
        matches!(rig.driver.get(&sid("s1")).unwrap().state, SessionState::Exited(exit) if exit.code == Some(3)),
        "{:?}",
        rig.driver.get(&sid("s1")).unwrap().state
    );
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 item 4 and TM-6: the reaper queues the exit while the pump settles, so the pump takes it after its last read of the
/// link. The drain has not run, so it is runnable work: the pump reports more, and the next pump drains and closes the link.
#[test]
fn an_exit_taken_at_the_settle_leaves_the_drain_runnable() {
    let mut rig = Rig::new(limits(|_| {}));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig.worker_says(LinkId(1), exited(6));
    link(&rig).late_exits.push(ProcessIdentity {
        pid: 500,
        start_time: 1,
    });
    let report = rig.pump();
    assert!(report.more, "the undrained link is runnable work");
    assert!(rig.wake_set(), "the wake stays set for the drain");
    rig.settle();
    assert!(
        matches!(rig.driver.get(&sid("s1")).unwrap().state, SessionState::Exited(exit) if exit.code == Some(6)),
        "{:?}",
        rig.driver.get(&sid("s1")).unwrap().state
    );
    assert!(link(&rig).links[&LinkId(1)].closed_by_host);
}

/// R-78 item 4, 9B and TM-6: the pump bound delays the exit drain. Each pump that stops at the bound reports more work, and
/// the pumps go on until the drain reads the `Exited` and closes the link.
#[test]
fn a_drain_delayed_by_the_pump_bound_stays_runnable() {
    let mut rig = Rig::new(limits(|l| {
        l.pump_events = 1;
        l.mandatory_events = 64;
    }));
    run_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    for _ in 0..3 {
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Observed {
                observation: Observation::Bell,
            },
        );
    }
    rig.worker_says(LinkId(1), exited(9));
    worker_exits(&rig);
    let mut reports = Vec::new();
    while !link(&rig).links[&LinkId(1)].closed_by_host {
        let report = rig.pump();
        rig.drain_events();
        reports.push(report.more);
        assert!(
            report.more || link(&rig).links[&LinkId(1)].closed_by_host,
            "a delayed drain is runnable: {reports:?}"
        );
        assert!(reports.len() < 20, "the drain never ended: {reports:?}");
    }
    assert!(
        reports.len() > 1,
        "the bound delayed the drain: {reports:?}"
    );
    rig.settle();
    assert!(
        matches!(rig.driver.get(&sid("s1")).unwrap().state, SessionState::Exited(exit) if exit.code == Some(9)),
        "{:?}",
        rig.driver.get(&sid("s1")).unwrap().state
    );
}
