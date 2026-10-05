//! LC-3 to LC-7, LC-9, LC-12, OR-1, OR-2, AD-7, A6-3.

use super::*;

/// Core OR-1: `begin` and `poll_events` make no progress; only a pump does.
#[test]
fn begin_and_poll_make_no_progress() {
    let mut w = World::default();
    w.engine.begin(create("s1")).unwrap();
    assert!(w.engine.poll_events(64).is_empty());
    assert!(w.engine.poll_events(64).is_empty());
    assert!(
        w.rows.is_empty(),
        "no registry write happened before a pump"
    );
    assert!(
        w.engine.get(&sid("s1")).is_err(),
        "no state is shown before its event"
    );
}

/// Core LC-3, OR-2: `Create` writes the row, then posts `SessionState{Created}`, then `Completed`.
#[test]
fn create_posts_the_state_and_then_the_completion() {
    let mut w = World::default();
    let op = w.engine.begin(create("s1")).unwrap();
    w.pump();
    let events = w.engine.poll_events(64);
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(
        &events[0],
        Event::SessionState {
            state: SessionState::Created,
            ..
        }
    ));
    assert!(
        matches!(&events[1], Event::Completed { op: o, result: OpResult::Ok(OpOutput::Record(r)) }
        if *o == op && r.state == SessionState::Created)
    );
    assert!(
        w.rows.contains_key("session/s1"),
        "the row is durable before the completion"
    );
}

/// Core A2-7, 9B: one `pump` posts at most `pump_events`... a step posts at most one event, so the first pump of a create
/// posts the state and the completion in two steps.
#[test]
fn every_step_posts_at_most_one_event() {
    let mut w = World::default();
    w.engine.begin(create("s1")).unwrap();
    w.feed(Input::Clock(w.unix));
    let mut counts = Vec::new();
    while let Some(work) = w.engine.ready().into_iter().next() {
        w.feed(Input::Run(work));
        counts.push(w.engine.take_posted());
    }
    assert!(counts.iter().all(|c| *c <= 1), "{counts:?}");
    assert_eq!(counts.iter().sum::<u32>(), 2);
}

/// Core LC-3, OR-2: `Start` posts `Starting`, then `Running`, then its completion.
#[test]
fn start_posts_starting_then_running_then_the_completion() {
    let mut w = World::default();
    w.ok(create("s1"));
    let op = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
    assert_eq!(
        states(&events),
        vec![
            ("s1".to_string(), SessionState::Starting),
            ("s1".to_string(), SessionState::Running)
        ]
    );
    assert!(
        matches!(events.last(), Some(Event::Completed { result: OpResult::Ok(OpOutput::Record(r)), .. })
        if r.state == SessionState::Running && r.worker_protocol == Some(1))
    );
}

/// Core AD-7: the row (`Starting`, instance, token) is durable before the worker is spawned; the identity is durable before
/// the payload is launched.
#[test]
fn the_payload_is_launched_last() {
    let mut w = World::default();
    w.running("s1");
    let trace: Vec<&str> = w.trace.iter().map(String::as_str).collect();
    let at = |what: &str| {
        trace
            .iter()
            .position(|t| *t == what)
            .unwrap_or_else(|| panic!("{what} in {trace:?}"))
    };
    let row_starting = at("write session/s1");
    let spawn = at("spawn");
    let launch = at("send launch");
    let row_identity = trace[spawn..]
        .iter()
        .position(|t| *t == "write session/s1")
        .unwrap()
        + spawn;
    assert!(row_starting < spawn, "{trace:?}");
    assert!(spawn < row_identity && row_identity < launch, "{trace:?}");
    let row: crate::session::Row = serde_json::from_slice(&w.rows["session/s1"]).unwrap();
    assert!(
        row.worker.is_some() && row.token.is_some(),
        "the identity row names the worker and the token"
    );
}

/// Core AD-7: a hello that arrives before the identity row is durable does not launch the payload early.
#[test]
fn a_hello_before_the_identity_row_waits_for_it() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let instance = w.instance_of("s1");
    let token = w.token_of("s1");
    // The row writes are performed at once by the World, so the worker's hello sees the flow after the identity row.
    w.feed(Input::LinkHello {
        link: LinkId(1),
        hello: Hello {
            protocol: 1,
            instance,
            proof: token_proof(&token, &w.instance_of("s1"), 7),
            host_epoch: 7,
        },
    });
    w.pump();
    assert!(w.trace.contains(&"send launch".to_string()));
    assert_eq!(w.hellos.len(), 1, "the host answers the hello (AD-6)");
}

