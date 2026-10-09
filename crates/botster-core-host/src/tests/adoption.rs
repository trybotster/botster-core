//! The adoption of a row whose worker may live (Core AD-1, AD-2, AD-4, AD-6, A10-1, LC-11; steward ruling R-35, contracts
//! `main` `f969f5e`, with the correction `c3ed727`; steward ruling R-36, contracts `main` `c62085f`, with the follow-up
//! `d18b6de`; DESIGN.md "Adoption (P5)").
//!
//! A first host leaves its rows and its workers (LC-12). A new host opened over it (`World::over`) adopts them: the scripted
//! worker at each endpoint (`Endpoint`) answers the new host's hello and reports its payload. For `AdoptAll`, the row's
//! recorded state and the reported payload state decide the row's one `SessionState` (R-35). For `Adopt(id)` of a `Lost`
//! session, the report alone decides it, and nothing is launched (R-36).

use super::*;
use crate::session::Row;

fn row_key(name: &str) -> String {
    format!("{}{name}", crate::session::ROW_PREFIX)
}

/// A first host whose session `name` ended between AD-7 steps 3 and 4: the row is `Starting` with the worker's identity,
/// and no `Launch` was sent.
fn crashed_before_launch(name: &str) -> World {
    let mut first = World::default();
    first.autopilot = Autopilot::Silent;
    first.ok(create(name));
    first.engine.begin(Op::Start { id: sid(name) }).unwrap();
    first.pump();
    let row = Row::decode(&sid(name), &first.rows[&row_key(name)]).expect("the row decodes");
    assert_eq!(row.state, SessionState::Starting);
    assert!(row.worker.is_some(), "AD-7 step 3: the identity is durable");
    assert_eq!(launches(&first), 0, "AD-7 step 4 did not happen");
    first
}

/// Writes the row of `name` again with Core's own encoder, changed by `change`.
fn rewrite_row(w: &mut World, name: &str, change: impl FnOnce(&mut Row)) {
    let mut row = Row::decode(&sid(name), &w.rows[&row_key(name)]).expect("the row decodes");
    change(&mut row);
    w.rows
        .insert(row_key(name), serde_json::to_vec(&row).unwrap());
}

/// A new host over `first`, whose worker of `name` reports `payload` after its hello.
fn adopting(first: &World, name: &str, payload: AdoptedPayload) -> World {
    with_endpoint(first, name, Some(payload))
}

/// A new host over `first`, whose worker of `name` accepts a connect at its endpoint. With `None` it answers no hello.
fn with_endpoint(first: &World, name: &str, payload: Option<AdoptedPayload>) -> World {
    let mut again = World::over(first);
    again.endpoints.insert(
        first.instance_of(name),
        Endpoint {
            token: first.token_of(name),
            protocol: HELLO_PROTOCOL,
            payload,
        },
    );
    again
}

fn launches(w: &World) -> usize {
    w.sent
        .iter()
        .filter(|(_, m)| matches!(m, HostMsg::Launch(_)))
        .count()
}

/// Runs `AdoptAll` and returns its events, the completion last (A2-1: `Ok(())`).
fn adopt_all(w: &mut World) -> Vec<Event> {
    let adopt = w.engine.begin(Op::AdoptAll).expect("AdoptAll is admitted");
    let events = w.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        events.last(),
        Some(&Event::Completed {
            op: adopt,
            result: OpResult::Ok(OpOutput::Unit)
        })
    );
    events
}

/// The states that `events` post for `name`, in order.
fn states_of(events: &[Event], name: &str) -> Vec<SessionState> {
    states(events)
        .into_iter()
        .filter(|(id, _)| id == name)
        .map(|(_, state)| state)
        .collect()
}

fn running_payload() -> AdoptedPayload {
    AdoptedPayload::Running {
        payload: botster_core_link::msg::PayloadId {
            pid: 900,
            start_time: 3,
        },
    }
}

fn never_ran(cause: ExitCause) -> SessionState {
    SessionState::Exited(Exit {
        code: None,
        signal: None,
        cause,
    })
}

/// Core AD-1, AD-7, LC-11; steward ruling R-35 (a): a `Starting` row whose worker got no `Launch` is adopted. The new host
/// sends the one `Launch` built from the row, and the row's one `SessionState` is `Running`, before the completion of
/// `AdoptAll`. The engine's proof of `conf::ad_1_starting_with_identity_adopts` and
/// `conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`.
#[test]
fn a_starting_row_whose_worker_got_no_launch_runs_after_one_launch() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![SessionState::Running]);
    assert_eq!(launches(&again), 1, "R-35 (a): exactly one Launch");
    let argv = again.sent.iter().find_map(|(_, m)| match m {
        HostMsg::Launch(spec) => Some(spec.argv.clone()),
        _ => None,
    });
    assert_eq!(
        argv,
        Some(request().argv),
        "the Launch is built from the row"
    );
    assert_eq!(again.connects, vec![first.instance_of("s")]);
    assert_eq!(
        again.engine.get(&sid("s")).unwrap().state,
        SessionState::Running
    );
    assert_eq!(
        again.instance_of("s"),
        first.instance_of("s"),
        "ID-2: the instance survives adoption"
    );
}

