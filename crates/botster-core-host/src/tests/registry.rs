//! The registry across handles: what a new handle does with the rows that an earlier host left (Core AD-1, AD-2, ID-1,
//! LC-3, LC-7, LC-11, A10-2). The rows are bytes that Core's own encoder wrote; a damaged row starts from them (A10-2).
//!
//! A row that names a worker is recovered by the adoption handshake (AD-6), which is not built yet: these tests assert
//! nothing about the state of such a row beyond the one `SessionState` that every row posts (LC-11).

use super::*;
use crate::io::Work;
use botster_core_edges::edges::{GroupSignal, StorageError};

fn row_key(name: &str) -> String {
    format!("{}{name}", crate::session::ROW_PREFIX)
}

/// The ids of the session rows of a registry.
fn row_ids(rows: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    rows.keys()
        .filter_map(|key| key.strip_prefix(crate::session::ROW_PREFIX))
        .map(str::to_string)
        .collect()
}

/// Runs `AdoptAll` on `w` and returns its events, the completion last.
fn adopt_all(w: &mut World) -> Vec<Event> {
    let adopt = w.engine.begin(Op::AdoptAll).expect("AdoptAll is admitted");
    let events = w.until(|e| matches!(e, Event::Completed { op, .. } if *op == adopt));
    assert_eq!(
        events.last(),
        Some(&Event::Completed {
            op: adopt,
            result: OpResult::Ok(OpOutput::Unit)
        }),
        "A2-1: AdoptAll completes Ok(())"
    );
    events
}

fn remove_report(w: &mut World, name: &str) -> UploadsOutcome {
    match w.ok(Op::Remove { id: sid(name) }) {
        OpOutput::RemoveReport(report) => report.uploads,
        other => panic!("{other:?}"),
    }
}

/// Core AD-1, LC-11, AD-2, A10-2: `AdoptAll` recovers every row, and each row posts exactly one `SessionState`. A `Created`
/// row stays `Created`. A row that Core's decoder rejects is `Lost(RegistryCorrupt)`: a cut row, a row of a later version,
/// and a row filed under another id. The id of a rejected row stays in use until `Remove` (AD-2).
#[test]
fn adopt_all_posts_one_state_for_every_row_and_a_rejected_row_is_registry_corrupt() {
    let mut first = World::default();
    first.ok(create("kept"));
    first.running("ran");
    for name in ["cut", "later", "moved"] {
        first.ok(create(name));
    }
    let cut = first.rows[&row_key("cut")].clone();
    first
        .rows
        .insert(row_key("cut"), cut[..cut.len() / 2].to_vec());
    let mut later: serde_json::Value =
        serde_json::from_slice(&first.rows[&row_key("later")]).unwrap();
    later["version"] = serde_json::json!(crate::session::ROW_VERSION + 1);
    first
        .rows
        .insert(row_key("later"), serde_json::to_vec(&later).unwrap());
    let kept = first.rows[&row_key("kept")].clone();
    first.rows.insert(row_key("moved"), kept);

    let mut again = World::over(&first);
    let events = adopt_all(&mut again);
    let posted = states(&events);
    let mut ids: Vec<String> = posted.iter().map(|(id, _)| id.clone()).collect();
    ids.sort();
    assert_eq!(ids, row_ids(&first.rows), "LC-11: one SessionState per row");
    let state_of = |name: &str| posted.iter().find(|(id, _)| id == name).map(|(_, s)| *s);
    assert_eq!(state_of("kept"), Some(SessionState::Created));
    // AD-1: the adopted `Created` session is one that has not started yet, so `Start` is admitted and starts it.
    again.ok(Op::Start { id: sid("kept") });
    for name in ["cut", "later", "moved"] {
        assert_eq!(
            state_of(name),
            Some(SessionState::Lost(LostReason::RegistryCorrupt)),
            "{name}"
        );
        assert_eq!(
            again.engine.get(&sid(name)).unwrap().state,
            state_of(name).unwrap()
        );
        assert_eq!(
            again.engine.begin(create(name)).unwrap_err().code,
            ErrorCode::IdInUse,
            "AD-2: {name} stays in use until Remove"
        );
    }
}

/// Core AD-2, LC-7, A6-3: `Remove` of a `Lost(RegistryCorrupt)` session claims nothing about its uploads (no complete,
/// trusted result can come: `outcome_unknown`), deletes the row, and frees the id.
#[test]
fn a_corrupt_row_is_removed_with_an_unknown_outcome_and_frees_its_id() {
    let mut first = World::default();
    first.ok(create("cut"));
    let cut = first.rows[&row_key("cut")].clone();
    first
        .rows
        .insert(row_key("cut"), cut[..cut.len() / 2].to_vec());
    let mut again = World::over(&first);
    adopt_all(&mut again);
    assert_eq!(
        remove_report(&mut again, "cut"),
        UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
    );
    assert!(
        !again.rows.contains_key(&row_key("cut")),
        "LC-7 step 4: the row is deleted"
    );
    again.ok(create("cut"));
}