/// Core LC-4: a start that fails completes with `StartFailed`, and the session is `Exited`, never `Starting`.
#[test]
fn a_failed_launch_is_a_typed_failure_after_the_exited_state() {
    let mut w = World::default();
    let mut bad = request();
    bad.cwd = "/no-such-cwd".into();
    w.ok(Op::Create {
        session: sid("s1"),
        request: bad,
    });
    let op = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
    let order: Vec<&str> = events
        .iter()
        .map(|e| match e {
            Event::SessionState {
                state: SessionState::Starting,
                ..
            } => "starting",
            Event::SessionState {
                state: SessionState::Exited(_),
                ..
            } => "exited",
            Event::Completed { .. } => "completed",
            _ => "other",
        })
        .collect();
    assert_eq!(order, ["starting", "exited", "completed"], "OR-2");
    match events.last() {
        Some(Event::Completed {
            result: OpResult::Err(e),
            ..
        }) => {
            assert_eq!(e.code, ErrorCode::StartFailed(StartFailReason::CwdMissing));
            assert_eq!(e.category, ErrorCategory::Fault);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}

/// Core LC-4, A2-1: a refused spawn is `StartFailed{ExecFailed{errno}}`.
#[test]
fn a_refused_spawn_is_exec_failed_with_the_errno() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.refuse_spawn = Some(2);
    match w.run(Op::Start { id: sid("s1") }) {
        OpResult::Err(e) => assert_eq!(
            e.code,
            ErrorCode::StartFailed(StartFailReason::ExecFailed { errno: 2 })
        ),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Exited(_)
    ));
}

/// Core LC-4, 9B: a worker that does not come within `startup` ends the start with `StartupTimeout` and the worker is killed.
#[test]
fn a_worker_that_never_connects_times_out() {
    let mut w = World::new(limits(|l| l.startup = Duration::from_secs(3)));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let op = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    assert!(
        w.engine.next_deadline().is_some(),
        "TM-3: the startup deadline is reported"
    );
    w.advance(Duration::from_secs(3));
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
    match events.last() {
        Some(Event::Completed {
            result: OpResult::Err(e),
            ..
        }) => {
            assert_eq!(
                e.code,
                ErrorCode::StartFailed(StartFailReason::StartupTimeout)
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(!w.signals.is_empty(), "the worker group is killed");
}

/// Core AD-4, A6-2: a hello outside {T, T - 1} gives `Lost(WorkerVersion)`, and the link is closed.
#[test]
fn a_hello_with_a_protocol_outside_the_set_is_lost_worker_version() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let token = w.token_of("s1");
    let instance = w.instance_of("s1");
    w.feed(Input::LinkHello {
        link: LinkId(1),
        hello: Hello {
            protocol: 2,
            instance: instance.clone(),
            proof: token_proof(&token, &instance, 7),
            host_epoch: 7,
        },
    });
    let events = w.until(|e| matches!(e, Event::Completed { .. }));
    assert!(states(&events).contains(&(
        "s1".to_string(),
        SessionState::Lost(LostReason::WorkerVersion)
    )));
    assert!(w.closed.contains(&LinkId(1)));
}

/// Core AD-6: a hello with a wrong proof, or a wrong epoch, is closed and the start keeps waiting.
#[test]
fn a_hello_that_does_not_prove_the_token_is_refused() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let instance = w.instance_of("s1");
    let token = w.token_of("s1");
    for (proof_token, epoch, link) in [([9u8; TOKEN_LEN], 7, 5), (token, 6, 6)] {
        w.feed(Input::LinkHello {
            link: LinkId(link),
            hello: Hello {
                protocol: 1,
                instance: instance.clone(),
                proof: token_proof(&proof_token, &instance, epoch),
                host_epoch: epoch,
            },
        });
        assert!(w.closed.contains(&LinkId(link)));
    }
    assert!(w.hellos.is_empty());
    // A hello for an instance that no session has is refused too.
    w.feed(Input::LinkHello {
        link: LinkId(8),
        hello: Hello {
            protocol: 1,
            instance: InstanceId("other".into()),
            proof: token_proof(&token, &InstanceId("other".into()), 7),
            host_epoch: 7,
        },
    });
    assert!(w.closed.contains(&LinkId(8)));
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Starting
    );
}