/// Steward ruling R-35 (a), LC-4: an adopted `Launch` that fails gives the same outcome as a failed launch of an ordinary
/// `Start` of the same request.
#[test]
fn an_adopted_launch_that_fails_ends_as_an_ordinary_failed_start() {
    let failing = Op::Create {
        session: sid("s"),
        request: SpawnRequest {
            cwd: "/no-such".into(),
            ..request()
        },
    };
    let mut ordinary = World::default();
    ordinary.ok(failing.clone());
    assert!(matches!(
        ordinary.run(Op::Start { id: sid("s") }),
        OpResult::Err(_)
    ));
    let expected = ordinary.engine.get(&sid("s")).unwrap().state;
    assert!(matches!(expected, SessionState::Exited(_)), "{expected:?}");

    let mut first = World::default();
    first.autopilot = Autopilot::Silent;
    first.ok(failing);
    first.engine.begin(Op::Start { id: sid("s") }).unwrap();
    first.pump();
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![expected]);
    assert_eq!(launches(&again), 1);
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(
        row.state, expected,
        "the end is written, as for a failed start"
    );
}

/// Steward ruling R-35 (b): a spawn in flight is adopted with no second `Launch`. The row posts nothing until the spawn
/// answers, no operation is admitted on the session meanwhile, and `AdoptAll` completes after the row's state.
#[test]
fn a_spawn_in_flight_adopts_with_no_second_launch() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::Spawning);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let early = again.engine.poll_events(64);
    assert!(states_of(&early, "s").is_empty(), "{early:?}");
    assert!(
        !early.iter().any(|e| matches!(e, Event::Completed { .. })),
        "{early:?}"
    );
    assert_eq!(
        again
            .engine
            .begin(Op::Stop { id: sid("s") })
            .unwrap_err()
            .code,
        ErrorCode::UnknownSession,
        "the row is no session of this handle until its state is posted"
    );
    assert_eq!(
        again.engine.get(&sid("s")).unwrap_err().code,
        ErrorCode::UnknownSession
    );
    again.worker_says(
        "s",
        WorkerMsg::Launched {
            features: BTreeSet::from([Feature::FocusReport]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    );
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(states_of(&events, "s"), vec![SessionState::Running]);
    assert_eq!(launches(&again), 0, "R-35 (b): no second Launch");
}

/// Core AD-1: a `Starting` row whose payload runs is adopted `Running` with the payload's identity, and no `Launch`.
#[test]
fn a_running_payload_adopts_running_with_its_identity() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", running_payload());
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![SessionState::Running]);
    assert_eq!(launches(&again), 0);
    assert_eq!(
        again.engine.sessions[&sid("s")].payload,
        Some(ProcessIdentity {
            pid: 900,
            start_time: 3
        })
    );
}

/// Core AD-1, ST-5: an `Exited` row whose final-model worker lives is adopted `Exited`, with the exit that the row recorded.
#[test]
fn an_exited_row_with_a_live_worker_adopts_exited_with_its_exit() {
    let mut first = World::default();
    first.running("s");
    first.worker_says(
        "s",
        WorkerMsg::Exited {
            code: Some(3),
            signal: None,
        },
    );
    first.until(|e| {
        matches!(
            e,
            Event::SessionState {
                state: SessionState::Exited(_),
                ..
            }
        )
    });
    let recorded = Row::decode(&sid("s"), &first.rows[&row_key("s")])
        .unwrap()
        .state;
    assert!(matches!(recorded, SessionState::Exited(_)), "{recorded:?}");
    let mut again = adopting(
        &first,
        "s",
        AdoptedPayload::Exited {
            code: Some(3),
            signal: None,
        },
    );
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![recorded]);
    assert_eq!(
        again.rows[&row_key("s")],
        first.rows[&row_key("s")],
        "the row records this end already"
    );
    // ST-5 (review P5-F23): the final-model worker keeps its link, so the adopted session serves reads.
    assert!(matches!(
        again.ok(Op::ReadModeFlags { session: sid("s") }),
        OpOutput::Modes(_)
    ));
}

