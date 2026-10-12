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
                route: RouteKey(1),
                bytes,
            } => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

/// A route whose baseline is written, then PTY output of `len` bytes whose first write is out and not answered: the route
/// waits. Returns the world, the client, the write that is out, and the mark of the waiting report.
fn waiting_route(len: usize) -> (World, Client, Vec<u8>, u64) {
    waiting_route_with(limits(), len)
}

/// [`waiting_route`] with these route limits.
fn waiting_route_with(limits: AppliedRouteLimits, len: usize) -> (World, Client, Vec<u8>, u64) {
    let (mut w, client, _) = attached(limits);
    let actions = w.feed(Input::PtyOutput(vec![b'x'; len]));
    let write = writes(&actions).pop().expect("a write of the output");
    let reports = w.reports(&actions);
    let [(mark, true)] = progress(&reports)[..] else {
        panic!("one waiting report: {reports:?}");
    };
    (w, client, write, mark)
}

/// The kinds of `frames`, by name.
fn kinds(frames: &[ToClient]) -> Vec<&'static str> {
    frames
        .iter()
        .map(|f| match f {
            ToClient::Attached(_) => "attached",
            ToClient::Output { .. } => "output",
            ToClient::Resync(r) if r.reason == ResyncReason::Stalled => "resync",
            ToClient::BaselineBegin(_) => "baseline_begin",
            ToClient::Screen { .. } => "screen",
            ToClient::BaselineEnd(_) => "baseline_end",
            ToClient::Live => "live",
            ToClient::InputRefused(_) => "input_refused",
            other => panic!("{other:?}"),
        })
        .collect()
}

/// The resync and the new baseline of a resume.
const RESUMED: [&str; 5] = ["resync", "baseline_begin", "screen", "baseline_end", "live"];

/// OU-3a: a route reports `waiting` when it holds bytes that the transport has not accepted, and stops when the transport
/// took them. The mark counts the writes that the transport accepted bytes of.
#[test]
fn a_route_reports_waiting_until_its_transport_takes_the_bytes() {
    let (mut w, mut client, write, mark) = waiting_route(10);
    let actions = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(write.len()),
    });
    let reports = w.reports(&actions);
    assert_eq!(progress(&reports), [(mark + 1, false)], "{reports:?}");
    // A readiness that takes no byte is no progress (OU-3a, OU-6).
    let (mut w2, _, _, mark2) = waiting_route(10);
    let none = w2.feed(Input::RouteWritten {
        route: RouteKey(1),
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
        route: RouteKey(1),
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
            route: RouteKey(1),
            result: Ok(1),
        }));
        rest.remove(0);
        actions.extend(w.feed(Input::RouteWritable { route: RouteKey(1) }));
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
        route: RouteKey(1),
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
    client.take_all(&mut w, RouteKey(1), actions);
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
        route: RouteKey(1),
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
        .any(|a| matches!(a, Action::RouteClose { route: RouteKey(1) })));
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

/// OU-3b, OU-9, DP-5: a route that stalls before its transport took a byte keeps `attached` first, since it opens the
/// stream, and drops the baseline behind it. The resync follows `attached`, and the resume sends a new baseline. Input
/// before the new `live` is written is refused `not_ready`; after it, input is taken, and nothing waits.
#[test]
fn a_stall_before_the_first_accepted_byte_keeps_attached_first() {
    let mut w = World::running();
    let mut actions = attach(&mut w, DescriptorId(1), RouteId(1), options(), limits());
    actions.extend(w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(0),
    }));
    assert_eq!(progress(&w.reports(&actions)), [(0, true)]);
    let mut actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark: 0,
    });
    assert!(w.reports(&actions).contains(&WorkerMsg::RouteStalled {
        route: RouteId(1),
        mark: 0
    }));
    let early = w.feed(Input::RouteRead {
        route: RouteKey(1),
        bytes: client_frame(&bytes_frame(5, b"x")),
    });
    assert!(pty_writes(&early).is_empty());
    actions.extend(early);
    let mut client = Client::default();
    let all = client.take_all(&mut w, RouteKey(1), actions);
    let mut expected = vec!["attached"];
    expected.extend(RESUMED);
    expected.push("input_refused");
    assert_eq!(kinds(&client.frames), expected);
    assert_eq!(
        refusals(&client),
        vec![(Some(Op(5)), RefusalReason::NotReady)]
    );
    assert_eq!(
        progress(&w.reports(&all)).last().map(|p| p.1),
        Some(false),
        "every queued byte is written"
    );
    // The dropped baseline frames leave the exempt bytes (R-45): until the queue is written, only the refusal counts
    // against the PTY read budget.
    let refusal = client
        .frames
        .iter()
        .find(|f| matches!(f, ToClient::InputRefused(_)))
        .map(|f| botster_route_codec::prelude::stream_wrap(&f.encode()).len())
        .unwrap();
    let budgets = budgets(&all);
    let full = budgets.last().copied().flatten().unwrap();
    assert!(
        budgets[1..budgets.len() - 1]
            .iter()
            .all(|b| *b == Some(full - refusal)),
        "{budgets:?}"
    );
    let late = w.feed(Input::RouteRead {
        route: RouteKey(1),
        bytes: client_frame(&bytes_frame(6, b"y")),
    });
    assert_eq!(pty_writes(&late), b"y", "the new baseline is written");
}