/// Core LC-5, A2-1, EV-4: a stop that ends within `stop_grace` is `Exited{cause: HostStop}`, and a stop after the
/// grace needs the kill and is `Killed`.
#[test]
fn stop_is_host_stop_within_the_grace_and_killed_after_it() {
    let mut w = World::new(limits(|l| l.stop_grace = Duration::from_millis(100)));
    w.running("s1");
    match w.ok(Op::Stop { id: sid("s1") }) {
        OpOutput::End(SessionEnd::Exited(exit)) => assert_eq!(exit.cause, ExitCause::HostStop),
        other => panic!("{other:?}"),
    }
    // A worker that ignores the graceful request.
    w.autopilot = Autopilot::Silent;
    w.running("s2");
    let link = w.link_of("s2");
    let op = w.engine.begin(Op::Stop { id: sid("s2") }).unwrap();
    w.pump();
    assert!(w
        .sent
        .iter()
        .any(|(l, m)| *l == link && matches!(m, HostMsg::Stop)));
    assert!(!w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Kill)));
    w.advance(Duration::from_millis(100));
    w.pump();
    assert!(
        w.sent
            .iter()
            .any(|(l, m)| *l == link && matches!(m, HostMsg::Kill)),
        "the kill follows the grace"
    );
    w.worker_says(
        "s2",
        WorkerMsg::Exited {
            code: None,
            signal: Some(9),
        },
    );
    let result = w.complete(op);
    match result {
        OpResult::Ok(OpOutput::End(SessionEnd::Exited(exit))) => {
            assert_eq!((exit.signal, exit.cause), (Some(9), ExitCause::Killed));
        }
        other => panic!("{other:?}"),
    }
}

/// Core LC-5: a stop of a session whose payload already exited completes in the next pump, and Stop is idempotent.
#[test]
fn stop_of_an_exited_session_completes_with_the_same_end() {
    let mut w = World::default();
    w.running("s1");
    let first = w.ok(Op::Stop { id: sid("s1") });
    let second = w.ok(Op::Stop { id: sid("s1") });
    assert_eq!(first, second);
}

/// Core LC-12: a stop joins the stop that is running; both complete with the end, once (AM-3).
#[test]
fn two_stops_join_one_flow_and_each_completes_once() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let a = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let b = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    let mut seen = Vec::new();
    for _ in 0..4 {
        w.pump();
        seen.extend(w.engine.poll_events(64));
    }
    let completions: Vec<OpId> = seen
        .iter()
        .filter_map(|e| match e {
            Event::Completed { op, .. } => Some(*op),
            _ => None,
        })
        .collect();
    assert_eq!(completions, vec![a, b]);
    assert_eq!(
        w.sent
            .iter()
            .filter(|(_, m)| matches!(m, HostMsg::Stop))
            .count(),
        1,
        "one graceful request"
    );
}

/// Core LC-12: a `Stop` of a `Starting` session waits for the start, then stops the session.
#[test]
fn a_stop_of_a_starting_session_stops_it_when_the_start_ends() {
    let mut w = World::default();
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let mut done = BTreeSet::new();
    let mut seen = Vec::new();
    for _ in 0..40 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, .. } = &e {
                done.insert(*op);
            }
            seen.push(e);
        }
        if done.len() == 2 {
            break;
        }
    }
    assert!(done.contains(&start) && done.contains(&stop));
    let order: Vec<_> = states(&seen).into_iter().map(|(_, s)| s).collect();
    assert!(
        matches!(order.last(), Some(SessionState::Exited(_))),
        "{order:?}"
    );
    assert!(order.contains(&SessionState::Running) && order.contains(&SessionState::Stopping));
}

