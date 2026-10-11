//! Steward ruling R-78 items 3 and 4: a stop whose link is broken asks the worker's identity at its grace, and a gone
//! worker's link is read to its end of file before the session is classified.

use super::*;
use botster_core_edges::edges::GroupSignal;

const GRACE: Duration = Duration::from_millis(100);

fn world() -> World {
    let mut w = World::new(limits(|l| l.stop_grace = GRACE));
    w.autopilot = Autopilot::Silent;
    w
}

fn state(w: &World) -> SessionState {
    w.engine.get(&sid("s1")).unwrap().state
}

fn exited_frame(code: i32) -> WorkerMsg {
    WorkerMsg::Exited {
        code: Some(code),
        signal: None,
    }
}

fn read(w: &mut World) -> OpId {
    let op = w
        .engine
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    w.pump();
    op
}

fn done(w: &mut World) {
    let req = w.last_request("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Modes(Modes {
                flags: terminal_state().modes,
                model_rev: ModelRev(1),
            })),
        },
    );
}

fn is_exit(state: SessionState, code: i32) -> bool {
    matches!(state, SessionState::Exited(exit) if exit.code == Some(code))
}

fn completed(w: &mut World, op: OpId) -> Option<OpResult> {
    w.engine.poll_events(256).into_iter().find_map(|e| match e {
        Event::Completed { op: done, result } if done == op => Some(result),
        _ => None,
    })
}

/// R-78 item 3: the exit watch reaped the worker, and its report still waits in the edge when the grace of a stop with a
/// broken link ends. The identity check decides the reason: the worker is gone, never unreachable. The late report
/// changes nothing.
#[test]
fn a_reaped_exit_still_queued_at_the_stop_grace_is_worker_gone() {
    let mut w = world();
    w.running("s1");
    let worker = w.identity_of("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    // Reaped, and not reported yet.
    w.alive.remove(&worker);
    w.advance(GRACE);
    w.pump();
    w.feed(Input::ProcessExited {
        identity: worker,
        status: ExitStatus::Code(0),
    });
    assert_eq!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(LostReason::WorkerGone)))
    );
    assert_eq!(state(&w), SessionState::Lost(LostReason::WorkerGone));
}

/// R-78 item 3, the other order: the exit report comes before the grace of a stop with a broken link.
#[test]
fn an_exit_reported_before_the_stop_grace_is_worker_gone() {
    let mut w = world();
    w.running("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    w.exited("s1");
    w.advance(GRACE);
    w.pump();
    assert_eq!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(LostReason::WorkerGone)))
    );
}

/// R-78 item 4: the worker wrote `Exited` before it ended, and the host sees the exit before it reads the frame. The link
/// stays open; the frame decides the end, and the end of file changes nothing.
#[test]
fn an_exited_frame_read_after_the_exit_report_ends_exited() {
    let mut w = world();
    w.running("s1");
    let link = w.link_of("s1");
    w.exited("s1");
    w.pump();
    assert_eq!(state(&w), SessionState::Running, "the link is read first");
    assert!(!w.closed.contains(&link), "the link stays open");
    w.worker_says("s1", exited_frame(3));
    w.feed(Input::LinkClosed { link });
    w.pump();
    assert!(is_exit(state(&w), 3), "{:?}", state(&w));
}

/// R-78 item 4: a gone worker whose link ends with no `Exited` is `Lost(WorkerGone)` at the end of file.
#[test]
fn a_gone_worker_with_no_word_is_worker_gone_at_the_end_of_file() {
    let mut w = world();
    w.running("s1");
    let link = w.link_of("s1");
    w.exited("s1");
    w.pump();
    assert_eq!(state(&w), SessionState::Running);
    w.feed(Input::LinkClosed { link });
    w.pump();
    assert_eq!(state(&w), SessionState::Lost(LostReason::WorkerGone));
}

/// R-78 item 4 in `Stopping`: the grace ends between the exit and the end of file. Nothing is classified at the grace, no
/// gone worker is signalled, and the frame read later decides the end.
#[test]
fn a_stop_grace_between_the_exit_and_the_end_of_file_keeps_the_drain() {
    for frame in [true, false] {
        let mut w = world();
        w.running("s1");
        let link = w.link_of("s1");
        let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
        w.pump();
        w.exited("s1");
        w.advance(GRACE);
        w.pump();
        assert_eq!(state(&w), SessionState::Stopping, "no class at the grace");
        assert!(w.signals.is_empty(), "a gone worker is never signalled");
        if frame {
            w.worker_says("s1", exited_frame(4));
        }
        w.feed(Input::LinkClosed { link });
        let result = w.complete(stop);
        if frame {
            assert!(
                matches!(&result, OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) if exit.code == Some(4)),
                "{result:?}"
            );
        } else {
            assert_eq!(
                result,
                OpResult::Ok(OpOutput::End(SessionEnd::Lost(LostReason::WorkerGone)))
            );
        }
    }
}