/// OU-3b, OU-4: a frame with no byte written and no write out is not started: a stall drops it. Here the transport took no
/// byte of the output (`Ok(0)`), so the client reads only the resync and the new baseline.
#[test]
fn a_stall_drops_an_output_frame_that_the_transport_took_nothing_of() {
    let (mut w, mut client, _write, mark) = waiting_route(10);
    let before = client.frames.len();
    w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(0),
    });
    let actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    client.take_all(&mut w, RouteKey(1), actions);
    assert_eq!(kinds(&client.frames[before..]), RESUMED);
}

/// OU-4: a frame that the transport took a part of is started, with no write out: a stall completes it before the resync.
#[test]
fn a_stall_completes_a_partly_written_frame() {
    let (mut w, mut client, write, mark) = waiting_route(10);
    let before = client.frames.len();
    client.reader.push(&write[..1]);
    w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    // The write of the rest takes nothing: no write is out.
    w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(0),
    });
    let actions = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark: mark + 1,
    });
    assert!(w.reports(&actions).contains(&WorkerMsg::RouteStalled {
        route: RouteId(1),
        mark: mark + 1
    }));
    client.take_all(&mut w, RouteKey(1), actions);
    let mut expected = vec!["output"];
    expected.extend(RESUMED);
    assert_eq!(kinds(&client.frames[before..]), expected);
}

/// OU-3a: more bytes queued at the same mark change neither the mark nor `waiting`: no new report.
#[test]
fn more_output_at_the_same_mark_is_no_new_report() {
    let (mut w, _client, _write, _mark) = waiting_route(10);
    let actions = w.feed(Input::PtyOutput(b"y".to_vec()));
    assert!(progress(&w.reports(&actions)).is_empty());
}

/// OU-2: a resume whose new baseline is over the route's bound closes the route `SnapshotTooLarge`, with no resume.
#[test]
fn a_resume_whose_baseline_is_over_the_bound_closes_the_route() {
    let (_, client, _) = attached(limits());
    let screen = client
        .frames
        .iter()
        .find(|f| matches!(f, ToClient::Screen { .. }))
        .expect("the screen of the baseline")
        .encode()
        .len();
    let mut tight = limits();
    tight.max_screen_frame_bytes = screen.try_into().unwrap();
    let (mut w, mut client, write, mark) = waiting_route_with(tight, 10);
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    // The screen now holds the output, so its frame is over the bound. The healthy close delivers its `route_closed`
    // frame, then the host is told.
    client.reader.push(&write);
    let actions = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(write.len()),
    });
    let all = client.take_all(&mut w, RouteKey(1), actions);
    let reports = w.reports(&all);
    assert!(
        reports.contains(&WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None,
        }),
        "{reports:?}"
    );
    assert!(!reports
        .iter()
        .any(|r| matches!(r, WorkerMsg::RouteResumed { .. })));
}