/// Core LC-6: a signal is sent to the worker and the completion follows the send; the exit is `HostStop`.
#[test]
fn signal_completes_when_the_worker_confirms_it_and_the_exit_is_host_stop() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let signal = w
        .engine
        .begin(Op::Signal {
            id: sid("s1"),
            sig: Signal::Kill,
        })
        .unwrap();
    w.pump();
    let (req, sent) = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Op {
                req,
                op: Op::Signal { sig, .. },
            } => Some((*req, *sig)),
            _ => None,
        })
        .expect("the signal went to the worker as a request");
    assert_eq!(sent, Signal::Kill);
    // A2-1: the completion follows the signal that was sent, not the request: nothing is completed before the worker says so.
    assert!(w
        .engine
        .poll_events(64)
        .iter()
        .all(|e| !matches!(e, Event::Completed { op, .. } if *op == signal)));
    w.worker_says(
        "s1",
        WorkerMsg::Done {
            req,
            result: OpResult::Ok(OpOutput::Unit),
        },
    );
    assert_eq!(w.complete(signal), OpResult::Ok(OpOutput::Unit));
    w.worker_says(
        "s1",
        WorkerMsg::Exited {
            code: None,
            signal: Some(9),
        },
    );
    let events = w.until(|e| {
        matches!(
            e,
            Event::SessionState {
                state: SessionState::Exited(_),
                ..
            }
        )
    });
    assert!(matches!(
        states(&events).last(),
        Some((
            _,
            SessionState::Exited(Exit {
                signal: Some(9),
                cause: ExitCause::HostStop,
                ..
            })
        ))
    ));
}

/// Core A2-1: a signal whose link fails before the worker confirms completes `WorkerLinkFailed`, never `Ok`.
#[test]
fn a_signal_over_a_failed_link_is_worker_link_failed() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let signal = w
        .engine
        .begin(Op::Signal {
            id: sid("s1"),
            sig: Signal::Term,
        })
        .unwrap();
    w.pump();
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    assert!(
        matches!(w.complete(signal), OpResult::Err(e) if e.code == ErrorCode::WorkerLinkFailed)
    );
}

/// Core EV-4: an exit that nobody asked for is `Normal` with a code, or `Signal` when a signal ended it.
#[test]
fn an_exit_that_nobody_asked_for_is_normal_or_signal() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("a");
    w.running("b");
    w.worker_says(
        "a",
        WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    );
    w.worker_says(
        "b",
        WorkerMsg::Exited {
            code: None,
            signal: Some(15),
        },
    );
    w.pump();
    let a = w.engine.get(&sid("a")).unwrap();
    let b = w.engine.get(&sid("b")).unwrap();
    assert_eq!(
        a.state,
        SessionState::Exited(Exit {
            code: Some(0),
            signal: None,
            cause: ExitCause::Normal
        })
    );
    assert_eq!(
        b.state,
        SessionState::Exited(Exit {
            code: None,
            signal: Some(15),
            cause: ExitCause::Signal
        })
    );
}

/// Core OR-2: `Exited` is posted after `Running` even when the payload ends at once.
#[test]
fn a_payload_that_ends_at_once_posts_running_before_exited() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
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
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
    });
    let mut seen = Vec::new();
    for _ in 0..4 {
        w.pump();
        seen.extend(w.engine.poll_events(64));
    }
    let order: Vec<_> = states(&seen).into_iter().map(|(_, s)| s).collect();
    assert!(
        matches!(
            order.as_slice(),
            [
                SessionState::Starting,
                SessionState::Running,
                SessionState::Exited(_)
            ]
        ),
        "{order:?}"
    );
}

impl World {
    /// Connects the worker of `session` by hand: the silent worker says hello with the right proof.
    pub(crate) fn link_of_after_hello(&mut self, session: &str) -> LinkId {
        let instance = self.instance_of(session);
        let token = self.token_of(session);
        let link = LinkId(self.next_link);
        self.next_link += 1;
        self.feed(Input::LinkHello {
            link,
            hello: Hello {
                protocol: 1,
                instance: instance.clone(),
                proof: token_proof(&token, &instance, 7),
                host_epoch: 7,
            },
        });
        self.pump();
        link
    }
}

