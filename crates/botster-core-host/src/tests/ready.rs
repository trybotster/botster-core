//! What runs, and what waits (plan 2.4, EV-5(b), TM-6, AM-1, LC-7, LC-12). Each order that a clause fixes is checked under a
//! pump that picks the last ready work first (`World::pump_last_first`), an order that the contract leaves open.

use super::*;

fn silent_launch(w: &mut World, name: &str) -> OpId {
    let start = w.engine.begin(Op::Start { id: sid(name) }).unwrap();
    w.pump();
    let _ = w.link_of_after_hello(name);
    start
}

fn launched() -> WorkerMsg {
    WorkerMsg::Launched {
        features: BTreeSet::new(),
        terminal: terminal_state(),
        formats: vec![],
        payload: botster_core_link::msg::PayloadId {
            pid: 900,
            start_time: 3,
        },
    }
}

fn launch_spec(w: &World) -> Option<botster_core_link::msg::LaunchSpec> {
    w.sent.iter().find_map(|(_, m)| match m {
        HostMsg::Launch(spec) => Some((**spec).clone()),
        _ => None,
    })
}

fn completed(events: &[Event], op: OpId) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
}

/// Core AM-1, OR-1: each setter of a `Created` session that was admitted before `Start` is applied before the launch, in
/// whatever order the pump picks its work: the launch carries the setter's value.
#[test]
fn every_setter_admitted_before_a_start_reaches_the_launch_in_any_order() {
    let profile = ColorProfile {
        palette: None,
        foreground: Rgb { r: 1, g: 2, b: 3 },
        background: Rgb { r: 4, g: 5, b: 6 },
        cursor: None,
    };
    let size = Size {
        rows: 9,
        cols: 11,
        cell_px: None,
    };
    let setters = [
        Op::SetColorProfile {
            session: sid("s1"),
            profile: profile.clone(),
        },
        Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Largest,
        },
        Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        },
        Op::Resize {
            session: sid("s1"),
            size,
        },
    ];
    for setter in setters {
        let mut w = World::offering(Feature::SizePolicyOther);
        w.ok(create("s1"));
        w.engine.begin(setter.clone()).unwrap();
        w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
        for _ in 0..20 {
            if launch_spec(&w).is_some() {
                break;
            }
            w.pump_last_first();
        }
        let spec = launch_spec(&w).expect("the launch");
        match setter {
            Op::SetColorProfile { .. } => assert_eq!(spec.color_profile, Some(profile.clone())),
            Op::SetSizePolicy { .. } => assert_eq!(spec.size_policy, SizePolicy::Largest),
            Op::SetNotificationPolicy { .. } => {
                assert_eq!(spec.notification_policy, NotificationPolicy::None)
            }
            _ => assert_eq!(spec.size, size),
        }
    }
}

/// Core LC-7, AM-1: an `UpdateMetadata` that was admitted before `Remove` writes its row before the worker is asked for its
/// teardown, in whatever order the pump picks its work.
#[test]
fn a_remove_tears_down_after_the_metadata_update_admitted_before_it() {
    let mut w = World::default();
    w.running("s1");
    w.ok(Op::Stop { id: sid("s1") });
    let labels = BTreeMap::from([("k".to_string(), "v".to_string())]);
    let update = w
        .engine
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: labels.clone(),
        })
        .unwrap();
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    let from = w.trace.len();
    for _ in 0..20 {
        w.pump_last_first();
    }
    let results = w.complete_all(&[update, remove]);
    assert!(matches!(results[&update], OpResult::Ok(_)), "{results:?}");
    assert!(matches!(results[&remove], OpResult::Ok(_)), "{results:?}");
    let trace = &w.trace[from..];
    let written = trace.iter().position(|t| t == "write session/s1");
    let asked = trace.iter().position(|t| t == "send remove");
    assert!(
        matches!((written, asked), (Some(a), Some(b)) if a < b),
        "{trace:?}"
    );
}