/// Steward ruling R-35 (c): a `Stopping` row with no payload ends `Exited{cause: HostStop}` with no code and no signal, and
/// no `Launch`.
#[test]
fn a_stopping_row_with_no_payload_exits_host_stop_with_no_launch() {
    for payload in [
        AdoptedPayload::NotLaunched,
        AdoptedPayload::LaunchFailed {
            reason: StartFailReason::CwdMissing,
        },
    ] {
        let mut first = crashed_before_launch("s");
        rewrite_row(&mut first, "s", |row| row.state = SessionState::Stopping);
        let mut again = adopting(&first, "s", payload);
        let events = adopt_all(&mut again);
        assert_eq!(
            states_of(&events, "s"),
            vec![never_ran(ExitCause::HostStop)],
            "{payload:?}"
        );
        assert_eq!(launches(&again), 0, "{payload:?}");
    }
}

/// Core AD-1: a `Stopping` row whose payload runs is adopted `Stopping`; the stop is sent again, and the session then ends
/// `Exited{cause: HostStop}`.
#[test]
fn a_stopping_row_whose_payload_runs_resends_the_stop() {
    let mut first = World::default();
    first.running("s");
    first.autopilot = Autopilot::Silent;
    first.engine.begin(Op::Stop { id: sid("s") }).unwrap();
    first.pump();
    let row = Row::decode(&sid("s"), &first.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Stopping);
    let mut again = adopting(&first, "s", running_payload());
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![SessionState::Stopping]);
    assert!(
        again.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "AD-1: Core re-issues the stop"
    );
    // The scripted worker answers the stop at once: the session ends as the host asked.
    again.pump();
    let state = again.engine.get(&sid("s")).unwrap().state;
    assert!(
        matches!(
            state,
            SessionState::Exited(Exit {
                cause: ExitCause::HostStop,
                ..
            })
        ),
        "{state:?}"
    );
}

/// Steward ruling R-35 (d), with the correction `c3ed727`: a `Running` or `Exited` row whose authenticated worker reports
/// no payload that ever ran is `Lost(RegistryCorrupt)`. No `Launch` is sent and no process is signalled.
#[test]
fn a_running_or_exited_row_with_no_launched_payload_is_registry_corrupt() {
    let exited = never_ran(ExitCause::Normal);
    for recorded in [SessionState::Running, exited] {
        for payload in [AdoptedPayload::NotLaunched, AdoptedPayload::Spawning] {
            let mut first = crashed_before_launch("s");
            rewrite_row(&mut first, "s", |row| row.state = recorded);
            let mut again = adopting(&first, "s", payload);
            let events = adopt_all(&mut again);
            assert_eq!(
                states_of(&events, "s"),
                vec![SessionState::Lost(LostReason::RegistryCorrupt)],
                "{recorded:?} {payload:?}"
            );
            assert_eq!(launches(&again), 0);
            assert!(again.signals.is_empty(), "{:?}", again.signals);
        }
    }
}

/// Core AD-1: a `Starting` row with no recorded worker identity is `Lost(StartInterrupted)`, with no probe and no connect.
#[test]
fn a_starting_row_without_identity_is_start_interrupted() {
    let mut first = crashed_before_launch("s");
    rewrite_row(&mut first, "s", |row| row.worker = None);
    let mut again = World::over(&first);
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::StartInterrupted)]
    );
    assert!(again.connects.is_empty());
}

/// Core AD-2, AD-6: a row whose worker identity matches no live process is `Lost(WorkerGone)`. Core connects to nothing and
/// signals nothing.
#[test]
fn a_row_whose_worker_is_gone_is_worker_gone() {
    let mut first = World::default();
    first.running("s");
    let identity = first.identity_of("s");
    first.alive.remove(&identity);
    let mut again = adopting(&first, "s", running_payload());
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerGone)]
    );
    assert!(again.connects.is_empty());
    assert!(again.signals.is_empty());
}

/// Core AD-6, A10-1, A11-1: a process whose hello does not prove the token is not the session's worker. It is never
/// signalled, its link is closed, and the row is `Lost(WorkerGone)` with its id kept (lead ruling on #176a-2).
#[test]
fn a_hello_with_a_wrong_proof_is_worker_gone_and_never_signalled() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    again
        .endpoints
        .get_mut(&first.instance_of("s"))
        .unwrap()
        .token = [0xEE; TOKEN_LEN];
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerGone)]
    );
    assert!(again.signals.is_empty(), "{:?}", again.signals);
    assert_eq!(again.closed.len(), 1, "the link is closed");
    assert_eq!(launches(&again), 0);
    assert_eq!(
        again.engine.begin(create("s")).unwrap_err().code,
        ErrorCode::IdInUse
    );
    // A10-1, AD-6: `Remove` never signals the recorded identity either, and the worker's cleanup is not known (A6-3).
    let remove = again.engine.begin(Op::Remove { id: sid("s") }).unwrap();
    assert!(matches!(
        again.complete(remove),
        OpResult::Ok(OpOutput::RemoveReport(r))
            if r.uploads == UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
    ));
    assert!(again.signals.is_empty(), "{:?}", again.signals);
}