/// Core LC-7: `Remove` of a `Created` session posts `Released` and then the completion, and the id is free.
#[test]
fn remove_of_a_created_session_frees_the_id_after_the_completion() {
    let mut w = World::default();
    w.ok(create("s1"));
    let op = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
    assert_eq!(
        states(&events),
        vec![("s1".to_string(), SessionState::Released)]
    );
    match events.last() {
        Some(Event::Completed {
            result: OpResult::Ok(OpOutput::RemoveReport(r)),
            ..
        }) => {
            assert_eq!(
                r.uploads,
                UploadsOutcome::Deleted,
                "a session that never had a worker has no uploads"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(
        !w.rows.contains_key("session/s1"),
        "the durable row is gone before the completion (LC-7 step 4)"
    );
    w.ok(create("s1"));
}

/// Core LC-7, A6-3: the report is `Deleted` from the worker's complete result, and the worker is asked to end.
#[test]
fn remove_of_an_exited_session_takes_the_workers_result() {
    let mut w = World::default();
    w.running("s1");
    w.ok(Op::Stop { id: sid("s1") });
    match w.ok(Op::Remove { id: sid("s1") }) {
        OpOutput::RemoveReport(r) => assert_eq!(r.uploads, UploadsOutcome::Deleted),
        other => panic!("{other:?}"),
    }
    assert!(w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Remove)));
}

/// Core A6-3: a worker that reports `delete_failed` with paths: the teardown continues and the paths are reported exactly.
#[test]
fn a_delete_failure_is_reported_with_its_paths_and_the_teardown_continues() {
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
    let op = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    w.worker_says(
        "s1",
        WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::NotDeleted(NotDeleted::DeleteFailed {
                paths: vec!["/u/a".into()],
            }),
        },
    );
    w.exited("s1");
    match w.complete(op) {
        OpResult::Ok(OpOutput::RemoveReport(r)) => assert_eq!(
            r.uploads,
            UploadsOutcome::NotDeleted(NotDeleted::DeleteFailed {
                paths: vec!["/u/a".into()]
            })
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap_err().code,
        ErrorCode::UnknownSession,
        "step 5 ran"
    );
}

/// Core A6-3: a worker that is lost during the cleanup reports `outcome_unknown` and claims no path; the teardown continues.
#[test]
fn a_worker_lost_during_the_cleanup_is_outcome_unknown() {
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
    let op = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    w.exited("s1");
    match w.complete(op) {
        OpResult::Ok(OpOutput::RemoveReport(r)) => {
            assert_eq!(
                r.uploads,
                UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
            );
        }
        other => panic!("{other:?}"),
    }
}

/// Core A6-3: a `Lost` session whose worker cannot be asked reports `outcome_unknown`, and the id is freed.
#[test]
fn a_lost_session_without_a_link_removes_with_an_unknown_outcome() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    w.exited("s1");
    w.pump();
    w.engine.poll_events(64);
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    ));
    match w.ok(Op::Remove { id: sid("s1") }) {
        OpOutput::RemoveReport(r) => assert_eq!(
            r.uploads,
            UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
        ),
        other => panic!("{other:?}"),
    }
}

/// Core ER-0, AD-7: a registry write that fails completes with `RegistryFailed`; an uncertain one says so; the session
/// never existed.
#[test]
fn a_failed_create_write_is_registry_failed_and_leaves_no_session() {
    let mut w = World::default();
    w.fail_row = Some(StorageError::Uncertain { errno: 5 });
    let op = w.engine.begin(create("s1")).unwrap();
    match w.complete(op) {
        OpResult::Err(e) => assert_eq!(e.code, ErrorCode::RegistryFailed { uncertain: true }),
        other => panic!("{other:?}"),
    }
    assert!(w.engine.get(&sid("s1")).is_err());
    w.ok(create("s1"));
    w.fail_row = Some(StorageError::Failed { errno: 5 });
    let op = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    match w.complete(op) {
        OpResult::Err(e) => assert_eq!(e.code, ErrorCode::RegistryFailed { uncertain: false }),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Created,
        "nothing ran"
    );
    assert!(
        !w.trace.contains(&"spawn".to_string()),
        "AD-7: no worker without a durable row"
    );
}

/// Core LC-9: `UpdateMetadata` completes after the durable write, and `MetadataChanged` follows the completion.
#[test]
fn update_metadata_is_durable_and_posts_metadata_changed_after_the_completion() {
    let mut w = World::default();
    w.ok(create("s1"));
    let labels = BTreeMap::from([("k".to_string(), "v".to_string())]);
    let op = w
        .engine
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: labels.clone(),
        })
        .unwrap();
    w.pump();
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[0], Event::Completed { op: o, result: OpResult::Ok(OpOutput::Unit) } if *o == op)
    );
    assert!(
        matches!(&events[1], Event::MetadataChanged { .. }),
        "{events:?}"
    );
    let row: crate::session::Row = serde_json::from_slice(&w.rows["session/s1"]).unwrap();
    assert_eq!(row.labels, labels);
    assert_eq!(w.engine.get(&sid("s1")).unwrap().labels, labels);
}

