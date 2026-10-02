//! The edges of the flows: a start that fails each way, a stop whose row fails, a remove whose row delete fails, and the
//! rows that `AdoptAll` takes or leaves (Core AD-7, LC-4, LC-7, LC-12, R-16, AD-1).

use super::*;
use crate::flow::{Flow, RemovePhase};
use crate::session::Admit;
use botster_core_edges::edges::GroupSignal;

/// Core LC-4, A2-1: a launch that the worker refuses ends the start `Exited`, with the exit recorded and the admission state
/// `Exited`; a stop that waited for the start does not run on the failed session.
#[test]
fn a_refused_launch_leaves_an_exited_session_and_no_stop_runs() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::LaunchFailed {
            reason: StartFailReason::ExecFailed { errno: 2 },
        },
    });
    let mut seen = Vec::new();
    for _ in 0..10 {
        w.pump();
        seen.extend(w.engine.poll_events(64));
    }
    assert!(seen
        .iter()
        .any(|e| matches!(e, Event::Completed { op, result: OpResult::Err(_) } if *op == start)));
    assert!(seen.iter().any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(OpOutput::End(SessionEnd::Exited(_))) } if *op == stop)));
    let s = &w.engine.sessions[&sid("s1")];
    assert_eq!(s.admit, Admit::Exited);
    assert!(s.exit.is_some(), "the exit is recorded");
    assert!(
        !w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "no stop is sent to a payload that never ran"
    );
}

/// Core AD-7: when the identity row of a start cannot be written, the payload is never launched: the worker is killed, its
/// link is closed, and the session is `Lost(StartInterrupted)` with `RegistryFailed`.
#[test]
fn a_failed_identity_row_kills_the_worker_and_closes_its_link() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    // Run the start until the worker is spawned; the next row write is the identity row.
    for _ in 0..40 {
        if w.trace.iter().any(|t| t == "spawn") {
            break;
        }
        w.feed(Input::Run(crate::io::Work::Session(sid("s1"))));
    }
    assert!(w.trace.iter().any(|t| t == "spawn"));
    w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
    let instance = w.instance_of("s1");
    let token = w.token_of("s1");
    // The worker says hello; the identity row write then fails.
    let link = LinkId(w.next_link);
    w.next_link += 1;
    w.feed(Input::LinkHello {
        link,
        hello: Hello {
            protocol: 1,
            instance: instance.clone(),
            proof: token_proof(&token, &instance, 7),
            host_epoch: 7,
        },
    });
    w.pump();
    let result = w.complete(start);
    assert!(
        matches!(&result, OpResult::Err(e) if matches!(e.code, ErrorCode::RegistryFailed { .. })),
        "{result:?}"
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::StartInterrupted)
    );
    let worker = w.identity_of("s1");
    assert!(
        w.signals.contains(&(worker, GroupSignal::Kill)),
        "the worker is killed"
    );
    assert!(w.closed.contains(&link), "its link is closed");
}

/// Core LC-4: the startup deadline kills the worker and closes its link; the start ends `StartupTimeout`.
#[test]
fn the_startup_deadline_kills_the_worker_and_closes_its_link() {
    let mut w = World::new(limits(|l| l.startup = Duration::from_secs(2)));
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.advance(Duration::from_secs(2));
    w.pump();
    assert!(matches!(w.complete(start), OpResult::Err(_)));
    let worker = w.identity_of("s1");
    assert!(w.signals.contains(&(worker, GroupSignal::Kill)));
    assert!(w.closed.contains(&link));
}

