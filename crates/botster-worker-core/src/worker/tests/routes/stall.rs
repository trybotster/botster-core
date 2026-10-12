//! The stall clock of a route (steward ruling R-90; DESIGN.md "The stall clock (R-90)"): the progress reports, their order
//! and bound, the stall at a matching mark, the resync at the resume, and the close.

use super::*;
use botster_route_codec::prelude::ResyncReason;

/// The progress reports among `reports`, as `(mark, waiting)`.
fn progress(reports: &[WorkerMsg]) -> Vec<(u64, bool)> {
    reports
        .iter()
        .filter_map(|r| match r {
            WorkerMsg::RouteProgress {
                route: RouteId(1),
                mark,
                waiting,
            } => Some((*mark, *waiting)),
            _ => None,
        })
        .collect()
}

/// The bytes of the route writes of route 1 among `actions`.
fn writes(actions: &[Action]) -> Vec<Vec<u8>> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::RouteWrite {
                route: RouteId(1),
                bytes,
            } => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

/// A route whose baseline is written, then PTY output of `len` bytes whose first write is out and not answered: the route
/// waits. Returns the world, the client, the write that is out, and the mark of the waiting report.
fn waiting_route(len: usize) -> (World, Client, Vec<u8>, u64) {
    let (mut w, client, _) = attached(limits());
    let actions = w.feed(Input::PtyOutput(vec![b'x'; len]));
    let write = writes(&actions).pop().expect("a write of the output");
    let reports = w.reports(&actions);
    let [(mark, true)] = progress(&reports)[..] else {
        panic!("one waiting report: {reports:?}");
    };
    (w, client, write, mark)
}

/// OU-3a: a route reports `waiting` when it holds bytes that the transport has not accepted, and stops when the transport
/// took them. The mark counts the writes that the transport accepted bytes of.
#[test]
fn a_route_reports_waiting_until_its_transport_takes_the_bytes() {
    let (mut w, mut client, write, mark) = waiting_route(10);
    let actions = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(write.len()),
    });
    let reports = w.reports(&actions);
    assert_eq!(progress(&reports), [(mark + 1, false)], "{reports:?}");
    // A readiness that takes no byte is no progress (OU-3a, OU-6).
    let (mut w2, _, _, mark2) = waiting_route(10);
    let none = w2.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(0),
    });
    assert!(progress(&w2.reports(&none)).is_empty());
    let _ = (&mut client, mark2);
}

/// The order rule (R-90 follow-up): a progress report of a change is on the link before every report that the worker sends
/// after the change, even when an earlier progress report is not written yet. Here the link writes nothing: the
/// `waiting: false` report is out, the next output makes the route wait again, and its title is parsed after that.
#[test]
fn a_waiting_report_goes_before_the_title_of_the_output_that_caused_it() {
    let (mut w, _client, write, mark) = waiting_route(10);
    w.instant_link = false;
    let mut actions = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(write.len()),
    });
    actions.extend(w.feed(Input::PtyOutput(b"y\x1b]2;sentinel\x07".to_vec())));
    let reports = w.reports(&actions);
    assert_eq!(
        progress(&reports),
        [(mark + 1, false), (mark + 1, true)],
        "{reports:?}"
    );
    let waiting = reports
        .iter()
        .position(|r| matches!(r, WorkerMsg::RouteProgress { waiting: true, .. }))
        .unwrap();
    let title = reports
        .iter()
        .position(|r| {
            matches!(r, WorkerMsg::Observed { observation: Observation::Title { title, .. } } if title == "sentinel")
        })
        .expect("the title report");
    assert!(waiting < title, "{reports:?}");
}

/// The bound: progress changes alone queue at most one unwritten progress report per route. The latest state follows when
/// the driver wrote the last one.
#[test]
fn progress_changes_alone_queue_one_unwritten_report() {
    let (mut w, _client, write, mark) = waiting_route(3 * FRAME as usize);
    w.instant_link = false;
    // Each accepted byte is progress while the route still waits: three changes of the mark.
    let mut actions = Vec::new();
    let mut rest = write;
    for _ in 0..3 {
        actions.extend(w.feed(Input::RouteWritten {
            route: RouteId(1),
            result: Ok(1),
        }));
        rest.remove(0);
        actions.extend(w.feed(Input::RouteWritable { route: RouteId(1) }));
    }
    let reports = w.reports(&actions);
    assert_eq!(progress(&reports), [(mark + 1, true)], "{reports:?}");
    let sent = w.sent;
    let flushed = w.feed(Input::LinkWritten { total: sent });
    assert_eq!(
        progress(&w.reports(&flushed)),
        [(mark + 3, true)],
        "the latest state, once"
    );
    let _ = rest;
}