/// Core AD-4, A6-2, LC-9: a worker whose protocol is outside the adoptable set is `Lost(WorkerVersion)`. Its protocol is
/// recorded, and it is not signalled.
#[test]
fn an_out_of_set_protocol_is_worker_version_with_the_protocol_recorded() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    again
        .endpoints
        .get_mut(&first.instance_of("s"))
        .unwrap()
        .protocol = 9;
    assert!(!again.engine.adoptable_worker_protocols().contains(&9));
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerVersion)]
    );
    assert_eq!(
        again.engine.get(&sid("s")).unwrap().worker_protocol,
        Some(9)
    );
    assert!(again.signals.is_empty());
    assert_eq!(launches(&again), 0);
}

/// DESIGN.md "Adoption (P5)" 3.7, AD-2: a worker that does not answer within `startup` is `Lost(WorkerUnreachable)`, and the
/// worker protocol stays absent because no hello was read (LC-9).
#[test]
fn an_unanswered_adoption_is_unreachable_at_the_startup_deadline() {
    let first = crashed_before_launch("s");
    let mut again = with_endpoint(&first, "s", None);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    assert!(states_of(&again.engine.poll_events(64), "s").is_empty());
    let startup = again.engine.cfg.limits.startup;
    again.advance(startup);
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    assert_eq!(again.engine.get(&sid("s")).unwrap().worker_protocol, None);
    assert!(again.signals.is_empty());
}

/// Core LC-9, AD-2: the row of a running session records its worker's protocol. A new host that never reads the worker's
/// hello does not show that protocol: the session is `Lost(WorkerUnreachable)` with no protocol. The `Lost` row keeps the
/// protocol that it recorded (R-36: it records only its end).
#[test]
fn an_unanswered_adoption_drops_the_protocol_that_the_row_recorded() {
    let mut first = World::default();
    first.ok(create("s"));
    first.ok(Op::Start { id: sid("s") });
    let row = Row::decode(&sid("s"), &first.rows[&row_key("s")]).expect("the row decodes");
    assert_eq!(row.worker_protocol, Some(HELLO_PROTOCOL));
    let mut again = with_endpoint(&first, "s", None);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let startup = again.engine.cfg.limits.startup;
    again.advance(startup);
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    assert_eq!(again.engine.get(&sid("s")).unwrap().worker_protocol, None);
    let lost = Row::decode(&sid("s"), &again.rows[&row_key("s")]).expect("the row decodes");
    assert_eq!(
        lost.state,
        SessionState::Lost(LostReason::WorkerUnreachable)
    );
    assert_eq!(lost.worker_protocol, Some(HELLO_PROTOCOL));
}

/// Core AD-2; steward ruling R-36 (contracts `main` `c62085f`): an adoption whose link is lost after the `Launch` is
/// `Lost(WorkerUnreachable)`, and the row records it with the worker's identity. `Adopt(id)` re-reads the worker: its payload
/// runs, so the session is `Running`, with no second `Launch`, and the row records `Running`.
#[test]
fn a_retry_after_an_accepted_launch_sends_no_second_launch() {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    again.autopilot = Autopilot::Silent;
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    assert_eq!(launches(&again), 1);
    let link = again.link_of("s");
    again.feed(Input::LinkClosed { link });
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Lost(LostReason::WorkerUnreachable));
    assert_eq!(
        row.worker.map(|w| w.identity()),
        Some(first.identity_of("s")),
        "R-36: a Lost row keeps the worker's identity"
    );
    // The worker accepted the `Launch`: its payload runs.
    again
        .endpoints
        .get_mut(&first.instance_of("s"))
        .unwrap()
        .payload = Some(running_payload());
    let record = match again.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(record.state, SessionState::Running);
    assert_eq!(launches(&again), 1, "no second Launch");
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Running);
}

/// A handle whose adopted `Launch` was lost with its link: the session is `Lost(WorkerUnreachable)`, and the worker's
/// report is still `NotLaunched`.
fn lost_before_the_launch() -> (World, World) {
    let first = crashed_before_launch("s");
    let mut again = adopting(&first, "s", AdoptedPayload::NotLaunched);
    again.autopilot = Autopilot::Silent;
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let link = again.link_of("s");
    again.feed(Input::LinkClosed { link });
    again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    (first, again)
}

fn retry_reports(w: &mut World, first: &World, payload: AdoptedPayload) {
    w.endpoints
        .get_mut(&first.instance_of("s"))
        .unwrap()
        .payload = Some(payload);
}