/// Core R-16, LC-12: a plain `Stop` and a `StopAll` on one session whose Stopping row fails: the `Stop` fails
/// `RegistryFailed`, and the `StopAll` still stops the session and completes when it ends.
#[test]
fn a_stop_and_a_stop_all_share_a_failed_stopping_row() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    let all = w.engine.begin(Op::StopAll).unwrap();
    w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
    w.pump();
    let events = w.engine.poll_events(64);
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Err(err) }
            if *op == stop && matches!(err.code, ErrorCode::RegistryFailed { .. }))
        ),
        "{events:?}"
    );
    assert!(
        w.sent.iter().any(|(_, m)| matches!(m, HostMsg::Stop)),
        "the StopAll stops it"
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

/// Core LC-7, A2-1: a `Remove` whose row delete fails leaves the session as it was, in the admission state of its shown
/// state, so that the remove can be tried again.
#[test]
fn a_failed_row_delete_restores_the_admission_state() {
    for (name, shown, admit) in [
        ("created", None, Admit::Created),
        ("exited", Some(true), Admit::Exited),
        ("lost", Some(false), Admit::Lost),
    ] {
        let mut w = World::default();
        match shown {
            None => {
                w.ok(create("s1"));
            }
            Some(exit) => {
                w.autopilot = Autopilot::Silent;
                w.running("s1");
                if exit {
                    w.worker_says(
                        "s1",
                        WorkerMsg::Exited {
                            code: Some(0),
                            signal: None,
                        },
                    );
                } else {
                    w.exited("s1");
                }
                w.pump();
                w.engine.poll_events(64);
            }
        }
        let remove = w.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
        for _ in 0..30 {
            let phase = |w: &World| match &w.engine.sessions[&sid("s1")].flow {
                Flow::Remove(f) => Some(f.phase),
                _ => None,
            };
            if phase(&w) == Some(RemovePhase::AwaitTeardown) {
                if w.engine.sessions[&sid("s1")].worker.link.is_some() {
                    w.worker_says(
                        "s1",
                        WorkerMsg::RemoveResult {
                            uploads: UploadsOutcome::Deleted,
                        },
                    );
                }
                if !w.engine.sessions[&sid("s1")].worker.gone {
                    w.exited("s1");
                }
            }
            if phase(&w) == Some(RemovePhase::DeleteRow) {
                break;
            }
            if !w.step() {
                break;
            }
        }
        w.fail_row = Some(botster_core_edges::edges::StorageError::Failed { errno: 5 });
        let result = w.complete(remove);
        assert!(
            matches!(&result, OpResult::Err(e) if matches!(e.code, ErrorCode::RegistryFailed { .. })),
            "{name}: {result:?}"
        );
        assert_eq!(w.engine.sessions[&sid("s1")].admit, admit, "{name}");
        assert!(
            w.engine.get(&sid("s1")).is_ok(),
            "{name}: the session stays"
        );
    }
}

/// Core AD-1, LC-9: `AdoptAll` takes the rows of the registry: a `Created` row is kept as it is, a row of a session that ran is
/// `Lost`, a row that is damaged or of another version is left, and a session that exists already is not replaced.
#[test]
fn adopt_all_takes_the_good_rows_and_leaves_the_others() {
    let mut first = World::default();
    first.autopilot = Autopilot::Silent;
    first.ok(create("a"));
    first.running("b");
    first.ok(create("kept"));
    let mut rows = first.rows.clone();
    let mut other_version: serde_json::Value = serde_json::from_slice(&rows["session/a"]).unwrap();
    other_version["version"] = serde_json::json!(99);
    other_version["id"] = serde_json::json!("v99");
    rows.insert(
        "session/v99".into(),
        serde_json::to_vec(&other_version).unwrap(),
    );
    rows.insert("session/bad".into(), b"{not json".to_vec());
    let mut again = World::default();
    again.rows = rows;
    again.ok(create("kept"));
    again.engine.poll_events(64);
    let kept_instance = again.instance_of("kept");
    let adopt = again.engine.begin(Op::AdoptAll).unwrap();
    for _ in 0..30 {
        again.pump();
        again.engine.poll_events(64);
    }
    let _ = adopt;
    assert_eq!(
        again.engine.get(&sid("a")).unwrap().state,
        SessionState::Created
    );
    assert_eq!(
        again.engine.get(&sid("b")).unwrap().state,
        SessionState::Lost(LostReason::Other)
    );
    assert!(
        again.engine.get(&sid("v99")).is_err(),
        "another version is left"
    );
    assert!(
        again.engine.get(&sid("bad")).is_err(),
        "a damaged row is left"
    );
    assert_eq!(
        again.instance_of("kept"),
        kept_instance,
        "an existing session is not replaced"
    );
    assert_eq!(again.engine.sessions[&sid("a")].admit, Admit::Created);
    assert_eq!(again.engine.sessions[&sid("b")].admit, Admit::Lost);
}
