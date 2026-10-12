//! DP-8 route adoption on the host (design note `st-dp8-route-adoption-design.md` section 6, 0181's STDP8-D4): a row's
//! `RouteLive` records are kept until its state posts, registered with new ids then, and each posts one `RouteAdopted`
//! before the row is done. The stall rule is "add at post, remove at consume" (#298).

use super::*;
use crate::run::DeadlineKind;
use botster_core_link::msg::RouteLive;

const CLOSE_AFTER: Duration = ADOPTED_CLOSE_AFTER;

/// The worker's record of a live route with key `key`: tagged `t<key>`, `o<key>` and `c<key>`, or with no tags.
fn live(key: u64, tagged: bool, stalled: Option<u64>) -> RouteLive {
    RouteLive {
        key,
        route_tag: tagged.then(|| format!("t{key}")),
        owner: tagged.then(|| format!("o{key}")),
        client: ClientId(format!("c{key}")),
        limits: adopted_route_limits(),
        stalled,
    }
}

/// A new host, with `mandatory` slots, over a first host whose session `s` crashed before its launch. The worker of `s`
/// reports `payload` and the live routes `routes`.
fn adopting_routes(payload: AdoptedPayload, routes: Vec<RouteLive>, mandatory: u32) -> World {
    let mut first = crashed_before_launch("s");
    first.engine.cfg.limits.mandatory_events = mandatory;
    let mut again = adopting(&first, "s", payload);
    again
        .endpoints
        .get_mut(&first.instance_of("s"))
        .expect("scripted above")
        .routes = routes;
    again
}

/// The `RouteId` of each key that the host sent, in send order.
fn route_ids(w: &World) -> Vec<(u64, RouteId)> {
    w.sent
        .iter()
        .filter_map(|(_, m)| match m {
            HostMsg::RouteId { key, route } => Some((*key, *route)),
            _ => None,
        })
        .collect()
}

/// Begins `AdoptAll` and pumps, with no poll, until the row's state posted.
fn until_state_posted(w: &mut World) -> OpId {
    let op = w.engine.begin(Op::AdoptAll).expect("AdoptAll is admitted");
    for _ in 0..20 {
        w.pump();
        if w.engine.sessions[&sid("s")].shown.is_some() {
            return op;
        }
    }
    panic!("the state never posted");
}

/// The instant of the route's armed step.
fn due_at(w: &World, route: RouteId) -> Option<Instant> {
    w.engine
        .deadlines()
        .into_iter()
        .find(|(_, kind)| matches!(kind, DeadlineKind::Route(r) if *r == route))
        .map(|(at, _)| at)
}

/// One poll that frees every slot, then one pump; the events of the poll.
fn poll_then_pump(w: &mut World) -> Vec<Event> {
    let events = w.engine.poll_events(64);
    w.pump();
    events
}

/// DP-8, AD-1, LC-11: the row's state posts first, then one `RouteAdopted` per reported route with its attach tags and a
/// new id of this handle (an absent tag is the empty string), then the completion of `AdoptAll`. The host sends each
/// route's `RouteId` to the worker.
#[test]
fn adopt_all_posts_a_route_adopted_per_live_route_after_the_state_and_before_the_completion() {
    let mut w = adopting_routes(
        running_payload(),
        vec![live(10, true, None), live(20, false, None)],
        100,
    );
    let events = adopt_all(&mut w);
    let ids = route_ids(&w);
    assert_eq!(
        ids.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
        [10, 20]
    );
    let (a, b) = (ids[0].1, ids[1].1);
    assert_ne!(a, b);
    let instance = w.engine.sessions[&sid("s")].instance.clone();
    let kinds: Vec<&Event> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::SessionState { .. } | Event::RouteAdopted { .. } | Event::Completed { .. }
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            &Event::SessionState {
                id: sid("s"),
                instance: instance.clone(),
                state: SessionState::Running
            },
            &Event::RouteAdopted {
                session: sid("s"),
                instance: instance.clone(),
                route: a,
                route_tag: "t10".into(),
                owner: "o10".into(),
                client: ClientId("c10".into()),
            },
            &Event::RouteAdopted {
                session: sid("s"),
                instance,
                route: b,
                route_tag: String::new(),
                owner: String::new(),
                client: ClientId("c20".into()),
            },
            events.last().unwrap(),
        ]
    );
    assert_eq!(w.engine.sessions[&sid("s")].routes, BTreeSet::from([a, b]));
}