/// Core AD-2; steward ruling R-36 with the follow-up `d18b6de`: `Adopt(id)` sends no `Launch` on a `Lost` row. A worker that
/// reports `NotLaunched` gives `Lost(StartInterrupted)`, and the row records it.
#[test]
fn a_retry_whose_worker_got_no_launch_is_start_interrupted_with_no_launch() {
    let (_, mut again) = lost_before_the_launch();
    assert_eq!(launches(&again), 1);
    let record = match again.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    let interrupted = SessionState::Lost(LostReason::StartInterrupted);
    assert_eq!(record.state, interrupted);
    assert_eq!(launches(&again), 1, "R-36: nothing is relaunched");
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, interrupted);
    assert_eq!(
        again
            .engine
            .begin(Op::Adopt { id: sid("s") })
            .unwrap_err()
            .code,
        ErrorCode::WrongState,
        "StartInterrupted is no indeterminate Lost"
    );
}

/// Steward ruling R-36 with the follow-up `d18b6de`, as R-35 (b): a retry that finds the spawn in flight sends no `Launch`.
/// The session keeps its `Lost` state until the spawn answers, then it is `Running`.
#[test]
fn a_retry_that_finds_a_spawn_in_flight_waits_with_no_launch() {
    let (first, mut again) = lost_before_the_launch();
    retry_reports(&mut again, &first, AdoptedPayload::Spawning);
    let retry = again.engine.begin(Op::Adopt { id: sid("s") }).unwrap();
    again.pump();
    let early = again.engine.poll_events(64);
    assert!(states_of(&early, "s").is_empty(), "{early:?}");
    assert!(
        !early.iter().any(|e| matches!(e, Event::Completed { .. })),
        "{early:?}"
    );
    again.worker_says(
        "s",
        WorkerMsg::Launched {
            features: BTreeSet::from([Feature::FocusReport]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    );
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == retry));
    assert_eq!(states_of(&events, "s"), vec![SessionState::Running]);
    assert!(matches!(
        events.last(),
        Some(Event::Completed {
            result: OpResult::Ok(OpOutput::Record(_)),
            ..
        })
    ));
    assert_eq!(launches(&again), 1, "no Launch on a Lost row");
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Running);
}

/// Steward ruling R-36 with the follow-up `d18b6de`, LC-4: a retry whose worker reports a failed spawn ends as an ordinary
/// failed start, and the end is written.
#[test]
fn a_retry_that_finds_a_failed_spawn_ends_as_a_failed_start() {
    let (first, mut again) = lost_before_the_launch();
    retry_reports(
        &mut again,
        &first,
        AdoptedPayload::LaunchFailed {
            reason: StartFailReason::CwdMissing,
        },
    );
    let record = match again.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(record.state, never_ran(ExitCause::Other));
    assert_eq!(launches(&again), 1);
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, never_ran(ExitCause::Other));
}

/// Core AD-2, A2-1 `Adopt`: only a `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` session can be adopted again, and a
/// retry in flight refuses a second one.
#[test]
fn adopt_is_admitted_only_for_an_indeterminate_lost_session() {
    let mut w = World::default();
    w.ok(create("c"));
    assert_eq!(
        w.engine.begin(Op::Adopt { id: sid("c") }).unwrap_err().code,
        ErrorCode::WrongState
    );
    let first = crashed_before_launch("s");
    let mut again = with_endpoint(&first, "s", None);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let startup = again.engine.cfg.limits.startup;
    again.advance(startup);
    again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    again.engine.begin(Op::Adopt { id: sid("s") }).unwrap();
    assert_eq!(
        again
            .engine
            .begin(Op::Adopt { id: sid("s") })
            .unwrap_err()
            .code,
        ErrorCode::WrongState,
        "a retry runs already"
    );
}

/// Steward ruling R-35 (a), LC-4: a `Starting` row whose worker reports a failed spawn ends as an ordinary failed start, with
/// no `Launch`, and the end is written.
#[test]
fn a_starting_row_with_a_failed_spawn_ends_as_a_failed_start() {
    let first = crashed_before_launch("s");
    let mut again = adopting(
        &first,
        "s",
        AdoptedPayload::LaunchFailed {
            reason: StartFailReason::CwdMissing,
        },
    );
    let events = adopt_all(&mut again);
    assert_eq!(states_of(&events, "s"), vec![never_ran(ExitCause::Other)]);
    assert_eq!(launches(&again), 0);
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, never_ran(ExitCause::Other));
}