/// R-78 item 4: a `Done` read in the drain completes its request; a request with no `Done` fails at the end of file, not at
/// the exit.
#[test]
fn a_done_read_in_the_drain_completes_its_request() {
    let mut w = world();
    w.running("s1");
    let link = w.link_of("s1");
    let answered = read(&mut w);
    let unanswered = read(&mut w);
    w.exited("s1");
    w.pump();
    assert!(
        completed(&mut w, unanswered).is_none(),
        "no failure at the exit"
    );
    // The worker answered the first request before it ended.
    let req = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Op { req, .. } => Some(*req),
            _ => None,
        })
        .unwrap();
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Modes(Modes {
                flags: terminal_state().modes,
                model_rev: ModelRev(1),
            })),
        },
    );
    w.feed(Input::LinkClosed { link });
    let results = w.complete_all(&[answered, unanswered]);
    assert!(matches!(
        results[&answered],
        OpResult::Ok(OpOutput::Modes(_))
    ));
    assert!(matches!(
        &results[&unanswered],
        OpResult::Err(e) if e.code == ErrorCode::WorkerLinkFailed
    ));
}

/// R-78 item 4: after a write failure nothing more is sent, and a new request fails at once; the request in flight waits
/// for its `Done`.
#[test]
fn a_write_failure_fails_new_requests_and_keeps_the_ones_in_flight() {
    let mut w = world();
    w.running("s1");
    let link = w.link_of("s1");
    let in_flight = read(&mut w);
    w.feed(Input::LinkWriteFailed { link });
    let sent = w.sent.len();
    let new = w
        .engine
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    assert!(matches!(
        w.complete(new),
        OpResult::Err(e) if e.code == ErrorCode::WorkerLinkFailed
    ));
    assert_eq!(w.sent.len(), sent, "nothing more is sent");
    assert!(completed(&mut w, in_flight).is_none());
    done(&mut w);
    assert!(matches!(
        w.complete(in_flight),
        OpResult::Ok(OpOutput::Modes(_))
    ));
    assert_eq!(state(&w), SessionState::Running);
}

/// R-78 test 15: a live worker whose link's write side failed, and whose link never ends. The stop signals the verified
/// worker, and at the grace the identity check finds it alive: `Lost(WorkerUnreachable)` under the existing grace.
#[test]
fn a_live_worker_with_a_failed_write_side_is_unreachable_at_the_stop_grace() {
    let mut w = world();
    w.running("s1");
    let worker = w.identity_of("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkWriteFailed { link });
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert!(w.signals.contains(&(worker, GroupSignal::EndPayload)));
    w.advance(GRACE);
    w.pump();
    assert_eq!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(
            LostReason::WorkerUnreachable
        )))
    );
    assert!(!w.closed.contains(&link), "no end of file came");
}

/// R-78 (Sol): the identity check at the grace finds the worker gone while its link can still be read. The drain goes on;
/// the `Exited` read later decides the end.
#[test]
fn a_probe_that_finds_the_worker_gone_keeps_the_drain() {
    let mut w = world();
    w.running("s1");
    let worker = w.identity_of("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkWriteFailed { link });
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    w.alive.remove(&worker);
    w.advance(GRACE);
    w.pump();
    assert_eq!(state(&w), SessionState::Stopping, "the link is read first");
    w.worker_says("s1", exited_frame(5));
    w.feed(Input::LinkClosed { link });
    let result = w.complete(stop);
    assert!(
        matches!(&result, OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) if exit.code == Some(5)),
        "{result:?}"
    );
}

/// R-78 tests 16 and 17 in the engine: the exit report and the write failure in both orders, then the grace before the end
/// of file. Nothing is classified and no request fails before the drain; the `Done` and the `Exited` read in the drain
/// decide the results.
#[test]
fn an_exit_and_a_write_failure_in_either_order_keep_the_drain_past_the_grace() {
    for exit_first in [true, false] {
        let mut w = world();
        w.running("s1");
        let link = w.link_of("s1");
        let in_flight = read(&mut w);
        let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
        w.pump();
        if exit_first {
            w.exited("s1");
            w.feed(Input::LinkWriteFailed { link });
        } else {
            w.feed(Input::LinkWriteFailed { link });
            w.exited("s1");
        }
        w.advance(GRACE);
        w.pump();
        assert_eq!(
            state(&w),
            SessionState::Stopping,
            "exit first: {exit_first}"
        );
        assert!(
            completed(&mut w, in_flight).is_none(),
            "exit first: {exit_first}"
        );
        assert!(w.signals.is_empty(), "exit first: {exit_first}");
        let req = w
            .sent
            .iter()
            .find_map(|(_, m)| match m {
                HostMsg::Op { req, .. } => Some(*req),
                _ => None,
            })
            .unwrap();
        w.worker_says(
            "s1",
            WorkerMsg::Done {
                req,
                result: OpResult::Ok(OpOutput::Modes(Modes {
                    flags: terminal_state().modes,
                    model_rev: ModelRev(1),
                })),
            },
        );
        w.worker_says("s1", exited_frame(6));
        w.feed(Input::LinkClosed { link });
        let results = w.complete_all(&[in_flight, stop]);
        assert!(matches!(
            results[&in_flight],
            OpResult::Ok(OpOutput::Modes(_))
        ));
        assert!(
            matches!(&results[&stop], OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) if exit.code == Some(6)),
            "exit first: {exit_first}: {:?}",
            results[&stop]
        );
    }
}