/// STDP8-D4 (R3): with one mandatory slot, each phase-3 step posts one event and waits for a poll; `AdoptAll` completes
/// only after the last `RouteAdopted`.
#[test]
fn with_one_slot_each_route_adopted_waits_for_a_poll_and_the_completion_comes_last() {
    let mut w = adopting_routes(
        running_payload(),
        vec![live(1, true, None), live(2, true, None)],
        1,
    );
    let op = until_state_posted(&mut w);
    let mut posted = Vec::new();
    for _ in 0..10 {
        let events = poll_then_pump(&mut w);
        // An op's completion has its room reserved at admission; every other event needs the one slot.
        let slotted = events
            .iter()
            .filter(|e| !matches!(e, Event::Completed { .. }))
            .count();
        assert!(slotted <= 1, "{events:?}");
        posted.extend(events);
        if posted
            .iter()
            .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
        {
            break;
        }
    }
    let names: Vec<&str> = posted
        .iter()
        .map(|e| match e {
            Event::SessionState { .. } => "state",
            Event::RouteAdopted { .. } => "adopted",
            Event::Completed { .. } => "completed",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(names, ["state", "adopted", "adopted", "completed"]);
}

/// R-90 add at post (#298), DP-8: a route that was `Stalled` arms its close step at the post of its `RouteAdopted`, from
/// that pump, not from the pump that consumed its record (a late poll).
#[test]
fn a_stalled_route_arms_its_close_step_at_the_route_adopted_post() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, Some(4))], 1);
    until_state_posted(&mut w);
    let route = route_ids(&w)[0].1;
    assert_eq!(due_at(&w, route), None, "nothing is armed before the post");
    w.advance(Duration::from_secs(30));
    let events = poll_then_pump(&mut w);
    assert!(
        matches!(events[..], [Event::SessionState { .. }]),
        "{events:?}"
    );
    assert_eq!(due_at(&w, route), Some(w.now + CLOSE_AFTER));
}

/// R-90 remove at consume (#298), DP-8: a resume that Core consumes before the `RouteAdopted` post ends the stall. The
/// post arms nothing, because the resume waits behind it.
#[test]
fn a_resume_consumed_before_the_route_adopted_post_arms_no_close_step() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, Some(4))], 1);
    until_state_posted(&mut w);
    let route = route_ids(&w)[0].1;
    w.worker_says("s", WorkerMsg::RouteResumed { route, mark: 5 });
    let mut events = poll_then_pump(&mut w);
    events.extend(poll_then_pump(&mut w));
    events.extend(poll_then_pump(&mut w));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::RouteResumed { route: r } if *r == route)),
        "{events:?}"
    );
    assert_eq!(due_at(&w, route), None);
}

/// DP-8, EV-6: a close that Core consumes before the `RouteAdopted` post posts after it. The row is done after the
/// `RouteAdopted`, and the route's entry goes with its close.
#[test]
fn a_close_before_the_route_adopted_post_posts_after_it() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, None)], 1);
    let op = until_state_posted(&mut w);
    let route = route_ids(&w)[0].1;
    w.worker_says(
        "s",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::PeerClosed,
            route_tag: Some("t1".into()),
        },
    );
    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(poll_then_pump(&mut w));
    }
    events.extend(w.engine.poll_events(64));
    let order: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::RouteAdopted { .. } => Some("adopted"),
            Event::RouteClosed { .. } => Some("closed"),
            Event::Completed { op: o, .. } if *o == op => Some("completed"),
            _ => None,
        })
        .collect();
    assert_eq!(order[..2], ["adopted", "completed"], "{events:?}");
    assert!(order.contains(&"closed"));
    assert!(order.iter().position(|e| *e == "adopted") < order.iter().position(|e| *e == "closed"));
    assert!(!w.engine.routes.contains_key(&route));
}

/// DP-8: `Detach` of an adopted route sends `HostMsg::Detach` with the route's new id, which the worker looks up.
#[test]
fn detach_of_an_adopted_route_names_its_new_id() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, None)], 100);
    adopt_all(&mut w);
    let route = route_ids(&w)[0].1;
    w.engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .expect("Detach is admitted");
    w.pump();
    assert!(w.sent.iter().any(|(_, m)| matches!(
        m,
        HostMsg::Detach { route: r, .. } if *r == route
    )));
}