/// Core AD-1: a `Starting` row whose payload ended meanwhile is adopted `Exited` with the reported code, and the end is
/// written.
#[test]
fn a_starting_row_whose_payload_ended_adopts_exited_and_writes_it() {
    let first = crashed_before_launch("s");
    let mut again = adopting(
        &first,
        "s",
        AdoptedPayload::Exited {
            code: Some(0),
            signal: None,
        },
    );
    let events = adopt_all(&mut again);
    let exited = SessionState::Exited(Exit {
        code: Some(0),
        signal: None,
        cause: ExitCause::Normal,
    });
    assert_eq!(states_of(&events, "s"), vec![exited]);
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, exited);
    // ST-5 (review P5-F23): the worker keeps the final model, and its link stays.
    assert!(matches!(
        again.ok(Op::ReadModeFlags { session: sid("s") }),
        OpOutput::Modes(_)
    ));
}

/// Core AD-1, LC-5: a `Stopping` row whose payload ended is `Exited{cause: HostStop}` with the reported signal.
#[test]
fn a_stopping_row_whose_payload_ended_exits_host_stop_with_its_signal() {
    let mut first = crashed_before_launch("s");
    rewrite_row(&mut first, "s", |row| row.state = SessionState::Stopping);
    let mut again = adopting(
        &first,
        "s",
        AdoptedPayload::Exited {
            code: None,
            signal: Some(15),
        },
    );
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Exited(Exit {
            code: None,
            signal: Some(15),
            cause: ExitCause::HostStop,
        })]
    );
}

/// Core AD-2: a link of an adoption that ends before the report leaves the worker indeterminate: `Lost(WorkerUnreachable)`
/// at once, before the `startup` deadline.
#[test]
fn a_link_that_ends_before_the_report_is_unreachable() {
    let first = crashed_before_launch("s");
    let mut again = with_endpoint(&first, "s", None);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let link = again.link_of("s");
    again.feed(Input::LinkClosed { link });
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
}

/// Core AD-2: a worker that ends during its adoption is `Lost(WorkerGone)`.
#[test]
fn a_worker_that_ends_during_its_adoption_is_worker_gone() {
    let first = crashed_before_launch("s");
    let mut again = with_endpoint(&first, "s", None);
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    again.pump();
    let identity = again.identity_of("s");
    again.feed(Input::ProcessExited {
        identity,
        status: ExitStatus::Code(0),
    });
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerGone)]
    );
}

/// Core EV-5b, EV-5d, LC-11: the state of an adopted row waits for queue room; meanwhile the row is no session and
/// `AdoptAll` does not complete. A poll that frees room lets it post.
#[test]
fn an_adopted_state_waits_for_queue_room() {
    let mut first = crashed_before_launch("a");
    first.ok(create("b"));
    first.engine.cfg.limits.mandatory_events = 1;
    let mut again = adopting(&first, "a", running_payload());
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    let report = again.pump();
    assert!(!report.more, "the parked step is not runnable work (TM-6)");
    assert_eq!(
        again.connects,
        vec![first.instance_of("a")],
        "the probe, the connect and the handshake post nothing, so they run while the queue is full"
    );
    assert_eq!(
        again.engine.get(&sid("a")).unwrap_err().code,
        ErrorCode::UnknownSession
    );
    let first_events = again.engine.poll_events(64);
    assert_eq!(
        states(&first_events),
        vec![("b".to_string(), SessionState::Created)]
    );
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(states_of(&events, "a"), vec![SessionState::Running]);
}

/// Core AD-1, AD-2: a row that recorded its end posts it; a recorded `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` is
/// left for `Adopt(id)` (steward ruling R-36). A recorded `Lost(Other)`, which Core never writes, and a row with a worker
/// identity but no token are corrupt records (R-35 correction `c3ed727`). None of them is probed or connected.
#[test]
fn recorded_ends_and_inconsistent_rows_post_without_a_handshake() {
    let mut first = crashed_before_launch("gone");
    let corrupt = ["other", "tokenless"];
    for name in corrupt.into_iter().chain(["unreachable", "version"]) {
        first.ok(create(name));
        let template = first.rows[&row_key("gone")].clone();
        let mut row = Row::decode(&sid("gone"), &template).unwrap();
        row.id = sid(name);
        first
            .rows
            .insert(row_key(name), serde_json::to_vec(&row).unwrap());
    }
    rewrite_row(&mut first, "gone", |row| {
        row.state = SessionState::Lost(LostReason::WorkerGone)
    });
    rewrite_row(&mut first, "other", |row| {
        row.state = SessionState::Lost(LostReason::Other)
    });
    rewrite_row(&mut first, "unreachable", |row| {
        row.state = SessionState::Lost(LostReason::WorkerUnreachable)
    });
    rewrite_row(&mut first, "version", |row| {
        row.state = SessionState::Lost(LostReason::WorkerVersion)
    });
    rewrite_row(&mut first, "tokenless", |row| row.token = None);
    let mut again = World::over(&first);
    let events = adopt_all(&mut again);
    for (name, reason) in [
        ("gone", LostReason::WorkerGone),
        ("unreachable", LostReason::WorkerUnreachable),
        ("version", LostReason::WorkerVersion),
    ] {
        assert_eq!(
            states_of(&events, name),
            vec![SessionState::Lost(reason)],
            "{name}"
        );
    }
    for name in corrupt {
        assert_eq!(
            states_of(&events, name),
            vec![SessionState::Lost(LostReason::RegistryCorrupt)],
            "{name}"
        );
    }
    assert!(again.connects.is_empty());
}