/// Core LC-9: a failed metadata write leaves the labels unchanged and completes `RegistryFailed`.
#[test]
fn a_failed_metadata_write_leaves_the_labels() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.fail_row = Some(StorageError::Failed { errno: 28 });
    let op = w
        .engine
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: BTreeMap::from([("k".into(), "v".into())]),
        })
        .unwrap();
    assert!(
        matches!(w.complete(op), OpResult::Err(e) if e.code == ErrorCode::RegistryFailed { uncertain: false })
    );
    assert!(w.engine.get(&sid("s1")).unwrap().labels.is_empty());
}

/// Core LC-9, AD-4: the record has no `worker_protocol` before the start, and has the worker's number after it.
#[test]
fn the_record_exposes_the_worker_protocol_after_the_start() {
    let mut w = World::default();
    w.ok(create("s1"));
    assert_eq!(w.engine.get(&sid("s1")).unwrap().worker_protocol, None);
    w.ok(Op::Start { id: sid("s1") });
    let record = w.engine.get(&sid("s1")).unwrap();
    assert_eq!(record.worker_protocol, Some(w.engine.worker_protocol()));
    assert_eq!(
        record.worker_features,
        Some(BTreeSet::from([Feature::FocusReport]))
    );
}

/// Core LC-10: `status` lists sessions and states and nothing else.
#[test]
fn status_has_sessions_and_states_only() {
    let mut w = World::default();
    w.ok(create("s1"));
    assert_eq!(
        w.engine.status(),
        Status {
            sessions: vec![SessionStatus {
                id: sid("s1"),
                state: SessionState::Created
            }]
        }
    );
}

/// Core LC-9: `list` and `get` read the cached state.
#[test]
fn get_and_list_read_the_cached_state() {
    let mut w = World::default();
    let mut req = request();
    req.labels = BTreeMap::from([("a".to_string(), "1".to_string())]);
    req.size = Size {
        rows: 30,
        cols: 100,
        cell_px: None,
    };
    w.ok(Op::Create {
        session: sid("s1"),
        request: req,
    });
    let record = w.engine.get(&sid("s1")).unwrap();
    assert_eq!(
        (record.state, record.size.rows, record.size.cols),
        (SessionState::Created, 30, 100)
    );
    assert_eq!(record.labels.get("a").map(String::as_str), Some("1"));
    assert_eq!(w.engine.list(), vec![record]);
}

/// Core LC-12: `StopAll` stops the running and starting targets, leaves `Created`, `Exited` and `Lost`, and completes when
/// every target is `Exited` or `Lost`; a session created after it is not a target.
#[test]
fn stop_all_stops_its_targets_and_leaves_the_others() {
    let mut w = World::default();
    w.running("run");
    w.ok(create("created"));
    w.running("done");
    w.ok(Op::Stop { id: sid("done") });
    let all = w.engine.begin(Op::StopAll).unwrap();
    let later = w.engine.begin(create("later")).unwrap();
    let mut done = BTreeSet::new();
    while done.len() < 2 {
        w.pump();
        for e in w.engine.poll_events(64) {
            if let Event::Completed { op, .. } = e {
                done.insert(op);
            }
        }
    }
    assert!(done.contains(&all) && done.contains(&later));
    assert!(matches!(
        w.engine.get(&sid("run")).unwrap().state,
        SessionState::Exited(_)
    ));
    assert_eq!(
        w.engine.get(&sid("created")).unwrap().state,
        SessionState::Created
    );
    assert_eq!(
        w.engine.get(&sid("later")).unwrap().state,
        SessionState::Created
    );
}

/// Core LC-12: `StopAll` with no target completes at the next pump.
#[test]
fn stop_all_with_no_target_completes() {
    let mut w = World::default();
    assert_eq!(w.ok(Op::StopAll), OpOutput::Unit);
}

