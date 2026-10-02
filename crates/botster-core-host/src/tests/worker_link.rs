//! The worker link: observations, losses, routes and the cached features.

use super::*;

/// Core AD-2, EV-4: a worker that ends while the session runs is `Lost(WorkerGone)`, and its pending read fails.
#[test]
fn a_worker_process_that_ends_is_lost_worker_gone() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.exited("s1");
    w.pump();
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    );
    // A later write to it is a certain zero (IN-7).
    let write = Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Text { text: "x".into() },
        guard: None,
    };
    match w.ok(write) {
        OpOutput::Input(r) => assert_eq!(
            r.outcome,
            WriteOutcome::NotWritten(NotWrittenReason::SessionEnded)
        ),
        other => panic!("{other:?}"),
    }
}

/// Core A2-1: a read on a session whose worker link ended is `WorkerLinkFailed`, and the session is not changed by it.
#[test]
fn a_read_over_a_broken_link_is_worker_link_failed() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    match w.run(Op::ReadModeFlags { session: sid("s1") }) {
        OpResult::Err(e) => assert_eq!(e.code, ErrorCode::WorkerLinkFailed),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Running
    );
}

/// Core A2-1: a read of a `Starting` session waits for the launch, then goes to the worker.
#[test]
fn a_read_of_a_starting_session_waits_for_the_launch() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    w.engine.poll_events(64);
    let read = w.engine.begin(Op::ReadModeFlags { session: sid("s1") });
    // Admitted in `Starting`: the start of the scripted worker is already through, so the read runs.
    let read = read.unwrap();
    assert!(matches!(w.complete(read), OpResult::Ok(OpOutput::Modes(_))));
}

/// Core A2-6, AD-4: a feature that the worker lacks makes the call `Unsupported`.
#[test]
fn a_session_whose_worker_lacks_a_feature_refuses_the_call() {
    let mut w = World::default();
    w.running("s1");
    assert_eq!(
        w.engine
            .begin(Op::SetNotificationPolicy {
                session: sid("s1"),
                policy: NotificationPolicy::None
            })
            .unwrap_err()
            .code,
        ErrorCode::Unsupported { what: None },
        "the scripted worker reports focus_report only"
    );
    w.ok(create("s2"));
    assert!(
        w.engine
            .begin(Op::SetNotificationPolicy {
                session: sid("s2"),
                policy: NotificationPolicy::None
            })
            .is_ok(),
        "a Created session has no worker, so Core stores the policy"
    );
}

/// Core A3-1 (`SetNotificationPolicy` in `Created`): the policy is stored in the durable row and reaches the worker at start.
#[test]
fn a_notification_policy_in_created_is_durable_and_reaches_the_worker_at_start() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.ok(Op::SetNotificationPolicy {
        session: sid("s1"),
        policy: NotificationPolicy::None,
    });
    let row: crate::session::Row = serde_json::from_slice(&w.rows["session/s1"]).unwrap();
    assert_eq!(
        row.request.notification_policy,
        Some(NotificationPolicy::None)
    );
    w.ok(Op::Start { id: sid("s1") });
    let launch = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Launch(spec) => Some(spec.clone()),
            _ => None,
        })
        .expect("the launch was sent");
    assert_eq!(launch.notification_policy, NotificationPolicy::None);
}

/// Core A2-1 (`env` is exact): the launch request carries the request's argv, env, cwd and size, and nothing else.
#[test]
fn the_launch_carries_the_request_exactly() {
    let mut w = World::default();
    let mut req = request();
    req.env = BTreeMap::from([("A".to_string(), "1".to_string())]);
    req.argv = vec!["/bin/echo".into(), "hi".into()];
    w.ok(Op::Create {
        session: sid("s1"),
        request: req.clone(),
    });
    w.ok(Op::Start { id: sid("s1") });
    let spec = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Launch(spec) => Some(spec.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (spec.argv, spec.env, spec.cwd),
        (req.argv, req.env, req.cwd)
    );
}

/// Core DP-7: `Detach` is sent to the worker and completes after `RouteClosed` is posted.
#[test]
fn detach_completes_after_the_route_closed_event() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let endpoint = StreamEndpoint::new(());
    let attached = w
        .engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(endpoint),
            AttachOptions {
                file_directory: "/tmp".into(),
                file_permissions: None,
                route_features: vec![],
                terminal_formats: vec![],
                connect_deadline: None,
                owner: None,
                query_deadline: Some(Duration::from_secs(1)),
                route_tag: Some("tag".into()),
                route_limits: None,
                history: None,
                stall_deadline: None,
                answers_queries: true,
                input: true,
            },
        )
        .unwrap();
    w.pump();
    let route = attached.route;
    assert!(w.trace.contains(&"handoff".to_string()));
    let detach = w
        .engine
        .begin(Op::Detach {
            route,
            reason: DetachReason::Detached,
        })
        .unwrap();
    w.pump();
    assert!(w
        .sent
        .iter()
        .any(|(_, m)| matches!(m, HostMsg::Detach { .. })));
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Detached,
            route_tag: Some("tag".into()),
        },
    );
    let events = w.until(|e| matches!(e, Event::Completed { op, .. } if *op == detach));
    let closed = events
        .iter()
        .position(|e| matches!(e, Event::RouteClosed { route_tag: Some(t), .. } if t == "tag"));
    assert!(
        closed.is_some() && closed < Some(events.len() - 1),
        "{events:?}"
    );
}