/// The bound: a link write that does not reach the route's last progress report sends no newer one; the write that reaches
/// it does.
#[test]
fn a_newer_report_waits_until_the_link_wrote_the_last_one() {
    let (mut w, _client, _write, mark) = waiting_route(3 * FRAME as usize);
    w.instant_link = false;
    let first = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    assert_eq!(progress(&w.reports(&first)), [(mark + 1, true)]);
    let sent = w.sent;
    w.feed(Input::RouteWritable { route: RouteKey(1) });
    w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    let short = w.feed(Input::LinkWritten { total: sent - 1 });
    assert!(progress(&w.reports(&short)).is_empty(), "{short:?}");
    let reached = w.feed(Input::LinkWritten { total: sent });
    assert_eq!(progress(&w.reports(&reached)), [(mark + 2, true)]);
    // The link wrote exactly the last report: the next change goes out at once.
    let sent = w.sent;
    w.feed(Input::LinkWritten { total: sent });
    w.feed(Input::RouteWritable { route: RouteKey(1) });
    let next = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    assert_eq!(progress(&w.reports(&next)), [(mark + 3, true)]);
}

/// R-90 Q3: a new host has none of the old link's reports. After `Adopted`, a waiting route reports its state again on the
/// new link.
#[test]
fn an_adoption_reports_a_waiting_route_again_after_adopted() {
    let (mut w, _client, _write, mark) = waiting_route(10);
    let actions = adopt(&mut w, EPOCH + 3);
    let (_, frames) = after_fence(&mut w, &actions);
    let msgs: Vec<WorkerMsg> = frames[1..]
        .iter()
        .map(|f| WorkerMsg::decode(&f.payload).unwrap())
        .collect();
    assert!(matches!(msgs[0], WorkerMsg::Adopted { .. }), "{msgs:?}");
    assert_eq!(
        msgs[1..],
        [WorkerMsg::RouteProgress {
            route: RouteId(1),
            mark,
            waiting: true
        }]
    );
}

/// OU-2, DP-3: a resume whose `baseline_begin` is over the route's `max_frame_bytes` closes the route `SnapshotTooLarge`. The
/// modes that the output set while the route was `Stalled` make the frame larger than at the attach.
#[test]
fn a_resume_whose_baseline_begin_is_over_the_frame_bound_closes_the_route() {
    let (_, client, _) = attached(limits());
    let begin = client
        .frames
        .iter()
        .find(|f| matches!(f, ToClient::BaselineBegin(_)))
        .expect("the baseline_begin of the attach")
        .encode()
        .len();
    let mut tight = limits();
    tight.max_frame_bytes = begin.try_into().unwrap();
    let (mut w, mut client, write, mark) = waiting_route_with(tight, 10);
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    w.feed(Input::PtyOutput(b"\x1b[?1003h\x1b[?1006h".to_vec()));
    client.reader.push(&write);
    let actions = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(write.len()),
    });
    let all = client.take_all(&mut w, RouteKey(1), actions);
    let reports = w.reports(&all);
    assert!(
        reports.contains(&WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None,
        }),
        "{reports:?}"
    );
    assert!(!reports
        .iter()
        .any(|r| matches!(r, WorkerMsg::RouteResumed { .. })));
}

/// OU-2, OU-9: a second stall before the first resync is written drops that resync with its baseline: the client reads one
/// resync and one baseline, from the second stall.
#[test]
fn a_second_stall_before_the_resync_is_written_drops_the_first_resync() {
    let (mut w, mut client, write, mark) = waiting_route(3 * FRAME as usize);
    let before = client.frames.len();
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    // One byte of the started frame resumes the route; the write of its rest is out.
    client.reader.push(&write[..1]);
    let mut actions = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    assert!(w.reports(&actions).contains(&WorkerMsg::RouteResumed {
        route: RouteId(1),
        mark: mark + 1
    }));
    let second = w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark: mark + 1,
    });
    assert!(w.reports(&second).contains(&WorkerMsg::RouteStalled {
        route: RouteId(1),
        mark: mark + 1
    }));
    actions.extend(second);
    client.take_all(&mut w, RouteKey(1), actions);
    let mut expected = vec!["output"];
    expected.extend(RESUMED);
    assert_eq!(kinds(&client.frames[before..]), expected);
}

