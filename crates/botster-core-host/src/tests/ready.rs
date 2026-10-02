//! What is ready to run, and what waits (plan 2.4, EV-5(b), TM-6, AM-1 order, LC-7, LC-12): the exact conditions.

use super::*;
use crate::flow::{Flow, RemovePhase};
use crate::io::Work;

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

/// Core AM-1: a start does not begin before the setters that were admitted ahead of it have run; with no setter it begins at
/// once.
#[test]
fn a_start_waits_for_exactly_the_setters_ahead_of_it() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(
        w.engine.ready().contains(&Work::Session(sid("s1"))),
        "no setter"
    );
    let mut w = World::default();
    w.ok(create("s1"));
    let resize = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 9,
                cols: 9,
                cell_px: None,
            },
        })
        .unwrap();
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let ready = w.engine.ready();
    assert!(ready.contains(&Work::Op(resize)));
    assert!(
        !ready.contains(&Work::Session(sid("s1"))),
        "the setter runs first"
    );
    w.feed(Input::Run(Work::Op(resize)));
    assert!(w.engine.ready().contains(&Work::Session(sid("s1"))));
}

fn exited(w: &mut World, name: &str) {
    w.running(name);
    w.worker_says(
        name,
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
}

fn meta(name: &str) -> Op {
    Op::UpdateMetadata {
        id: sid(name),
        labels: Default::default(),
    }
}

/// Runs the flow of `name` until it is at `phase`.
fn run_flow_to(w: &mut World, name: &str, phase: RemovePhase) {
    for _ in 0..8 {
        if matches!(&w.engine.sessions[&sid(name)].flow, Flow::Remove(f) if f.phase == phase) {
            return;
        }
        w.feed(Input::Run(Work::Session(sid(name))));
    }
    panic!("the remove flow never reached {phase:?}");
}

/// Core LC-7, AM-1: a `Remove` waits for the `UpdateMetadata` that was admitted ahead of it on its own session, not for one
/// of another session, and not for one that is done.
#[test]
fn a_remove_waits_only_for_its_own_unfinished_metadata_update() {
    // The update of its own session holds the teardown, until the update is done.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    exited(&mut w, "s1");
    let update = w.engine.begin(meta("s1")).unwrap();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    run_flow_to(&mut w, "s1", RemovePhase::SendRemove);
    assert!(
        !w.engine.ready().contains(&Work::Session(sid("s1"))),
        "held"
    );
    w.feed(Input::Run(Work::Op(update)));
    assert!(
        w.engine.ready().contains(&Work::Session(sid("s1"))),
        "released"
    );
    // The update of another session does not hold it.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    exited(&mut w, "s1");
    w.ok(create("s2"));
    w.engine.begin(meta("s2")).unwrap();
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    run_flow_to(&mut w, "s1", RemovePhase::SendRemove);
    assert!(
        w.engine.ready().contains(&Work::Session(sid("s1"))),
        "not held"
    );
}

/// Core EV-5(b), TM-6: a state step that needs mandatory room is not ready while the queue is full, and a step that needs
/// none is ready; a step that posts `Released` or closes a route needs room too.
#[test]
fn steps_that_need_room_are_not_ready_in_a_full_queue() {
    let mut w = World::new(limits(|l| {
        l.mandatory_events = 1;
        l.max_sessions = 4;
    }));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    // `Created` of s1 was polled. A second create fills the queue, unpolled.
    w.engine.begin(create("s2")).unwrap();
    w.pump();
    assert!(!w.engine.has_room());
    // A setter posts no event: it is ready. The row write of a create is ready; its state step is not.
    let resize = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 9,
                cols: 9,
                cell_px: None,
            },
        })
        .unwrap();
    w.engine.begin(create("s3")).unwrap();
    assert!(
        w.engine.ready().contains(&Work::Op(resize)),
        "a setter needs no room"
    );
    w.pump();
    assert!(w.rows.contains_key("session/s3"), "the row was written");
    assert!(
        w.engine.get(&sid("s3")).is_err(),
        "Created waits for room: the state is not shown"
    );
    assert!(!w
        .engine
        .ready()
        .iter()
        .any(|x| *x == Work::Session(sid("s3"))));
    // The remove of a `Created` session posts `Released`, which needs room.
    w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    for _ in 0..10 {
        w.pump();
    }
    assert!(
        !w.engine
            .ready()
            .iter()
            .any(|x| *x == Work::Session(sid("s1"))),
        "the Released step waits"
    );
    assert!(
        w.engine.get(&sid("s1")).is_ok(),
        "the session is not released yet"
    );
    // A poll frees the room: everything goes on.
    for _ in 0..10 {
        w.engine.poll_events(64);
        w.pump();
    }
    assert!(w.engine.get(&sid("s3")).is_ok());
    assert!(w.engine.get(&sid("s1")).is_err(), "released");
}