/// A running session whose stop meets a broken link (LC-5): `stop_grace` ends it `Lost(WorkerUnreachable)`, and the row
/// records that end with the worker's identity (steward ruling R-36).
fn stopped_with_a_broken_link() -> World {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.autopilot = Autopilot::Silent;
    w.running("s");
    let link = w.link_of("s");
    w.feed(Input::LinkClosed { link });
    let stop = w.engine.begin(Op::Stop { id: sid("s") }).unwrap();
    w.pump();
    w.advance(Duration::from_millis(100));
    assert!(matches!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(
            LostReason::WorkerUnreachable
        )))
    ));
    let row = Row::decode(&sid("s"), &w.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Lost(LostReason::WorkerUnreachable));
    assert_eq!(row.worker.map(|r| r.identity()), Some(w.identity_of("s")));
    w
}

/// Puts a scripted worker at the endpoint of `name` on this handle.
fn endpoint_here(w: &mut World, name: &str, payload: AdoptedPayload) {
    let (instance, token) = (w.instance_of(name), w.token_of(name));
    w.endpoints.insert(
        instance,
        Endpoint {
            token,
            protocol: HELLO_PROTOCOL,
            payload: Some(payload),
        },
    );
}

fn stops(w: &World) -> usize {
    w.sent
        .iter()
        .filter(|(_, m)| matches!(m, HostMsg::Stop))
        .count()
}

/// Core A2-1, AD-2, LC-5; steward ruling R-36 (integration A1): `Adopt(id)` is admitted after the stop path's
/// `Lost(WorkerUnreachable)`. A payload that runs is `Running`: there is no `Stopping` outcome, and no stop and no `Launch`
/// is sent. A host that still wants the payload ended calls `Stop`.
#[test]
fn a_retry_after_a_broken_link_stop_adopts_a_running_payload_as_running() {
    let mut w = stopped_with_a_broken_link();
    endpoint_here(&mut w, "s", running_payload());
    let (launches_before, stops_before) = (launches(&w), stops(&w));
    let record = match w.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(record.state, SessionState::Running);
    assert_eq!(stops(&w), stops_before, "R-36: no Stopping outcome");
    assert_eq!(launches(&w), launches_before);
    let row = Row::decode(&sid("s"), &w.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Running);
    assert!(
        w.engine.begin(Op::Stop { id: sid("s") }).is_ok(),
        "the host may stop the adopted payload"
    );
}

/// Core AD-1, AD-2, LC-5; steward ruling R-36 (integration A1): a retry that finds the payload ended is `Exited`, and the
/// row records it. The `Stop` that ended in `Lost` completed, so the cause follows the report alone (`Signal`), as for a new
/// handle that reads the same row.
#[test]
fn a_retry_after_a_broken_link_stop_finds_the_payload_ended() {
    let mut w = stopped_with_a_broken_link();
    endpoint_here(
        &mut w,
        "s",
        AdoptedPayload::Exited {
            code: None,
            signal: Some(9),
        },
    );
    let record = match w.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    let exited = SessionState::Exited(Exit {
        code: None,
        signal: Some(9),
        cause: ExitCause::Signal,
    });
    assert_eq!(record.state, exited);
    let row = Row::decode(&sid("s"), &w.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, exited);
}

/// Steward ruling R-36 with the follow-up `d18b6de` (integration A1): a retry after the stop path's `Lost` whose worker
/// reports `NotLaunched` is `Lost(StartInterrupted)`, with no `Launch`.
#[test]
fn a_retry_after_a_broken_link_stop_with_no_launched_payload_is_start_interrupted() {
    let mut w = stopped_with_a_broken_link();
    endpoint_here(&mut w, "s", AdoptedPayload::NotLaunched);
    let launches_before = launches(&w);
    let record = match w.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        record.state,
        SessionState::Lost(LostReason::StartInterrupted)
    );
    assert_eq!(launches(&w), launches_before);
}

