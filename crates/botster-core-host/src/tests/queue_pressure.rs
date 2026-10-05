//! EV-5, TM-3 to TM-6, ST-6 and the observations that become events.

use super::*;

/// Core EV-5b, TM-6: with no mandatory room a state step is parked: the session keeps its state, `more` stays false, and a
/// poll that frees room makes it runnable again.
#[test]
fn a_full_queue_parks_the_transition_and_a_poll_unparks_it() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 2;
        l.mandatory_events = 2;
    }));
    w.running("s1");
    w.engine.poll_events(64);
    w.ok(create("s2"));
    // The queue holds nothing now. Fill it: `Starting` of s2 and one more.
    w.engine.begin(Op::Start { id: sid("s2") }).unwrap();
    let report = w.pump();
    assert!(!report.more, "the parked step is not runnable work (TM-6)");
    // `Starting` and `Running`: two states fill the queue of two; the third step (`Completed`) does not need room.
    assert_eq!(
        w.engine.get(&sid("s2")).unwrap().state,
        SessionState::Running
    );
    // Now a stop: its states need room that is not there.
    w.engine.begin(Op::Stop { id: sid("s2") }).unwrap();
    let report = w.pump();
    assert!(!report.more);
    assert_eq!(
        report.events_posted, 0,
        "nothing is posted while the queue is full"
    );
    assert_eq!(
        w.engine.get(&sid("s2")).unwrap().state,
        SessionState::Running,
        "the state step was not taken (EV-5b)"
    );
    w.engine.poll_events(64);
    w.pump();
    let shown = states(&w.engine.poll_events(64));
    assert_eq!(
        shown.first(),
        Some(&("s2".to_string(), SessionState::Stopping)),
        "EV-5d: the room that a poll frees lets the parked step run: {shown:?}"
    );
}

/// Core EV-5c, LC-5: the effects of a stop go on while the mandatory queue is full: the graceful request, the grace deadline
/// and the kill after it. Only the state events wait, and they follow in order once a poll frees room.
#[test]
fn a_stop_has_its_effects_while_the_queue_is_full() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 4;
        l.mandatory_events = 2;
        l.stop_grace = Duration::from_millis(100);
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    // Unpolled mandatory events fill the queue; then the stop is admitted.
    w.engine.begin(create("s2")).unwrap();
    w.engine.begin(create("s3")).unwrap();
    w.pump();
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the graceful request went out"
    );
    assert!(
        w.engine.next_deadline().is_some(),
        "the grace deadline runs"
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running,
        "the state event waits"
    );
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Kill)),
        "the kill followed the grace"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: None,
            signal: Some(9),
        },
    );
    let mut shown = Vec::new();
    for _ in 0..6 {
        for event in w.engine.poll_events(64) {
            if let Event::SessionState { id, state, .. } = event {
                if id == sid("s1") {
                    shown.push(state);
                }
            }
        }
        w.pump();
    }
    assert!(
        matches!(
            &shown[..],
            [SessionState::Stopping, SessionState::Exited(_)]
        ),
        "{shown:?}"
    );
}

/// Core EV-5, OR-2, 9B: no mandatory event is lost or replaced when the host polls in small batches: each state of the
/// session comes once, in order, and each completion comes once, after the state that its operation reached (OR-2). The order
/// of a completion against the next operation's states is not fixed (OR-3), so it is not checked.
#[test]
fn no_mandatory_event_is_lost_under_a_small_queue() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 1;
        l.mandatory_events = 2;
    }));
    let create = w.engine.begin(create("s1")).unwrap();
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let mut seen = Vec::new();
    for _ in 0..30 {
        w.pump();
        seen.extend(w.engine.poll_events(1));
    }
    let states: Vec<SessionState> = states(&seen).into_iter().map(|(_, s)| s).collect();
    assert_eq!(
        states,
        [
            SessionState::Created,
            SessionState::Starting,
            SessionState::Running
        ]
    );
    let at = |wanted: &dyn Fn(&Event) -> bool| {
        let found: Vec<usize> = seen
            .iter()
            .enumerate()
            .filter(|(_, e)| wanted(e))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(found.len(), 1, "exactly once: {seen:?}");
        found[0]
    };
    let completed =
        |op: OpId| move |e: &Event| matches!(e, Event::Completed { op: o, .. } if *o == op);
    let state = |s: SessionState| move |e: &Event| matches!(e, Event::SessionState { state, .. } if *state == s);
    assert!(at(&state(SessionState::Created)) < at(&completed(create)));
    assert!(at(&state(SessionState::Running)) < at(&completed(start)));
}

