//! DP-8 route adoption on the worker (design note `st-dp8-route-adoption-design.md`, sections 5 and 10): the live routes of
//! the report, the link table, and what a reported route holds back until its `RouteId` (STDP8-D1, D2, D3, D5). Steward
//! rulings R-98 and R-99.

use super::*;

/// The options of a route with both tags.
fn tagged() -> AttachOptions {
    serde_json::from_value(serde_json::json!({
        "file_directory": "/tmp",
        "route_tag": "tag",
        "owner": "owner",
    }))
    .unwrap()
}

/// [`attach`] for the client `client`.
fn attach_as(
    w: &mut World,
    descriptor: DescriptorId,
    route: RouteId,
    options: AttachOptions,
    client: &str,
) -> Vec<Action> {
    w.send_with(
        &HostMsg::AttachRoute {
            route,
            options,
            limits: limits(),
            client: ClientId(client.into()),
        },
        vec![descriptor],
    )
}

/// The new host's `RouteId` for `key`: the actions, and the worker's reports among them.
fn route_id(w: &mut World, key: u64, route: u64) -> (Vec<Action>, Vec<WorkerMsg>) {
    let actions = w.send(&HostMsg::RouteId {
        key,
        route: RouteId(route),
    });
    let reports = w.reports(&actions);
    (actions, reports)
}

/// The `ClientInput` observations among `reports`, as `(route, input_rev.client)`.
fn client_inputs(reports: &[WorkerMsg]) -> Vec<(RouteId, u64)> {
    reports
        .iter()
        .filter_map(|m| match m {
            WorkerMsg::Observed {
                observation: Observation::ClientInput { route, input_rev },
            } => Some((*route, input_rev.0)),
            _ => None,
        })
        .collect()
}

/// One complete client input frame on route `key`.
fn input(w: &mut World, key: u64, op: u64) -> Vec<Action> {
    w.feed(Input::RouteRead {
        route: RouteKey(key),
        bytes: client_frame(&bytes_frame(op, b"i")),
    })
}

fn session_ended(code: i32) -> RouteCloseReason {
    RouteCloseReason::SessionEnded {
        exit: Exit {
            code: Some(code),
            signal: None,
            cause: ExitCause::Other,
        },
    }
}

/// DP-8, R-99 (2): the report lists each live route once, in key order, with the tags of its attach and its applied limits.
/// A route that took its Closed transition (a `Detach` whose `route_closed` is not written yet) is not live: it is not
/// reported, and when it ends on the new link, the new host gets no `RouteClosed` for it.
#[test]
fn the_report_lists_each_live_route_with_its_attach_tags_and_limits() {
    let mut w = World::running();
    let mut one = Client::default();
    let a = attach_as(&mut w, DescriptorId(1), RouteId(4), tagged(), "alice");
    one.take_all(&mut w, RouteKey(1), a);
    let mut two = Client::default();
    let b = attach(&mut w, DescriptorId(2), RouteId(5), options(), limits());
    two.take_all(&mut w, RouteKey(2), b);
    // Route 6 is detached while its baseline is not written: its `route_closed` waits behind it.
    let detached = attach(&mut w, DescriptorId(3), RouteId(6), options(), limits());
    w.send(&HostMsg::Detach {
        route: RouteId(6),
        reason: DetachReason::Detached,
    });
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report, live) = adopted_routes(&mut w, &actions);
    assert_eq!(report.routes, 2);
    assert_eq!(
        live,
        vec![
            RouteLive {
                key: 1,
                route_tag: Some("tag".into()),
                owner: Some("owner".into()),
                client: ClientId("alice".into()),
                limits: limits(),
                stalled: None,
            },
            RouteLive {
                key: 2,
                route_tag: None,
                owner: None,
                client: ClientId("c".into()),
                limits: limits(),
                stalled: None,
            },
        ]
    );
    let mut three = Client::default();
    let n = detached.len();
    let all = three.take_all(&mut w, RouteKey(3), detached);
    assert!(all.contains(&Action::RouteClose { route: RouteKey(3) }));
    assert_eq!(
        three.frames.last(),
        Some(&route_closed(CloseReason::Detached, None))
    );
    assert_eq!(
        closes(&mut w, &all[n..]),
        vec![],
        "no close of a route that was not live"
    );
    // The route was not reported, so a `RouteId` for its key changes nothing.
    let (_, reports) = route_id(&mut w, 3, 6);
    assert_eq!(
        reports,
        vec![],
        "a key that the report did not list gets no id"
    );
}