/// Core ID-1, LC-3, AD-2: a new handle refuses `Create` of an id that a durable row holds, before `AdoptAll` and after it,
/// and writes no row: the earlier row, with its worker's identity and token (AD-6), stays as it was.
#[test]
fn create_refuses_an_id_that_a_durable_row_holds() {
    let mut first = World::default();
    first.ok(create("kept"));
    first.running("ran");
    let mut again = World::over(&first);
    for name in row_ids(&first.rows) {
        assert_eq!(
            again.engine.begin(create(&name)).unwrap_err().code,
            ErrorCode::IdInUse,
            "before AdoptAll: {name}"
        );
    }
    adopt_all(&mut again);
    for name in row_ids(&first.rows) {
        assert_eq!(
            again.engine.begin(create(&name)).unwrap_err().code,
            ErrorCode::IdInUse,
            "after AdoptAll: {name}"
        );
    }
    assert_eq!(again.rows, first.rows, "no row was written");
}

/// Core LC-7, AD-2, AD-6, A6-3 (audit A9): `Remove` of an adopted session whose worker this handle did not spawn, and whose
/// worker still runs, ends. The host checks the worker's identity and kills the matching worker; when the worker is gone,
/// the teardown completes with an unknown upload outcome (no trusted result can come) and frees the id.
#[test]
fn remove_of_an_adopted_session_kills_its_running_worker_and_completes() {
    let mut first = World::default();
    first.running("s1");
    let worker = first.identity_of("s1");
    // The worker outlives its host (LC-12).
    let mut again = World::over(&first);
    adopt_all(&mut again);
    let remove = again.engine.begin(Op::Remove { id: sid("s1") }).unwrap();
    again.pump();
    assert_eq!(
        again.signals,
        vec![(worker, GroupSignal::Kill)],
        "the matching worker is ended"
    );
    let grace = again.engine.limits().stop_grace;
    again.advance(grace);
    match again.complete(remove) {
        OpResult::Ok(OpOutput::RemoveReport(report)) => assert_eq!(
            report.uploads,
            UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
        ),
        other => panic!("{other:?}"),
    }
    again.ok(create("s1"));
}

/// Core AD-6, AD-2, LC-7: when the recorded worker is gone, or another process has its pid, `Remove` of the adopted session
/// signals nothing ("a process that does not match is never signalled") and completes at once, with no grace to wait.
#[test]
fn remove_of_an_adopted_session_whose_worker_is_gone_signals_nothing() {
    for pid_reused in [false, true] {
        let mut first = World::default();
        first.running("s1");
        let worker = first.identity_of("s1");
        first.alive.remove(&worker);
        if pid_reused {
            first.alive.insert(ProcessIdentity {
                pid: worker.pid,
                start_time: worker.start_time + 1,
            });
        }
        let mut again = World::over(&first);
        adopt_all(&mut again);
        assert_eq!(
            remove_report(&mut again, "s1"),
            UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown),
            "pid reused: {pid_reused}"
        );
        assert!(
            again.signals.is_empty(),
            "pid reused: {pid_reused}: {:?}",
            again.signals
        );
        again.ok(create("s1"));
    }
}

/// Core AD-6, AD-2, LC-7 (audit A9): each identity answer settles the session whose worker was checked. Two adopted sessions
/// are removed together: the worker of `a` still runs and is killed, so `a` waits for its grace; the worker of `b` is gone,
/// so `b` completes at once and nothing signals it.
#[test]
fn each_identity_answer_settles_the_session_whose_worker_was_checked() {
    let mut first = World::default();
    first.running("a");
    first.running("b");
    let a = first.identity_of("a");
    let b = first.identity_of("b");
    first.alive.remove(&b);
    let mut again = World::over(&first);
    adopt_all(&mut again);
    let remove_a = again.engine.begin(Op::Remove { id: sid("a") }).unwrap();
    let remove_b = again.engine.begin(Op::Remove { id: sid("b") }).unwrap();
    let events = again.until(|e| matches!(e, Event::Completed { op, .. } if *op == remove_b));
    match events.last() {
        Some(Event::Completed {
            result: OpResult::Ok(OpOutput::RemoveReport(report)),
            ..
        }) => assert_eq!(
            report.uploads,
            UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
        ),
        other => panic!("{other:?}"),
    }
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, .. } if *op == remove_a)),
        "a waits for its grace: {events:?}"
    );
    assert_eq!(
        again.signals,
        vec![(a, GroupSignal::Kill)],
        "b is never signalled"
    );
    let grace = again.engine.limits().stop_grace;
    again.advance(grace);
    match again.complete(remove_a) {
        OpResult::Ok(OpOutput::RemoveReport(report)) => assert_eq!(
            report.uploads,
            UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)
        ),
        other => panic!("{other:?}"),
    }
}