/// Core TM-6: work that waits for mandatory room is not runnable: the pump reports no `more`, and the engine reports no
/// deadline for it.
#[test]
fn work_parked_on_room_has_no_deadline_and_is_not_runnable() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 2;
        l.mandatory_events = 1;
    }));
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(!w.pump().more);
    assert_eq!(w.engine.next_deadline(), None);
}

/// Core EV-6, EV-2: a class D overflow leaves the marker, and a keyed value is the latest one.
#[test]
fn observations_become_keyed_and_droppable_events_with_the_injected_time() {
    let mut w = World::new(limits(|l| l.event_queue = 16));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    for n in 0..20u64 {
        w.unix = 2000 + n;
        w.feed(Input::Clock(w.unix));
        w.worker_says("s1", observation(Observation::Bell));
    }
    w.worker_says(
        "s1",
        observation(Observation::Title {
            title: "T1".into(),
            model_rev: ModelRev(2),
        }),
    );
    w.worker_says(
        "s1",
        observation(Observation::Title {
            title: "T2".into(),
            model_rev: ModelRev(3),
        }),
    );
    let events = w.engine.poll_events(64);
    let bells: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            Event::Bell { at, .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(
        bells,
        (2004..2020).collect::<Vec<_>>(),
        "the oldest four were dropped (EV-2)"
    );
    assert!(
        matches!(&events[0], Event::EventsLost { kinds, .. } if kinds.contains(&LostKind::Bell))
    );
    let titles: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::TitleChanged { title, .. } => Some(title.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        titles,
        ["T2"],
        "a keyed event carries the latest value (EV-6)"
    );
    assert_eq!(
        w.engine
            .terminal_state(&sid("s1"))
            .unwrap()
            .title
            .as_deref(),
        Some("T2")
    );
}

/// Core ST-4: `terminal_state` is the cached state and follows the observations; `last_output_at` is the injected unix time.
#[test]
fn terminal_state_follows_the_worker_observations() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let t = w.engine.terminal_state(&sid("s1")).unwrap();
    assert_eq!((t.size.rows, t.size.cols, t.focused), (24, 80, Some(false)));
    w.unix = 5000;
    w.feed(Input::Clock(5000));
    w.worker_says(
        "s1",
        observation(Observation::Output {
            model_rev: ModelRev(9),
        }),
    );
    w.worker_says(
        "s1",
        observation(Observation::Focus {
            focused: FocusState::Unknown,
        }),
    );
    let t = w.engine.terminal_state(&sid("s1")).unwrap();
    assert_eq!(
        (t.last_output_at, t.model_rev, t.focused),
        (Some(5000), ModelRev(9), None)
    );
    w.worker_says(
        "s1",
        observation(Observation::ClientInput {
            route: RouteId(1),
            input_rev: InputRev(4),
        }),
    );
    assert_eq!(
        w.engine
            .terminal_state(&sid("s1"))
            .unwrap()
            .input_rev
            .client,
        InputRev(4)
    );
    assert!(w.engine.terminal_state(&sid("nope")).is_err());
}

/// Core TM-4, TM-3: `Silent` is posted by the first pump at or after the threshold, once per idle period, and the next output
/// re-arms it. The deadline is in `next_deadline` until it fires.
#[test]
fn silent_fires_once_at_the_threshold_and_rearms_on_output() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    w.worker_says(
        "s1",
        observation(Observation::Output {
            model_rev: ModelRev(2),
        }),
    );
    w.engine
        .set_silence_threshold(&sid("s1"), Some(Duration::from_secs(3)))
        .unwrap();
    let base = w.now;
    assert_eq!(
        w.engine.next_deadline(),
        Some(base + Duration::from_secs(3))
    );
    w.advance(Duration::from_millis(2999));
    w.pump();
    assert!(w
        .engine
        .poll_events(64)
        .iter()
        .all(|e| !matches!(e, Event::Silent { .. })));
    w.advance(Duration::from_millis(1));
    w.pump();
    let events = w.engine.poll_events(64);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Silent { .. }))
            .count(),
        1
    );
    assert_eq!(w.engine.next_deadline(), None, "once per idle period");
    w.advance(Duration::from_secs(10));
    w.pump();
    assert!(w
        .engine
        .poll_events(64)
        .iter()
        .all(|e| !matches!(e, Event::Silent { .. })));
    w.worker_says(
        "s1",
        observation(Observation::Output {
            model_rev: ModelRev(3),
        }),
    );
    assert!(
        w.engine.next_deadline().is_some(),
        "the next output re-arms it"
    );
}