/// Core DP-7, EV-5(b): a local `Detach` closes the route and so needs room; with no room it waits.
#[test]
fn a_local_detach_waits_for_room() {
    let mut w = World::new(limits(|l| l.mandatory_events = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = w
        .engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            AttachOptions {
                file_directory: "/tmp".into(),
                file_permissions: None,
                route_features: vec![],
                terminal_formats: vec![],
                connect_deadline: None,
                owner: None,
                query_deadline: Some(Duration::from_secs(1)),
                route_tag: None,
                route_limits: None,
                history: None,
                stall_deadline: None,
                answers_queries: true,
                input: true,
            },
        )
        .unwrap()
        .route;
    w.engine.poll_events(64);
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    // One mandatory event fills the queue.
    w.engine.begin(create("s2")).unwrap();
    w.pump();
    assert!(!w.engine.has_room());
    let detach = w
        .engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .unwrap();
    w.pump();
    assert!(
        w.engine.routes.contains_key(&route),
        "the route stays while there is no room"
    );
    assert!(!w.engine.ready().contains(&Work::Op(detach)));
    for _ in 0..6 {
        w.engine.poll_events(64);
        w.pump();
    }
    assert!(!w.engine.routes.contains_key(&route));
}

fn op_msgs(w: &World, link: LinkId) -> usize {
    w.sent
        .iter()
        .filter(|(l, m)| *l == link && matches!(m, HostMsg::Op { .. }))
        .count()
}

/// Core A2-1, IN-7: a read of a session whose start is not through waits for the launch of that session, and goes to the worker
/// after it, with its own request number.
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
    let read1 = w
        .engine
        .begin(Op::ReadCursor { session: sid("s1") })
        .unwrap();
    let read2 = w
        .engine
        .begin(Op::ReadCursor { session: sid("s2") })
        .unwrap();
    w.pump();
    assert_eq!(
        op_msgs(&w, l1) + op_msgs(&w, l2),
        0,
        "both wait for their launch"
    );
    w.feed(Input::LinkMsg {
        link: l1,
        msg: launched(),
    });
    w.complete(start1);
    assert_eq!(op_msgs(&w, l1), 1, "the read of s1 went out");
    assert_eq!(op_msgs(&w, l2), 0, "the read of s2 still waits");
    w.feed(Input::LinkMsg {
        link: l2,
        msg: launched(),
    });
    w.complete(start2);
    assert_eq!(op_msgs(&w, l2), 1);
    let r1 = *w.engine.sessions[&sid("s1")]
        .inflight
        .keys()
        .next()
        .unwrap();
    let r2 = *w.engine.sessions[&sid("s2")]
        .inflight
        .keys()
        .next()
        .unwrap();
    assert_eq!(r2, r1 + 1, "each request has its own number");
    let _ = (read1, read2);
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
    w.engine.poll_events(64);
    assert!(matches!(
        w.engine.ops[&all].step,
        crate::engine::Step::Await(crate::engine::Wait::StopAll(_))
    ));
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

/// Core AM-1: each kind of setter of a `Created` session holds the start until it has run.
#[test]
fn every_kind_of_created_setter_holds_the_start() {
    let setters = [
        Op::SetColorProfile {
            session: sid("s1"),
            profile: ColorProfile {
                palette: None,
                foreground: Rgb { r: 1, g: 2, b: 3 },
                background: Rgb { r: 4, g: 5, b: 6 },
                cursor: None,
            },
        },
        Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Latest,
        },
        Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        },
    ];
    for setter in setters {
        let mut w = World::default();
        w.ok(create("s1"));
        let op = w.engine.begin(setter.clone()).unwrap();
        w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
        assert!(
            !w.engine.ready().contains(&Work::Session(sid("s1"))),
            "{setter:?} holds the start"
        );
        w.feed(Input::Run(Work::Op(op)));
        assert!(
            w.engine.ready().contains(&Work::Session(sid("s1"))),
            "{setter:?} released it"
        );
    }
}

/// Steward ruling R-20: which ops have a fixed timing: a `Resize` in `Created`, a `Resize` to the current size, and a `Stop`
/// of an ended session; no other `Resize` and no other `Stop`.
#[test]
fn only_the_ops_of_the_ruling_have_a_fixed_timing() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("c"));
    let size = |rows| Size {
        rows,
        cols: 80,
        cell_px: None,
    };
    let created = w
        .engine
        .begin(Op::Resize {
            session: sid("c"),
            size: size(30),
        })
        .unwrap();
    assert!(w.engine.never_deferred(&Work::Op(created)));
    w.running("s1");
    let current = w.engine.get(&sid("s1")).unwrap().size;
    let same = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: current,
        })
        .unwrap();
    assert!(w.engine.never_deferred(&Work::Op(same)));
    let other = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: size(current.rows + 1),
        })
        .unwrap();
    assert!(
        !w.engine.never_deferred(&Work::Op(other)),
        "a new size goes to the worker"
    );
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    assert!(
        !w.engine.never_deferred(&Work::Op(stop)),
        "a stop of a running session"
    );
    assert!(!w.engine.never_deferred(&Work::Deadline));
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    let again = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    assert!(
        w.engine.never_deferred(&Work::Op(again)),
        "LC-5: the payload exited"
    );
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
    assert!(!w.engine.has_room());
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
    // The start is in its last step when the stop comes: the start completes, then the stop is sent.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = silent_launch(&mut w, "s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: launched(),
    });
    for _ in 0..20 {
        if matches!(&w.engine.sessions[&sid("s1")].flow, Flow::Start(f) if f.phase == crate::flow::StartPhase::Finish)
        {
            break;
        }
        if !w.step() {
            break;
        }
    }
    assert!(
        matches!(&w.engine.sessions[&sid("s1")].flow, Flow::Start(_)),
        "the start is finishing"
    );
    w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    assert!(
        matches!(&w.engine.sessions[&sid("s1")].flow, Flow::Start(_)),
        "the stop does not replace the start"
    );
    assert!(matches!(
        w.complete(start),
        OpResult::Ok(OpOutput::Record(_))
    ));
    w.pump();
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the stop follows the start"
    );
}