/// A route whose `baseline_begin` the mouse modes grow over its `max_frame_bytes`, `Stalled` at a mark with a write of the
/// output out, and the client that read it. Returns the world, the client, the write that is out, and the stall's mark.
fn stalled_with_a_resume_over_the_bound() -> (World, Client, Vec<u8>, u64) {
    let (_, client, _) = attached(limits());
    let begin = client
        .frames
        .iter()
        .find(|f| matches!(f, ToClient::BaselineBegin(_)))
        .expect("the baseline_begin of the attach")
        .encode()
        .len();
    let mut tight = limits();
    tight.max_frame_bytes = begin.try_into().unwrap();
    let (mut w, client, write, mark) = waiting_route_with(tight, 10);
    w.send(&HostMsg::RouteStallDue {
        route: RouteId(1),
        mark,
    });
    w.feed(Input::PtyOutput(b"\x1b[?1003h\x1b[?1006h".to_vec()));
    (w, client, write, mark)
}

/// One byte of the started frame is accepted, so the route tries to resume and cannot; then the transport takes nothing.
fn one_byte_then_blocked(w: &mut World, client: &mut Client, write: &[u8]) -> Vec<WorkerMsg> {
    client.reader.push(&write[..1]);
    let mut actions = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    actions.extend(w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(0),
    }));
    w.reports(&actions)
}

/// OU-2, OU-9 (0181's ST298-R2): a resume whose baseline cannot be formed starts the `SnapshotTooLarge` close and sends no
/// `RouteResumed`. The route stays `Stalled` at the stall's mark, as the host sees it, so when the close is not delivered,
/// the host's close step at that mark ends the route with the close's reason, and the host is told.
#[test]
fn a_failed_resume_with_blocked_writes_is_closed_by_the_close_step() {
    let (mut w, mut client, write, mark) = stalled_with_a_resume_over_the_bound();
    let reports = one_byte_then_blocked(&mut w, &mut client, &write);
    assert!(
        !reports.iter().any(|r| matches!(
            r,
            WorkerMsg::RouteResumed { .. } | WorkerMsg::RouteClosed { .. }
        )),
        "{reports:?}"
    );
    let actions = w.send(&HostMsg::RouteCloseDue {
        route: RouteId(1),
        mark,
    });
    assert!(actions
        .iter()
        .any(|a| matches!(a, Action::RouteClose { route: RouteKey(1) })));
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::SnapshotTooLarge,
            route_tag: None,
        }]
    );
}

/// OU-2 (0181's ST298-R2): the same with a healthy close that began while the route was `Stalled`. The failed resume keeps
/// it, and the close step ends the route with that first reason.
#[test]
fn a_failed_resume_of_a_closing_route_is_closed_by_the_close_step_with_its_first_reason() {
    let (mut w, mut client, write, mark) = stalled_with_a_resume_over_the_bound();
    w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    });
    let reports = one_byte_then_blocked(&mut w, &mut client, &write);
    assert!(
        !reports.iter().any(|r| matches!(
            r,
            WorkerMsg::RouteResumed { .. } | WorkerMsg::RouteClosed { .. }
        )),
        "{reports:?}"
    );
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

/// The order rule (R-90 follow-up; 0181's ST298-R4) on the `Output` paths: with an older `Output` report and an older
/// progress report unwritten, a progress change and then a later output leave both new reports pending. When the link
/// writes, the progress report goes first, then the `Output` report, one of each.
#[test]
fn a_pending_progress_report_goes_before_a_pending_output_report() {
    let (mut w, _client, _write, mark) = waiting_route(3 * FRAME as usize);
    w.instant_link = false;
    let first = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    assert_eq!(progress(&w.reports(&first)), [(mark + 1, true)]);
    let output = w.feed(Input::PtyOutput(b"a".to_vec()));
    assert!(w.reports(&output).iter().any(|r| matches!(
        r,
        WorkerMsg::Observed {
            observation: Observation::Output { .. }
        }
    )));
    w.feed(Input::RouteWritable { route: RouteKey(1) });
    let change = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(1),
    });
    let later = w.feed(Input::PtyOutput(b"b".to_vec()));
    assert!(w.reports(&change).is_empty() && w.reports(&later).is_empty());
    let sent = w.sent;
    let written = w.feed(Input::LinkWritten { total: sent });
    let reports = w.reports(&written);
    let kinds: Vec<&str> = reports
        .iter()
        .map(|r| match r {
            WorkerMsg::RouteProgress { .. } => "progress",
            WorkerMsg::Observed {
                observation: Observation::Output { .. },
            } => "output",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["progress", "output"], "{reports:?}");
    assert_eq!(progress(&reports), [(mark + 2, true)]);
}