/// Core A2-7: `set_silence_threshold` refuses an unknown session, and needs the feature.
#[test]
fn set_silence_threshold_errors() {
    let mut w = World::default();
    assert_eq!(
        w.engine
            .set_silence_threshold(&sid("nope"), None)
            .unwrap_err()
            .code,
        ErrorCode::UnknownSession
    );
    let mut bare = config(CoreLimits::default());
    bare.features.names.clear();
    let mut w = World::configured(bare);
    w.ok(create("s1"));
    assert_eq!(
        w.engine
            .set_silence_threshold(&sid("s1"), None)
            .unwrap_err()
            .code,
        ErrorCode::Unsupported { what: None }
    );
}

fn page(index: u32, last: bool) -> Page {
    Page {
        index,
        bytes: botster_route_codec::prelude::HexBytes(vec![index as u8; 4]),
        last,
    }
}

fn capture_worker(w: &mut World, session: &str, pages: Vec<Page>) -> OpId {
    let op = w
        .engine
        .begin(Op::CaptureSnapshot {
            session: sid(session),
            owner: ClientId("c1".into()),
        })
        .unwrap();
    w.pump();
    let req = w.last_request(session);
    let n = pages.len() as u32;
    w.worker_says(session, WorkerMsg::Pages { req, pages });
    w.worker_says(
        session,
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Capture(Capture {
                capture: CaptureId(0),
                page_count: n,
                total_bytes: 4 * u64::from(n),
                model_rev: ModelRev(5),
            })),
        },
    );
    op
}