/// Core EV-5(b), TM-6, LC-7: with the mandatory queue full, a step that posts a state waits (the row of a create is written,
/// and its `Created` is not shown; a remove does not release the id); a setter, which posts nothing, applies; a poll that
/// frees room lets every waiting step go on.
#[test]
fn a_state_step_waits_for_room_and_a_poll_lets_it_go_on() {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = 1;
        l.max_sessions = 4;
    }));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    // A second create fills the queue, unpolled.
    w.engine.begin(create("s2")).unwrap();
    w.pump();
    let size = Size {
        rows: 9,
        cols: 9,
        cell_px: None,
    };
    w.engine
        .begin(Op::Resize {
            session: sid("s1"),
            size,
        })
        .unwrap();
    w.engine.begin(create("s3")).unwrap();
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().size,
        size,
        "a setter needs no room"
    );
    assert!(w.rows.contains_key("session/s3"), "the row was written");
    assert!(
        w.engine.get(&sid("s3")).is_err(),
        "Created waits for room: the state is not shown"
    );
    let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    for _ in 0..10 {
        w.pump();
    }
    assert!(
        w.engine.get(&sid("s1")).is_ok(),
        "the session is not released yet"
    );
    let mut events = Vec::new();
    for _ in 0..10 {
        events.extend(w.engine.poll_events(64));
        w.pump();
    }
    assert!(completed(&events, remove), "{events:?}");
    assert!(w.engine.get(&sid("s3")).is_ok());
    assert!(w.engine.get(&sid("s1")).is_err(), "released");
}

/// Core DP-7, EV-5(b): a local `Detach` posts `RouteClosed`, so with no room it waits; after a poll the route closes and the
/// detach completes.
#[test]
fn a_local_detach_waits_for_room() {
    let mut w = World::new(limits(|l| l.mandatory_events = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.engine.poll_events(64);
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    // One mandatory event fills the queue.
    w.engine.begin(create("s2")).unwrap();
    w.pump();
    let detach = w
        .engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .unwrap();
    w.pump();
    let mut events = Vec::new();
    for _ in 0..6 {
        events.extend(w.engine.poll_events(64));
        w.pump();
    }
    let closed = events
        .iter()
        .position(|e| matches!(e, Event::RouteClosed { route: r, reason: RouteCloseReason::Detached, .. } if *r == route));
    let done = events
        .iter()
        .position(|e| matches!(e, Event::Completed { op, .. } if *op == detach));
    assert!(
        matches!((closed, done), (Some(c), Some(d)) if c < d),
        "DP-7: RouteClosed, then the completion: {events:?}"
    );
}

fn op_requests(w: &World, link: LinkId) -> Vec<u64> {
    w.sent
        .iter()
        .filter_map(|(l, m)| match m {
            HostMsg::Op { req, .. } if *l == link => Some(*req),
            _ => None,
        })
        .collect()
}

/// Core A2-1, IN-7: a read of a session whose start is not through waits for the launch of that session, and goes to the worker
/// after it, with a request number of its own.
#[test]
fn a_read_waits_for_the_launch_of_its_own_session() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.ok(create("s2"));
    let start1 = silent_launch(&mut w, "s1");
    let l1 = w.link_of("s1");
    let start2 = silent_launch(&mut w, "s2");
    let l2 = w.link_of("s2");
    for name in ["s1", "s2"] {
        w.engine
            .begin(Op::ReadCursor { session: sid(name) })
            .unwrap();
    }
    w.pump();
    assert!(
        op_requests(&w, l1).is_empty() && op_requests(&w, l2).is_empty(),
        "both wait for their launch"
    );
    w.feed(Input::LinkMsg {
        link: l1,
        msg: launched(),
    });
    w.complete(start1);
    assert_eq!(op_requests(&w, l1).len(), 1, "the read of s1 went out");
    assert!(op_requests(&w, l2).is_empty(), "the read of s2 still waits");
    w.feed(Input::LinkMsg {
        link: l2,
        msg: launched(),
    });
    w.complete(start2);
    assert_eq!(op_requests(&w, l2).len(), 1);
    assert_ne!(
        op_requests(&w, l1),
        op_requests(&w, l2),
        "each request has its own number"
    );
}

/// Core A2-1, IN-7: a read that finds the start of its session stopped by a stop waits for the launch too; one that finds the
/// link failed ends `WorkerLinkFailed` at once.
#[test]
fn a_read_in_a_stopping_start_waits_and_a_read_over_a_failed_link_ends() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    let read = w
        .engine
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    w.pump();
    assert!(
        w.engine
            .poll_events(64)
            .iter()
            .all(|e| !matches!(e, Event::Completed { op, .. } if *op == read)),
        "the read waits for the launch"
    );
    // The link fails during the start: a read ends at once.
    let mut w = World::new(limits(|l| l.mandatory_events = 2));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkClosed { link });
    let read = w
        .engine
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    w.pump();
    let events = w.engine.poll_events(64);
    assert!(
        events.iter().any(|e| matches!(e, Event::Completed { op, result: OpResult::Err(err) } if *op == read && err.code == ErrorCode::WorkerLinkFailed)),
        "{events:?}"
    );
}