/// Core AD-1, AD-2; steward ruling R-36 (integration A1, review P5-F24 withdrawn): a new handle recovers the row of a stop
/// that met a broken link. `AdoptAll` posts the recorded `Lost(WorkerUnreachable)` with no handshake, and `Adopt(id)` then
/// re-reads the worker: a running payload is `Running`, with no stop and no `Launch`.
#[test]
fn a_new_handle_retries_the_row_of_a_broken_link_stop() {
    let first = stopped_with_a_broken_link();
    let mut again = adopting(&first, "s", running_payload());
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    assert!(again.connects.is_empty());
    let record = match again.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(record.state, SessionState::Running);
    assert_eq!(again.connects, vec![first.instance_of("s")]);
    assert_eq!(stops(&again), 0);
    assert_eq!(launches(&again), 0);
}

/// Core AD-4, AD-2; steward ruling R-36: a start whose worker announces a protocol outside the set is `Lost(WorkerVersion)`,
/// and the row records it with the worker's identity. `Adopt(id)` is admitted; the start ended that worker, so the retry
/// finds it gone.
#[test]
fn a_start_lost_to_the_worker_version_is_written_and_may_be_retried() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s"));
    let start = w.engine.begin(Op::Start { id: sid("s") }).unwrap();
    w.pump();
    let (token, instance) = (w.token_of("s"), w.instance_of("s"));
    w.feed(Input::LinkHello {
        link: LinkId(90),
        hello: Hello {
            protocol: 9,
            instance: instance.clone(),
            proof: token_proof(&token, &instance, 7),
            host_epoch: 7,
        },
    });
    w.complete(start);
    let version = SessionState::Lost(LostReason::WorkerVersion);
    assert_eq!(w.engine.get(&sid("s")).unwrap().state, version);
    let row = Row::decode(&sid("s"), &w.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, version);
    assert!(
        row.worker.is_some(),
        "R-36: the row keeps the worker's identity"
    );
    let record = match w.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(record.state, SessionState::Lost(LostReason::WorkerGone));
}

/// Review P5-F26; steward ruling R-36: a stored `Lost(WorkerUnreachable)` row with no worker identity is posted as
/// recorded, and `Adopt(id)` is admitted. No worker can be found, so the retry is `Lost(StartInterrupted)` with no probe and
/// no connect, and the host does not stop.
#[test]
fn a_retry_of_a_lost_row_with_no_worker_identity_is_start_interrupted() {
    let mut first = stopped_with_a_broken_link();
    rewrite_row(&mut first, "s", |row| row.worker = None);
    let mut again = World::over(&first);
    let events = adopt_all(&mut again);
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    let record = match again.ok(Op::Adopt { id: sid("s") }) {
        OpOutput::Record(record) => record,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        record.state,
        SessionState::Lost(LostReason::StartInterrupted)
    );
    assert!(again.connects.is_empty());
    let row = Row::decode(&sid("s"), &again.rows[&row_key("s")]).unwrap();
    assert_eq!(row.state, SessionState::Lost(LostReason::StartInterrupted));
}

/// Review P5-F26; Core AD-2, AD-6: a stored `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` row with a worker identity
/// and an absent or invalid token cannot be authenticated. It is `Lost(RegistryCorrupt)` with no handshake, and `Adopt(id)`
/// is refused with `WrongState`. A valid row of the same state keeps the R-36 admission.
#[test]
fn a_lost_row_with_no_valid_token_is_registry_corrupt() {
    let mut first = stopped_with_a_broken_link();
    let template = Row::decode(&sid("s"), &first.rows[&row_key("s")]).unwrap();
    for (name, reason, token) in [
        ("absent", LostReason::WorkerUnreachable, None),
        (
            "short",
            LostReason::WorkerUnreachable,
            Some("abc".to_string()),
        ),
        ("version", LostReason::WorkerVersion, None),
    ] {
        let mut row = template.clone();
        row.id = sid(name);
        row.state = SessionState::Lost(reason);
        row.token = token;
        first
            .rows
            .insert(row_key(name), serde_json::to_vec(&row).unwrap());
    }
    let mut again = World::over(&first);
    let events = adopt_all(&mut again);
    for name in ["absent", "short", "version"] {
        assert_eq!(
            states_of(&events, name),
            vec![SessionState::Lost(LostReason::RegistryCorrupt)],
            "{name}"
        );
        assert_eq!(
            again
                .engine
                .begin(Op::Adopt { id: sid(name) })
                .unwrap_err()
                .code,
            ErrorCode::WrongState,
            "{name}"
        );
    }
    assert_eq!(
        states_of(&events, "s"),
        vec![SessionState::Lost(LostReason::WorkerUnreachable)]
    );
    again.engine.begin(Op::Adopt { id: sid("s") }).unwrap();
    assert!(
        again.connects.is_empty(),
        "AdoptAll connects to no Lost row"
    );
}