/// Core ST-6: Core mints the `CaptureId`, keeps the pages, serves them by index with `last` and `PageOutOfRange`, and a
/// release is idempotent.
#[test]
fn a_capture_is_minted_paged_and_released() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = capture_worker(&mut w, "s1", vec![page(0, false), page(1, true)]);
    let cap = match w.complete(op) {
        OpResult::Ok(OpOutput::Capture(c)) => c,
        other => panic!("{other:?}"),
    };
    assert_eq!((cap.page_count, cap.model_rev), (2, ModelRev(5)));
    assert_eq!(w.engine.read_page(cap.capture, 1).unwrap(), page(1, true));
    assert_eq!(
        w.engine.read_page(cap.capture, 2).unwrap_err().code,
        ErrorCode::PageOutOfRange
    );
    assert_eq!(
        w.engine.read_page(CaptureId(999), 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    w.engine.release(cap.capture);
    w.engine.release(cap.capture);
    assert_eq!(
        w.engine.read_page(cap.capture, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
}

/// Core ST-6, TM-3: a capture expires `capture_ttl` after it was taken, in a pump; it stays readable until then, and the
/// expiry is in `next_deadline`.
#[test]
fn a_capture_expires_in_a_pump() {
    let mut w = World::new(limits(|l| l.capture_ttl = Duration::from_secs(1)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = capture_worker(&mut w, "s1", vec![page(0, true)]);
    let cap = match w.complete(op) {
        OpResult::Ok(OpOutput::Capture(c)) => c,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        w.engine.next_deadline(),
        Some(w.now + Duration::from_secs(1))
    );
    w.advance(Duration::from_millis(1500));
    assert!(
        w.engine.read_page(cap.capture, 0).is_ok(),
        "OR-1: reads make no progress"
    );
    w.pump();
    assert_eq!(
        w.engine.read_page(cap.capture, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    assert_eq!(w.engine.next_deadline(), None);
}

/// Core ER-0, ST-6: `open_captures_per_client` is `CaptureLimit` at `begin`, and releasing resumes; `release_owner` releases
/// all of that client's captures.
#[test]
fn the_capture_limit_is_per_owner_and_resumes_on_release() {
    let mut w = World::new(limits(|l| l.open_captures_per_client = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = capture_worker(&mut w, "s1", vec![page(0, true)]);
    let cap = match w.complete(op) {
        OpResult::Ok(OpOutput::Capture(c)) => c,
        other => panic!("{other:?}"),
    };
    let again = Op::CaptureSnapshot {
        session: sid("s1"),
        owner: ClientId("c1".into()),
    };
    assert_eq!(
        w.engine.begin(again.clone()).unwrap_err().code,
        ErrorCode::CaptureLimit
    );
    assert!(w
        .engine
        .begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId("c2".into())
        })
        .is_ok());
    w.engine.release_owner(&ClientId("c1".into()));
    assert_eq!(
        w.engine.read_page(cap.capture, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
}

/// Core LC-7 step 2, ID-1: `Remove` releases the captures of the session, so an old capture is `UnknownCapture`.
#[test]
fn remove_releases_the_captures_of_the_session() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = capture_worker(&mut w, "s1", vec![page(0, true)]);
    let cap = match w.complete(op) {
        OpResult::Ok(OpOutput::Capture(c)) => c,
        other => panic!("{other:?}"),
    };
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
    );
    w.exited("s1");
    w.complete(remove);
    assert_eq!(
        w.engine.read_page(cap.capture, 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
}

/// Core EV-5, 6.2: `Released` retires the unpolled keyed and droppable events of the instance.
#[test]
fn released_retires_the_instances_unpolled_events() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    w.worker_says("s1", observation(Observation::Bell));
    w.worker_says(
        "s1",
        observation(Observation::Title {
            title: "t".into(),
            model_rev: ModelRev(2),
        }),
    );
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
    );
    w.exited("s1");
    let events = w.until(|e| matches!(e, Event::Completed { op, .. } if *op == remove));
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, Event::Bell { .. } | Event::TitleChanged { .. })),
        "{events:?}"
    );
}

fn capture_op(w: &mut World, owner: &str) -> Result<OpId, CoreError> {
    w.engine.begin(Op::CaptureSnapshot {
        session: sid("s1"),
        owner: ClientId(owner.into()),
    })
}

fn answer_capture(w: &mut World, ok: bool) {
    let req = w.last_request("s1");
    let result = if ok {
        w.worker_says(
            "s1",
            WorkerMsg::Pages {
                req,
                pages: vec![page(0, true)],
            },
        );
        OpResult::Ok(OpOutput::Capture(Capture {
            capture: CaptureId(0),
            page_count: 1,
            total_bytes: 4,
            model_rev: ModelRev(5),
        }))
    } else {
        OpResult::Err(CoreError::new(
            ErrorCode::SnapshotTooLarge,
            "cannot be formed",
        ))
    };
    w.worker_says("s1", WorkerMsg::Done { req, result });
}

/// Core A8-1: a capture reserves `max_snapshot_bytes` at `begin`, so k captures in progress are admitted and the next is
/// `CaptureLimit`, whatever a capture's size.
#[test]
fn a_capture_reserves_max_snapshot_bytes_at_begin() {
    let mut w = World::new(limits(|l| {
        l.max_snapshot_bytes = 1000;
        l.route_queue_bytes = 1000;
        l.snapshot_retained_bytes = 2000;
        l.open_captures_per_client = 8;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    capture_op(&mut w, "a").unwrap();
    capture_op(&mut w, "b").unwrap();
    assert_eq!(
        capture_op(&mut w, "c").unwrap_err().code,
        ErrorCode::CaptureLimit
    );
}

/// Core A8-1, AM-4: a finished capture whose `Completed` is not polled still counts `max_snapshot_bytes`; after the poll a
/// success counts its `total_bytes` and a failure counts nothing.
#[test]
fn the_reservation_changes_only_when_the_completion_is_polled() {
    let mut w = World::new(limits(|l| {
        l.max_snapshot_bytes = 1000;
        l.route_queue_bytes = 1000;
        l.snapshot_retained_bytes = 2100;
        l.open_captures_per_client = 8;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let ok = capture_op(&mut w, "a").unwrap();
    w.pump();
    answer_capture(&mut w, true);
    let failed = capture_op(&mut w, "b").unwrap();
    w.pump();
    answer_capture(&mut w, false);
    w.pump();
    assert_eq!(
        capture_op(&mut w, "c").unwrap_err().code,
        ErrorCode::CaptureLimit,
        "unpolled success and unpolled failure both count the maximum"
    );
    let events = w.engine.poll_events(64);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Err(e) } if *op == failed && e.code == ErrorCode::SnapshotTooLarge)));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == ok)));
    // 4 bytes held for the success, nothing for the failure: two reservations fit again.
    assert!(capture_op(&mut w, "d").is_ok());
    assert!(capture_op(&mut w, "e").is_ok());
    assert_eq!(
        capture_op(&mut w, "f").unwrap_err().code,
        ErrorCode::CaptureLimit
    );
}

/// E3-1 items 4, 5: a transition that finds the mandatory queue full is parked and keeps its state. A later `Silent` (class
/// K, no mandatory room needed) is posted meanwhile. A poll that frees room makes the transition runnable again.
#[test]
fn e3_1_runnable_silent_bypasses_a_transition_parked_on_queue_room() {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = 3;
        l.max_sessions = 5;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        observation(Observation::Output {
            model_rev: ModelRev(2),
        }),
    );
    w.engine
        .set_silence_threshold(&sid("s1"), Some(Duration::from_secs(1)))
        .unwrap();
    w.pump();
    w.engine.poll_events(64);
    // Three `Created` events fill the queue of three.
    for name in ["x1", "x2", "x3"] {
        w.engine.begin(create(name)).unwrap();
    }
    w.pump();
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let report = w.pump();
    assert!(!report.more, "a parked step is not runnable work (TM-6)");
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running,
        "the transition is parked and the state is unchanged"
    );
    w.advance(Duration::from_secs(1));
    w.pump();
    let events = w.engine.poll_events(64);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Silent { .. }))
            .count(),
        1,
        "Silent bypassed the parked step"
    );
    assert!(w.engine.runnable(), "the poll freed room (EV-5d)");
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Stopping,
        "the parked transition ran when room returned"
    );
    let _ = stop;
}

