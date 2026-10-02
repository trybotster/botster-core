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
    assert!(w.engine.ready().is_empty());
    w.engine.poll_events(64);
    assert!(
        !w.engine.ready().is_empty(),
        "EV-5d: room makes the parked work runnable"
    );
}

/// Core EV-5c, LC-5: the effects of a stop continue while the queue is full: the graceful request goes out; only the state
/// event waits.
#[test]
fn a_stop_sends_its_request_while_the_queue_is_full() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 2;
        l.mandatory_events = 1;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.poll_events(64);
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    // `Stopping` fits (one slot); the exit that follows needs a slot that the unpolled `Stopping` holds.
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the request went out"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    let report = w.pump();
    assert!(!report.more);
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Stopping
    );
    w.engine.poll_events(64);
    w.pump();
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}

/// Core EV-5, 9B: no mandatory event is lost or replaced when the host polls in small batches.
#[test]
fn no_mandatory_event_is_lost_under_a_small_queue() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 1;
        l.mandatory_events = 2;
    }));
    w.ok_events_collect();
}

impl World {
    fn ok_events_collect(&mut self) {
        self.engine.begin(create("s1")).unwrap();
        self.engine.begin(Op::Start { id: sid("s1") }).unwrap();
        let mut seen = Vec::new();
        for _ in 0..30 {
            self.pump();
            seen.extend(self.engine.poll_events(1));
        }
        let order: Vec<String> = seen
            .iter()
            .map(|e| match e {
                Event::SessionState { state, .. } => format!("{state:?}"),
                Event::Completed { .. } => "completed".into(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            order,
            ["Created", "completed", "Starting", "Running", "completed"],
            "every state and every completion, in order"
        );
    }
}

/// Core TM-6: `ready` (the runnable work) is empty when only blocked work remains, and the engine reports no deadline of
/// parked work.
#[test]
fn work_parked_on_room_has_no_deadline_and_is_not_ready() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 2;
        l.mandatory_events = 1;
    }));
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    assert!(w.engine.ready().is_empty());
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
    w.feed(Input::Features(Features {
        names: BTreeSet::new(),
        service_preamble_versions: vec![1],
    }));
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
    let req = *w.engine.sessions[&sid(session)]
        .inflight
        .keys()
        .next()
        .expect("the request was sent");
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

/// Core TM-3, TM-5: due deadlines are processed in order of due time, one per step.
#[test]
fn due_deadlines_run_in_order_of_due_time() {
    let mut w = World::new(limits(|l| l.capture_ttl = Duration::from_secs(5)));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.worker_says(
        "s1",
        observation(Observation::Output {
            model_rev: ModelRev(2),
        }),
    );
    w.engine
        .set_silence_threshold(&sid("s1"), Some(Duration::from_secs(2)))
        .unwrap();
    let op = capture_worker(&mut w, "s1", vec![page(0, true)]);
    w.complete(op);
    assert_eq!(
        w.engine.next_deadline(),
        Some(w.now + Duration::from_secs(2)),
        "silence is due first"
    );
    w.advance(Duration::from_secs(6));
    w.feed(Input::Clock(w.unix));
    assert_eq!(w.engine.ready().last(), Some(&Work::Deadline));
    let before = w.engine.captures.len();
    run_work(&mut w, Work::Deadline);
    assert_eq!(
        w.engine.captures.len(),
        before,
        "the silence deadline ran first"
    );
    run_work(&mut w, Work::Deadline);
    assert_eq!(w.engine.captures.len(), 0, "then the capture expiry");
}

fn capture_op(w: &mut World, owner: &str) -> Result<OpId, CoreError> {
    w.engine.begin(Op::CaptureSnapshot {
        session: sid("s1"),
        owner: ClientId(owner.into()),
    })
}

fn answer_capture(w: &mut World, ok: bool) {
    let req = *w.engine.sessions[&sid("s1")]
        .inflight
        .keys()
        .next()
        .expect("a request is in flight");
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