/// Steward ruling R-99 (1), DP-8: a route of an adopted `Exited` session drains, and the worker reports its close
/// `SessionEnded` when its queue is delivered. No stop flow runs for that session, so the host posts the close at once,
/// with the session's exit.
#[test]
fn a_draining_route_of_an_adopted_exited_session_closes_with_the_sessions_exit() {
    let mut w = adopting_routes(
        AdoptedPayload::Exited {
            code: Some(3),
            signal: None,
        },
        vec![live(1, true, None)],
        100,
    );
    adopt_all(&mut w);
    let route = route_ids(&w)[0].1;
    let SessionState::Exited(exit) = w.engine.sessions[&sid("s")].shown.unwrap() else {
        panic!("the row is adopted Exited");
    };
    w.worker_says(
        "s",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::SessionEnded {
                exit: Exit {
                    code: Some(3),
                    signal: None,
                    cause: ExitCause::Other,
                },
            },
            route_tag: Some("t1".into()),
        },
    );
    let events = w.until(|e| matches!(e, Event::RouteClosed { .. }));
    assert_eq!(
        events.last(),
        Some(&Event::RouteClosed {
            route,
            reason: RouteCloseReason::SessionEnded { exit },
            route_tag: Some("t1".into()),
        })
    );
}

/// DP-8: a row whose state is `Lost` registers no route, and `AdoptAll` does not wait for one.
#[test]
fn a_lost_row_registers_no_route() {
    let mut w = adopting_routes(AdoptedPayload::NotLaunched, vec![live(1, true, None)], 100);
    rewrite_row(&mut w, "s", |row| row.state = SessionState::Running);
    adopt_all(&mut w);
    assert_eq!(route_ids(&w), vec![]);
    assert!(w.engine.routes.is_empty());
}

/// DP-8: the report counts two routes, and the link ends after one `RouteLive`. No more records come, so the row does not
/// wait for the second one: `AdoptAll` completes when the link ends.
#[test]
fn a_link_that_ends_before_every_route_live_ends_the_wait_for_them() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, None)], 100);
    for endpoint in w.endpoints.values_mut() {
        endpoint.unsent_routes = 1;
    }
    let op = until_state_posted(&mut w);
    let completed = |events: &[Event]| {
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op: o, .. } if *o == op))
    };
    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(poll_then_pump(&mut w));
    }
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::RouteAdopted { .. })),
        "{events:?}"
    );
    assert!(!completed(&events), "the row waits for the second record");
    let link = w.link_of("s");
    w.feed(Input::LinkClosed { link });
    let mut events = Vec::new();
    for _ in 0..5 {
        events.extend(poll_then_pump(&mut w));
    }
    assert!(completed(&events), "{events:?}");
}

/// The events of `route` among `events`, in order: "adopted" and "closed".
fn route_order(events: &[Event], route: RouteId) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::RouteAdopted { route: r, .. } if *r == route => Some("adopted"),
            Event::RouteClosed { route: r, .. } if *r == route => Some("closed"),
            _ => None,
        })
        .collect()
}

/// DP-8 phase 3, EV-6 (0181, #313): the session ends, and the worker reports the route's delivered close, before the
/// route's `RouteAdopted` posts. The stop flow closes the session's routes directly, so it waits for that post: the
/// caller sees the route before its close.
#[test]
fn a_session_end_before_the_route_adopted_post_closes_the_route_after_it() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, None)], 1);
    until_state_posted(&mut w);
    let route = route_ids(&w)[0].1;
    w.worker_says(
        "s",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.worker_says(
        "s",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::SessionEnded {
                exit: Exit {
                    code: Some(0),
                    signal: None,
                    cause: ExitCause::Other,
                },
            },
            route_tag: Some("t1".into()),
        },
    );
    let mut events = Vec::new();
    for _ in 0..10 {
        events.extend(poll_then_pump(&mut w));
    }
    assert_eq!(
        route_order(&events, route),
        ["adopted", "closed"],
        "{events:?}"
    );
}

/// DP-8 phase 3, EV-6: a `Detach` of a route whose link is gone closes it locally. That close waits for the route's
/// `RouteAdopted` post.
#[test]
fn a_local_detach_before_the_route_adopted_post_closes_the_route_after_it() {
    let mut w = adopting_routes(running_payload(), vec![live(1, true, None)], 1);
    until_state_posted(&mut w);
    let route = route_ids(&w)[0].1;
    let link = w.link_of("s");
    w.feed(Input::LinkClosed { link });
    w.engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .expect("Detach is admitted");
    let mut events = Vec::new();
    for _ in 0..10 {
        events.extend(poll_then_pump(&mut w));
    }
    assert_eq!(
        route_order(&events, route),
        ["adopted", "closed"],
        "{events:?}"
    );
}