/// E3-1 item 5: an effect with no event runs while an earlier step is parked on room.
#[test]
fn e3_1_eventless_effect_overtakes_a_carried_or_parked_step() {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = 3;
        l.max_sessions = 6;
        l.stop_grace = Duration::from_millis(100);
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    w.running("s2");
    w.engine.poll_events(64);
    let stop2 = w.engine.begin(Op::Stop { id: sid("s2") }).unwrap();
    w.pump();
    w.engine.poll_events(64);
    // Three `Created` events fill the queue, so that the `Stopping` step of s1 parks.
    for name in ["x1", "x2", "x3"] {
        w.engine.begin(create(name)).unwrap();
    }
    w.pump();
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Kill)),
        "the kill of s2 ran while the step of s1 was parked"
    );
    let _ = stop2;
}

/// Core A2-1, IN-7, IN-9 (F17): a `SetNotificationPolicy` of an `Exited` session ends `WorkerLinkFailed` when a `Remove`
/// retires it (the registry path is only the one of `Created`), and a sent repeated key is `Unknown` with the bound of
/// every repeat.
#[test]
fn retirement_keeps_the_result_path_and_bounds_a_repeated_key() {
    let mut w = World::new(limits(|l| {
        l.max_key_repeat = 100;
    }));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::from([Feature::NotificationPolicy]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.complete(start);
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
    let policy = w
        .engine
        .begin(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::All,
        })
        .unwrap();
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
    );
    w.exited("s1");
    let mut done = BTreeMap::new();
    for _ in 0..10 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result } = e {
                done.insert(op, result);
            }
        }
    }
    assert!(
        matches!(done.get(&policy), Some(OpResult::Err(e)) if e.code == ErrorCode::WorkerLinkFailed),
        "{:?}",
        done.get(&policy)
    );
    assert!(done.contains_key(&remove));
    assert_eq!(
        HostEngine::held_bytes(&InputPayload::Key(KeyInput {
            key: botster_route_codec::prelude::Key::Char('a'.into()),
            shifted_key: None,
            base_layout_key: None,
            mods: vec![],
            event: botster_route_codec::prelude::KeyEvent::Press,
            text: None,
            repeat: Some(100),
        })),
        6400
    );
}