/// Core LC-11, AD-1, ID-2: a row of a session that this handle holds already posts that session's state once more, with its
/// own instance; the session is not replaced.
#[test]
fn a_row_of_a_session_of_this_handle_posts_its_state_with_its_instance() {
    let mut w = World::default();
    let create = w.engine.begin(create("own")).unwrap();
    let made = w.until(|e| matches!(e, Event::Completed { op, .. } if *op == create));
    let instance = made
        .iter()
        .find_map(|e| match e {
            Event::SessionState { instance, .. } => Some(instance.clone()),
            _ => None,
        })
        .expect("Created was posted");
    let events = adopt_all(&mut w);
    let own: Vec<(&InstanceId, &SessionState)> = events
        .iter()
        .filter_map(|e| match e {
            Event::SessionState {
                id,
                instance,
                state,
            } if *id == sid("own") => Some((instance, state)),
            _ => None,
        })
        .collect();
    assert_eq!(own, vec![(&instance, &SessionState::Created)]);
}

/// Core AD-7, ID-1, LC-3 (review finding P5-F8): a `Create` whose row write is uncertain assumes neither outcome, so the id
/// stays in use and no retry writes over the row; `AdoptAll` reads the registry and settles it. When the write took effect
/// (the edge applied the bytes, then reported the error), the row is recovered with the instance of the attempted row; when
/// it did not, the id is free again.
#[test]
fn an_uncertain_create_keeps_its_id_until_the_registry_is_read() {
    for took_effect in [false, true] {
        let mut w = World::default();
        let uncertain = StorageError::Uncertain { errno: 5 };
        if took_effect {
            w.fail_row_after_write = Some(uncertain);
        } else {
            w.fail_row = Some(uncertain);
        }
        match w.run(create("s1")) {
            OpResult::Err(e) => assert_eq!(e.code, ErrorCode::RegistryFailed { uncertain: true }),
            other => panic!("{other:?}"),
        }
        let writes = w.row_writes.len();
        assert_eq!(
            w.engine.begin(create("s1")).unwrap_err().code,
            ErrorCode::IdInUse,
            "AD-7: the row may exist"
        );
        assert_eq!(w.row_writes.len(), writes, "no second write");
        let attempted = w
            .rows
            .get(&row_key("s1"))
            .map(|bytes| crate::session::Row::decode(&sid("s1"), bytes).expect("Core wrote it"));
        assert_eq!(attempted.is_some(), took_effect);
        let events = adopt_all(&mut w);
        let posted: Vec<(&InstanceId, &SessionState)> = events
            .iter()
            .filter_map(|e| match e {
                Event::SessionState {
                    instance, state, ..
                } => Some((instance, state)),
                _ => None,
            })
            .collect();
        match attempted {
            Some(row) => {
                assert_eq!(posted, vec![(&row.instance, &SessionState::Created)]);
                assert_eq!(
                    w.engine.begin(create("s1")).unwrap_err().code,
                    ErrorCode::IdInUse
                );
            }
            None => {
                assert!(posted.is_empty(), "{posted:?}");
                w.ok(create("s1"));
            }
        }
    }
}

/// Core LC-11, AD-1: `AdoptAll` that reads the row of a session that this handle is still creating (the row is written, its
/// `Created` is not posted yet) waits for that `Created`, then posts the row's state, and completes after it. The pump runs
/// only ready work, and prefers `AdoptAll` whenever it is ready (an order that A5-2 leaves open).
#[test]
fn adopt_all_posts_the_state_of_a_row_whose_create_is_still_running() {
    let mut w = World::default();
    w.engine.begin(create("own")).unwrap();
    let adopt = w.engine.begin(Op::AdoptAll).unwrap();
    w.feed(Input::Clock(w.unix));
    // The create writes its row first, so that AdoptAll reads it.
    assert!(w.engine.ready().contains(&Work::Session(sid("own"))));
    w.feed(Input::Run(Work::Session(sid("own"))));
    assert!(w.rows.contains_key(&row_key("own")), "the row is written");
    while let Some(work) = {
        let ready = w.engine.ready();
        ready
            .iter()
            .find(|x| **x == Work::Op(adopt))
            .or(ready.first())
            .cloned()
    } {
        w.feed(Input::Run(work));
    }
    let events = w.engine.poll_events(64);
    let done = events
        .iter()
        .position(|e| matches!(e, Event::Completed { op, .. } if *op == adopt))
        .expect("AdoptAll completed");
    let created = events[..done]
        .iter()
        .filter(|e| matches!(e, Event::SessionState { id, state: SessionState::Created, .. } if *id == sid("own")))
        .count();
    assert_eq!(
        created, 2,
        "the create's Created and AdoptAll's state of the row, before its completion: {events:?}"
    );
}