/// Core LC-12: `StopAll` waits for a target that is stopping, and for a target whose start is not through; it stops that
/// target when the start ends.
#[test]
fn stop_all_waits_for_stopping_and_starting_targets() {
    // A target that is already stopping.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let all = w.engine.begin(Op::StopAll).unwrap();
    w.pump();
    assert!(
        !completed(&w.engine.poll_events(64), all),
        "StopAll waits for the stopping target"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    assert_eq!(w.complete(all), OpResult::Ok(OpOutput::Unit));
    // A target whose start is not through: the stop follows the launch.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = silent_launch(&mut w, "s1");
    let link = w.link_of("s1");
    let all = w.engine.begin(Op::StopAll).unwrap();
    w.pump();
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "not before the launch"
    );
    w.feed(Input::LinkMsg {
        link,
        msg: launched(),
    });
    w.complete(start);
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the stop follows the launch"
    );
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    assert_eq!(w.complete(all), OpResult::Ok(OpOutput::Unit));
}

/// Core LC-12, LC-5: a `Stop` that is admitted while the exit of the payload is being posted joins that exit: the end is the
/// exit that the worker reported, and no stop is sent. A `Stop` admitted while the start finishes follows the start.
#[test]
fn a_stop_joins_an_exit_in_flight_and_follows_a_start_that_finishes() {
    // The exit waits for room in the queue; the stop joins it.
    let mut w = World::new(limits(|l| l.mandatory_events = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    // An unpolled `Created` fills the queue, so that the end of s1 waits for room.
    w.engine.begin(create("x")).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(7),
            signal: None,
        },
    );
    w.pump();
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.engine.poll_events(64);
    let mut end = None;
    for _ in 0..10 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, result } = e {
                if op == stop {
                    end = Some(result);
                }
            }
        }
    }
    match end {
        Some(OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit)))) => {
            assert_eq!(exit.code, Some(7));
            assert_eq!(
                exit.cause,
                ExitCause::Normal,
                "the worker's exit, not a host stop"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(!w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)));
    // The launch is reported, and the stop comes before a pump finishes the start: the start completes `Running`, then the
    // stop is sent (LC-12: a `Starting` target is stopped as soon as its start completes).
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = silent_launch(&mut w, "s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: launched(),
    });
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "not before the start completes"
    );
    assert!(matches!(
        w.complete(start),
        OpResult::Ok(OpOutput::Record(SessionRecord {
            state: SessionState::Running,
            ..
        }))
    ));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the stop follows the start"
    );
}

/// Core OR-1, TM-2: `begin` makes no progress: a setter of a `Created` session changes the record only in a `pump`.
#[test]
fn a_created_setter_changes_nothing_before_a_pump() {
    let mut w = World::default();
    w.ok(create("s1"));
    let before = w.engine.get(&sid("s1")).unwrap();
    let new = Size {
        rows: before.size.rows + 1,
        ..before.size
    };
    w.engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: new,
        })
        .unwrap();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap(),
        before,
        "begin makes no progress"
    );
    w.pump();
    assert_eq!(w.engine.get(&sid("s1")).unwrap().size, new);
}

/// Core A2-1, AM-1: a read admitted while a `Create` and a `Start` of its session wait for a pump waits for the launch, and
/// goes to the worker after it; it does not fail for want of a link that the start has not made yet.
#[test]
fn a_read_admitted_behind_a_create_and_a_start_waits_for_the_launch() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.engine.begin(create("s1")).unwrap();
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let read = w
        .engine
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    w.pump();
    let early = w.engine.poll_events(64);
    assert!(!completed(&early, read), "{early:?}");
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: launched(),
    });
    w.complete(start);
    assert_eq!(
        op_requests(&w, link).len(),
        1,
        "the read went to the worker"
    );
}