/// Core OU-1, ER-0: a route over `routes_per_session` is `RouteLimit`, a relative file directory is `InvalidInput`, and a
/// query deadline is required when the route answers queries.
#[test]
fn attach_refuses_what_it_can_refuse_at_registration() {
    let mut w = World::new(limits(|l| l.routes_per_session = 1));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let options = |dir: &str, deadline: Option<Duration>| AttachOptions {
        file_directory: dir.into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        connect_deadline: None,
        owner: None,
        query_deadline: deadline,
        route_tag: None,
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    };
    let attach = |w: &mut World, o| {
        w.engine.attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            o,
        )
    };
    assert_eq!(
        attach(&mut w, options("rel", Some(Duration::from_secs(1))))
            .unwrap_err()
            .error
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(
        attach(&mut w, options("/tmp", None))
            .unwrap_err()
            .error
            .code,
        ErrorCode::InvalidInput
    );
    assert!(attach(&mut w, options("/tmp", Some(Duration::from_secs(1)))).is_ok());
    assert_eq!(
        attach(&mut w, options("/tmp", Some(Duration::from_secs(1))))
            .unwrap_err()
            .error
            .code,
        ErrorCode::RouteLimit
    );
    w.ok(create("c"));
    let wrong = w.engine.attach(
        ClientId("c".into()),
        sid("c"),
        RouteTransport::Stream(StreamEndpoint::new(())),
        options("/tmp", Some(Duration::from_secs(1))),
    );
    assert_eq!(
        wrong.unwrap_err().error.code,
        ErrorCode::WrongState,
        "A2-1: a Created session takes no route"
    );
}

/// Core DP-2: the descriptor of a route that was registered before the launch goes to the worker when it runs.
#[test]
fn a_route_of_a_starting_session_is_handed_over_at_launch() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let options = AttachOptions {
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
    };
    w.engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options,
        )
        .unwrap();
    w.pump();
    assert!(!w.trace.contains(&"handoff".to_string()), "no link yet");
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::new(),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.pump();
    assert!(w.trace.contains(&"handoff".to_string()));
}

/// Core LC-7 step 1: `Remove` closes a route that is still bound with `SessionRemoved`.
#[test]
fn remove_closes_the_bound_routes_with_session_removed() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let options = AttachOptions {
        file_directory: "/tmp".into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        connect_deadline: None,
        owner: None,
        query_deadline: Some(Duration::from_secs(1)),
        route_tag: Some("t".into()),
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    };
    let route = w
        .engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options,
        )
        .unwrap()
        .route;
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.pump();
    w.engine.poll_events(64);
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
    assert!(events.iter().any(|e| matches!(e, Event::RouteClosed { route: r, reason: RouteCloseReason::SessionRemoved, .. } if *r == route)));
    assert_eq!(
        w.engine
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
                    query_deadline: None,
                    route_tag: None,
                    route_limits: None,
                    history: None,
                    stall_deadline: None,
                    answers_queries: false,
                    input: true,
                }
            )
            .unwrap_err()
            .error
            .code,
        ErrorCode::UnknownSession
    );
}

/// Core AD-4, A6-2: the adoptable set is {T} for T = 1, and the compatibility predicate follows it.
#[test]
fn the_first_protocol_has_no_previous_one() {
    let w = World::default();
    assert_eq!(w.engine.worker_protocol(), 1);
    assert_eq!(w.engine.adoptable_worker_protocols(), BTreeSet::from([1]));
    assert_eq!(
        w.engine.worker_protocol_compatibility(Some(1)),
        WorkerCompatibility::Compatible
    );
    assert_eq!(
        w.engine.worker_protocol_compatibility(Some(2)),
        WorkerCompatibility::Incompatible
    );
    assert_eq!(
        w.engine.worker_protocol_compatibility(Some(0)),
        WorkerCompatibility::Incompatible
    );
    assert_eq!(
        w.engine.worker_protocol_compatibility(None),
        WorkerCompatibility::Unknown
    );
}

/// Core 9B, A2-6, A2-8: `limits`, `features` and `terminal_identity` report what the engine was given.
#[test]
fn limits_features_and_identity_are_reported() {
    let w = World::new(limits(|l| l.max_sessions = 3));
    assert_eq!(w.engine.limits().max_sessions, 3);
    assert!(w.engine.features().names.contains(&Feature::Silence));
    assert_eq!(w.engine.terminal_identity().term, "xterm-ghostty");
}

/// Core DP-2, AM-4, OU-1, steward ruling R-19: every synchronous refusal of `attach` hands the caller's transport back,
/// untouched; the transport belongs to Core only from a successful return.
#[test]
fn a_refused_attach_hands_the_transport_back() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let options = |dir: &str| AttachOptions {
        file_directory: dir.into(),
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
    };
    let try_attach = |w: &mut World, session: &str, dir: &str| {
        w.engine.attach(
            ClientId("c".into()),
            sid(session),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options(dir),
        )
    };
    // UnknownSession, InvalidInput (a relative directory): the same transport object comes back.
    let refused = try_attach(&mut w, "nope", "/tmp").unwrap_err();
    assert_eq!(refused.error.code, ErrorCode::UnknownSession);
    assert!(matches!(refused.transport, RouteTransport::Stream(_)));
    let refused = try_attach(&mut w, "s1", "relative").unwrap_err();
    assert_eq!(refused.error.code, ErrorCode::InvalidInput);
    assert!(matches!(refused.transport, RouteTransport::Stream(_)));
    // RouteLimit: fill the routes of the session, then one more.
    let limit = w.engine.limits().routes_per_session;
    for _ in 0..limit {
        try_attach(&mut w, "s1", "/tmp").unwrap();
    }
    let refused = try_attach(&mut w, "s1", "/tmp").unwrap_err();
    assert_eq!(refused.error.code, ErrorCode::RouteLimit);
    assert!(matches!(refused.transport, RouteTransport::Stream(_)));
    assert_eq!(w.engine.sessions[&sid("s1")].routes.len(), limit as usize);
}