/// A running worker whose one route drains for the session's end (OU-7): the exit is reported, and the route's tail and
/// `route_closed` wait in the returned actions.
fn draining_route() -> (World, Client, Vec<Action>) {
    let (mut w, client, _) = attached(limits());
    let mut actions = w.feed(Input::PtyOutput(b"bye".to_vec()));
    actions.extend(w.feed(Input::PayloadExited(ExitStatus::Code(3))));
    actions.extend(w.feed(Input::PtyDrained));
    assert!(!actions.contains(&Action::RouteClose { route: RouteKey(1) }));
    (w, client, actions)
}

/// R-99 (1), lead's required test: a route that drains after `Exited` is live, so the adoption reports it, and its
/// `RouteClosed{SessionEnded}` reaches the new host with the new id when the drain ends.
#[test]
fn a_route_that_drains_after_the_exit_is_reported_and_closes_with_its_new_id() {
    let (mut w, mut client, tail) = draining_route();
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, _, live) = adopted_routes(&mut w, &actions);
    assert_eq!(live.iter().map(|r| r.key).collect::<Vec<_>>(), [1]);
    route_id(&mut w, 1, 9);
    let n = tail.len();
    let all = client.take_all(&mut w, RouteKey(1), tail);
    assert_eq!(client.output().0, b"bye");
    assert_eq!(
        closes(&mut w, &all[n..]),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(9),
            reason: session_ended(3),
            route_tag: None,
        }]
    );
}

/// R-99, lead's added test, and STDP8-D2 (a close in the window): the route ends its drain after the adoption and before its
/// `RouteId`. It takes its Closed transition then, and its close waits: the `RouteId` sends it with the new id, once.
#[test]
fn a_drain_that_ends_before_the_route_id_closes_with_the_new_id_at_the_route_id() {
    let (mut w, mut client, tail) = draining_route();
    let actions = adopt(&mut w, EPOCH + 1);
    assert_eq!(adopted_routes(&mut w, &actions).1.routes, 1);
    let n = tail.len();
    let all = client.take_all(&mut w, RouteKey(1), tail);
    assert!(all.contains(&Action::RouteClose { route: RouteKey(1) }));
    assert_eq!(
        closes(&mut w, &all[n..]),
        vec![],
        "no id yet: the close waits"
    );
    let (_, reports) = route_id(&mut w, 1, 9);
    assert_eq!(
        reports,
        vec![WorkerMsg::RouteClosed {
            route: RouteId(9),
            reason: session_ended(3),
            route_tag: None,
        }]
    );
    let (_, again) = route_id(&mut w, 1, 9);
    assert_eq!(again, vec![], "the key is forgotten after its close");
}

/// STDP8-D2 and D5: client input in the window sends no `ClientInput` (no old id); the `RouteId` sends one, with the
/// worker's current client revision.
#[test]
fn client_input_before_the_route_id_is_reported_once_with_the_current_revision() {
    let (mut w, _, _) = attached(limits());
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report, _) = adopted_routes(&mut w, &actions);
    let snapshot = report.terminal.expect("a running model").input_rev.client.0;
    let mut window = input(&mut w, 1, 1);
    window.extend(w.feed(Input::PtyWritten(Ok(1))));
    window.extend(input(&mut w, 1, 2));
    assert_eq!(pty_writes(&window), b"ii", "the input reaches the PTY");
    assert_eq!(client_inputs(&w.reports(&window)), vec![]);
    let (_, reports) = route_id(&mut w, 1, 9);
    assert_eq!(client_inputs(&reports), vec![(RouteId(9), snapshot + 2)]);
}

/// STDP8-D2, ST-7: the end of a client input's transaction in the window sends no `ClientInputEnded` (no old id); the
/// `RouteId` sends one, after the route's `ClientInput`, so the new host raises the route's `Activity` edge.
#[test]
fn a_client_input_that_ends_before_the_route_id_is_reported_ended_once_at_the_route_id() {
    let (mut w, _, _) = attached(limits());
    let actions = adopt(&mut w, EPOCH + 1);
    adopted_routes(&mut w, &actions);
    let mut window = input(&mut w, 1, 1);
    window.extend(w.feed(Input::PtyWritten(Ok(1))));
    window.extend(input(&mut w, 1, 2));
    window.extend(w.feed(Input::PtyWritten(Ok(1))));
    assert_eq!(pty_writes(&window), b"ii", "both inputs are written");
    let ended = |reports: &[WorkerMsg]| -> Vec<&'static str> {
        reports
            .iter()
            .filter_map(|m| match m {
                WorkerMsg::Observed {
                    observation: Observation::ClientInput { route, .. },
                } if *route == RouteId(9) => Some("input"),
                WorkerMsg::Observed {
                    observation: Observation::ClientInputEnded { route },
                } if *route == RouteId(9) => Some("ended"),
                WorkerMsg::Observed {
                    observation:
                        Observation::ClientInput { .. } | Observation::ClientInputEnded { .. },
                } => Some("other route"),
                _ => None,
            })
            .collect()
    };
    assert_eq!(ended(&w.reports(&window)), Vec::<&str>::new());
    let (_, reports) = route_id(&mut w, 1, 9);
    assert_eq!(ended(&reports), ["input", "ended"]);
    let (_, again) = route_id(&mut w, 1, 9);
    assert_eq!(ended(&again), Vec::<&str>::new(), "the id is sent once");
}