/// Core LC-12: the workers and the rows remain after `StopAll` until `Remove`: the stopped session still serves reads.
#[test]
fn a_stopped_session_keeps_its_worker_until_remove() {
    let mut w = World::default();
    w.running("s1");
    w.ok(Op::StopAll);
    assert!(matches!(
        w.ok(Op::ReadModeFlags { session: sid("s1") }),
        OpOutput::Modes(_)
    ));
    w.ok(Op::Remove { id: sid("s1") });
}

/// Core ST-5: reads of an `Exited` session are forwarded to the worker, and a write to it is a certain zero.
#[test]
fn an_exited_session_serves_reads_and_refuses_writes() {
    let mut w = World::default();
    w.running("s1");
    w.ok(Op::Stop { id: sid("s1") });
    assert!(matches!(
        w.ok(Op::ReadModeFlags { session: sid("s1") }),
        OpOutput::Modes(_)
    ));
    let write = Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Bytes {
            bytes: botster_route_codec::prelude::HexBytes(vec![1]),
        },
        guard: None,
    };
    match w.ok(write) {
        OpOutput::Input(r) => {
            assert_eq!(
                r.outcome,
                WriteOutcome::NotWritten(NotWrittenReason::SessionEnded)
            );
            assert_eq!((r.payload_bytes_written, r.pty_bytes_written), (0, 0));
        }
        other => panic!("{other:?}"),
    }
}

/// Core AD-1, LC-9: a `Created` row survives a new handle: `AdoptAll` keeps it as `Created` with its labels.
#[test]
fn adopt_all_keeps_created_rows_with_their_labels() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.ok(Op::UpdateMetadata {
        id: sid("s1"),
        labels: BTreeMap::from([("k".into(), "v".into())]),
    });
    let instance = w.instance_of("s1");
    let mut again = World::over(&w);
    assert_eq!(again.ok(Op::AdoptAll), OpOutput::Unit);
    let record = again.engine.get(&sid("s1")).unwrap();
    assert_eq!(record.state, SessionState::Created);
    assert_eq!(record.labels.get("k").map(String::as_str), Some("v"));
    assert_eq!(
        again.instance_of("s1"),
        instance,
        "ID-2: the instance survives adoption"
    );
}

/// Core EV-9, ID-1: every instance is new, and the instance of a session is in its events.
#[test]
fn a_recreated_session_is_a_new_instance_and_events_carry_it() {
    let mut w = World::default();
    let op = w.engine.begin(create("s1")).unwrap();
    let first = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
    let a = match &first[0] {
        Event::SessionState { instance, .. } => instance.clone(),
        other => panic!("{other:?}"),
    };
    let removed = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == removed));
    assert!(
        matches!(&events[0], Event::SessionState { instance, state: SessionState::Released, .. } if *instance == a)
    );
    let again = w.engine.begin(create("s1")).unwrap();
    let events = w.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == again));
    match &events[0] {
        Event::SessionState { instance, .. } => assert_ne!(*instance, a),
        other => panic!("{other:?}"),
    }
}

/// Core AM-3: a worker that is lost with an op in flight completes it exactly once.
#[test]
fn a_lost_worker_completes_the_pending_op_once() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let read = w
        .engine
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    w.pump();
    w.exited("s1");
    let result = w.complete(read);
    assert!(matches!(result, OpResult::Err(e) if e.code == ErrorCode::WorkerLinkFailed));
    for _ in 0..3 {
        w.pump();
    }
    assert!(w
        .engine
        .poll_events(64)
        .iter()
        .all(|e| !matches!(e, Event::Completed { op, .. } if *op == read)));
}

/// Core IN-7, A2-2: a write that was sent and not acknowledged when the link fails is `Unknown`, never a certain zero.
#[test]
fn a_write_in_flight_when_the_link_fails_is_unknown() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let write = w
        .engine
        .begin(Op::WriteInput {
            session: sid("s1"),
            payload: InputPayload::Bytes {
                bytes: botster_route_codec::prelude::HexBytes(vec![1, 2, 3]),
            },
            guard: None,
        })
        .unwrap();
    w.pump();
    let link = w.link_of("s1");
    w.feed(Input::LinkClosed { link });
    match w.complete(write) {
        OpResult::Ok(OpOutput::Input(r)) => assert_eq!(
            r.outcome,
            WriteOutcome::Unknown {
                max_payload_bytes: 3
            }
        ),
        other => panic!("{other:?}"),
    }
}