/// OU-3b, OU-2: `RouteStallDue` at the route's mark makes it `Stalled`: the started frame is completed, the droppable frames
/// that are not started are dropped, and `resync{stalled}` follows at the resync point. The route resumes on its first
/// accepted byte: the resync and a new baseline, then `RouteResumed`.
#[test]
fn a_stall_at_the_mark_drops_the_unstarted_output_and_the_resume_resyncs() {
    let (mut w, mut client, write, mark) = waiting_route(3 * FRAME as usize);
    let before = client.frames.len();
    let actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    let reports = w.reports(&actions);
    assert!(
        reports.contains(&WorkerMsg::RouteStalled {
            route: RouteId(1),
            mark
        }),
        "{reports:?}"
    );
    // A `Stalled` route leaves the PTY read budget: with no other route, the limit is lifted (OU-3b).
    assert_eq!(budgets(&actions), [None], "{actions:?}");
    // Output while `Stalled` is not queued on the route (OU-3b, OU-7).
    let quiet = w.feed(Input::PtyOutput(b"lost".to_vec()));
    assert!(writes(&quiet).is_empty());
    // The write that was out completes its frame, and its acceptance resumes the route.
    client.reader.push(&write);
    let actions = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(write.len()),
    });
    let reports = w.reports(&actions);
    assert!(
        reports.contains(&WorkerMsg::RouteResumed {
            route: RouteId(1),
            mark: mark + 1
        }),
        "{reports:?}"
    );
    client.take_all(&mut w, RouteId(1), actions);
    let frames = &client.frames[before..];
    let kinds: Vec<&str> = frames
        .iter()
        .map(|f| match f {
            ToClient::Output { .. } => "output",
            ToClient::Resync(r) if r.reason == ResyncReason::Stalled => "resync",
            ToClient::BaselineBegin(_) => "baseline_begin",
            ToClient::Screen { .. } => "screen",
            ToClient::BaselineEnd(_) => "baseline_end",
            ToClient::Live => "live",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "output",
            "resync",
            "baseline_begin",
            "screen",
            "baseline_end",
            "live"
        ],
        "one started frame of {} is completed, the other two are dropped",
        3
    );
}

/// R-90: a due message with a mark that is not the route's (a progress report is in flight) changes nothing, and so does one
/// for a route that does not wait.
#[test]
fn a_stall_due_at_another_mark_or_with_nothing_waiting_is_declined() {
    let (mut w, _client, write, mark) = waiting_route(10);
    let actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark: mark + 1,
    });
    assert!(w.reports(&actions).is_empty(), "{actions:?}");
    let _ = w.feed(Input::RouteWritten {
        route: RouteId(1),
        result: Ok(write.len()),
    });
    let actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark: mark + 1,
    });
    assert!(w.reports(&actions).is_empty(), "nothing waits: {actions:?}");
}

/// OU-2, OU-2b: `RouteCloseDue` at the stall's mark closes a `Stalled` route `StallTimeout`, a failed close with no more
/// frames; at another mark it changes nothing.
#[test]
fn a_close_due_at_the_stall_mark_closes_the_route_stall_timeout() {
    let (mut w, _client, _write, mark) = waiting_route(10);
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    let none = w.send(&HostMsg::RouteCloseDue {
        route: RouteId(1),
        mark: mark + 1,
    });
    assert!(w.reports(&none).is_empty());
    let actions = w.send(&HostMsg::RouteCloseDue {
        route: RouteId(1),
        mark,
    });
    assert!(actions
        .iter()
        .any(|a| matches!(a, Action::RouteClose { route: RouteId(1) })));
    assert!(writes(&actions).is_empty(), "no frame after a failed close");
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::StallTimeout,
            route_tag: None,
        }]
    );
}

/// OU-2: a stalled route that was closing healthy keeps its first reason when the stall closes it.
#[test]
fn a_stall_close_of_a_detached_route_keeps_the_first_reason() {
    let (mut w, _client, _write, mark) = waiting_route(10);
    w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    });
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    let actions = w.send(&HostMsg::RouteCloseDue {
        route: RouteId(1),
        mark,
    });
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::Detached,
            route_tag: None,
        }]
    );
}