/// STDP8-D5: the client revisions that the new host reads never decrease, whatever the order of the `RouteId` frames. Input
/// on B, then on A, in the window; `RouteId` for A; fresh input on A; `RouteId` for B.
#[test]
fn the_client_revisions_on_the_new_link_never_decrease() {
    let (mut w, _, _) = attached(limits());
    let mut b = Client::default();
    let attach_b = attach(&mut w, DescriptorId(2), RouteId(2), options(), limits());
    b.take_all(&mut w, RouteKey(2), attach_b);
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report, live) = adopted_routes(&mut w, &actions);
    assert_eq!(live.len(), 2);
    let r0 = report.terminal.expect("a running model").input_rev.client.0;
    let mut window = input(&mut w, 2, 1);
    window.extend(input(&mut w, 1, 2));
    assert_eq!(client_inputs(&w.reports(&window)), vec![]);
    let mut seen = client_inputs(&route_id(&mut w, 1, 7).1);
    let fresh = input(&mut w, 1, 3);
    seen.extend(client_inputs(&w.reports(&fresh)));
    seen.extend(client_inputs(&route_id(&mut w, 2, 8).1));
    assert_eq!(
        seen,
        vec![
            (RouteId(7), r0 + 2),
            (RouteId(7), r0 + 3),
            (RouteId(8), r0 + 3)
        ]
    );
}

/// STDP8-D1: a key never changes, so an adopted route's write and input in flight complete under it, while a new route of
/// the new host has the adopted route's old id. Each host id names its own route.
#[test]
fn crossed_ids_name_their_own_routes_with_a_write_and_input_in_flight() {
    let (mut w, mut one, _) = attached(limits());
    let output = w.feed(Input::PtyOutput(b"abcdef".to_vec()));
    let mut admitted = input(&mut w, 1, 1);
    assert_eq!(pty_writes(&admitted), b"i", "the input is admitted");
    let actions = adopt(&mut w, EPOCH + 1);
    assert_eq!(adopted_routes(&mut w, &actions).1.routes, 1);
    route_id(&mut w, 1, 7);
    // The new host gives a new route the old link's id of the adopted route.
    let mut two = Client::default();
    let attach_two = attach(&mut w, DescriptorId(2), RouteId(1), options(), limits());
    assert!(attach_two.contains(&Action::BindRoute {
        descriptor: DescriptorId(2),
        route: RouteKey(2)
    }));
    two.take_all(&mut w, RouteKey(2), attach_two);
    // The write in flight completes on its own route, in parts.
    let write = output
        .iter()
        .find_map(|a| match a {
            Action::RouteWrite { route, bytes } if *route == RouteKey(1) => Some(bytes.clone()),
            _ => None,
        })
        .expect("a write of the output");
    // The transport takes 2 bytes; the worker writes the rest, which `take_all` answers.
    one.reader.push(&write[..2]);
    let rest = w.feed(Input::RouteWritten {
        route: RouteKey(1),
        result: Ok(2),
    });
    admitted.extend(w.feed(Input::PtyWritten(Ok(1))));
    one.take_all(&mut w, RouteKey(1), rest);
    assert_eq!(one.output().0, b"abcdef");
    // Input on each route is reported with that route's id on the new link.
    let mut both = input(&mut w, 1, 2);
    both.extend(w.feed(Input::PtyWritten(Ok(1))));
    both.extend(input(&mut w, 2, 3));
    let routes: Vec<RouteId> = client_inputs(&w.reports(&both))
        .into_iter()
        .map(|(route, _)| route)
        .collect();
    assert_eq!(routes, vec![RouteId(7), RouteId(1)]);
    // `Detach` of id 1 closes the new route only.
    let detach = w.send(&HostMsg::Detach {
        route: RouteId(1),
        reason: DetachReason::Detached,
    });
    let all = two.take_all(&mut w, RouteKey(2), detach);
    assert!(all.contains(&Action::RouteClose { route: RouteKey(2) }));
    assert!(!all.contains(&Action::RouteClose { route: RouteKey(1) }));
    assert_eq!(
        closes(&mut w, &all),
        vec![WorkerMsg::RouteClosed {
            route: RouteId(1),
            reason: RouteCloseReason::Detached,
            route_tag: None,
        }]
    );
}

/// STDP8-D2, a second host loss before `RouteId`: the next adoption clears the table again and reports every route that is
/// live then, also a route that never got an id. A route that closed in the first window is not live: its held close is
/// dropped, and no host is told.
#[test]
fn a_second_adoption_before_the_route_ids_reports_the_routes_that_are_still_live() {
    let (mut w, _, _) = attached(limits());
    let mut two = Client::default();
    let attach_two = attach(&mut w, DescriptorId(2), RouteId(2), options(), limits());
    two.take_all(&mut w, RouteKey(2), attach_two);
    let first = adopt(&mut w, EPOCH + 1);
    assert_eq!(adopted_routes(&mut w, &first).1.routes, 2);
    let ended = w.feed(Input::RouteEnded { route: RouteKey(2) });
    assert!(ended.contains(&Action::RouteClose { route: RouteKey(2) }));
    assert_eq!(closes(&mut w, &ended), vec![]);
    let mut second = w.feed(Input::Candidate(CandidateId(2)));
    second.extend(w.feed(Input::CandidateBytes(
        CandidateId(2),
        World::host_hello(instance(), EPOCH + 2, TOKEN),
    )));
    let at = second
        .iter()
        .position(|a| *a == Action::AdoptLink(CandidateId(2)))
        .expect("the second host replaced the link");
    let frames = w.frames(&second[at + 1..]);
    let msgs: Vec<WorkerMsg> = frames[1..]
        .iter()
        .map(|f| WorkerMsg::decode(&f.payload).unwrap())
        .collect();
    let [WorkerMsg::Adopted { report }, WorkerMsg::RouteLive(live)] = &msgs[..] else {
        panic!("{msgs:?}");
    };
    assert_eq!((report.routes, live.key), (1, 1));
    let (_, gone) = route_id(&mut w, 2, 5);
    assert_eq!(gone, vec![], "the closed route has no key on this link");
    let (_, reports) = route_id(&mut w, 1, 5);
    assert_eq!(reports, vec![], "route 1 holds nothing back");
}

/// STDP8-D3: each route has its own `RouteLive` frame, so a route set whose records together pass one frame's bound adopts
/// fully. Each `AttachRoute` fits the 64 KiB link bound; the three `RouteLive` records together do not.
#[test]
fn a_route_set_larger_than_one_frame_adopts_fully() {
    let mut w = World::running();
    let tag = "t".repeat(25 * 1024);
    for n in 1..=3 {
        let mut options = options();
        options.route_tag = Some(tag.clone());
        let mut client = Client::default();
        let actions = attach(&mut w, DescriptorId(n), RouteId(n), options, limits());
        client.take_all(&mut w, RouteKey(n), actions);
    }
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report, live) = adopted_routes(&mut w, &actions);
    assert_eq!(report.routes, 3);
    assert!(live
        .iter()
        .all(|r| r.route_tag.as_deref() == Some(tag.as_str())));
    let total: usize = live
        .iter()
        .map(|r| serde_json::to_vec(r).unwrap().len())
        .sum();
    assert!(total > 1 << 16, "{total}");
}

/// STDP8-D3, defence only: a `RouteLive` that the link's bound cannot carry is found before the report. The bound shrinks
/// after the attach here, which no link does, so the debug assertion fires (a release build closes the route
/// `HandoffFailed` and does not count it).
#[test]
#[should_panic(expected = "a RouteLive over the frame bound")]
fn a_route_live_over_the_frame_bound_is_found_before_the_report() {
    let mut w = World::running();
    let mut options = options();
    options.route_tag = Some("t".repeat(4 * 1024));
    let mut client = Client::default();
    let actions = attach(&mut w, DescriptorId(1), RouteId(1), options, limits());
    client.take_all(&mut w, RouteKey(1), actions);
    w.worker.frame_bound = 1024;
    adopt(&mut w, EPOCH + 1);
}
