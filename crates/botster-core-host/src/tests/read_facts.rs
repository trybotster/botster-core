//! The host's part of `ReadFacts` (Core ST-7). The cursor ordinal of an instance (steward ruling R-101 Q6 and its
//! follow-up) is a durable counter of the data directory, reserved in the registry by each `Start` before anything runs, and
//! carried to the worker in its launch only. The end of a client input raises the keyed `Activity` edge again (EV-6).

use super::*;
use crate::session::ORDINAL_KEY;
use botster_core_edges::edges::StorageError;
use botster_core_link::msg::MAX_CURSOR_ORDINAL;

/// The cursor ordinals of the launches that `w` sent, in order.
fn launched_ordinals(w: &World) -> Vec<u64> {
    w.sent
        .iter()
        .filter_map(|(_, msg)| match msg {
            HostMsg::Launch(spec) => Some(spec.cursor_ordinal),
            _ => None,
        })
        .collect()
}

fn counter(w: &World) -> Option<String> {
    w.rows
        .get(ORDINAL_KEY)
        .map(|bytes| String::from_utf8(bytes.clone()).unwrap())
}

fn start_error(w: &mut World, name: &str) -> ErrorCode {
    let op = w.engine.begin(Op::Start { id: sid(name) }).unwrap();
    match w.complete(op) {
        OpResult::Err(e) => e.code,
        other => panic!("{other:?}"),
    }
}

/// A handle over a registry whose counter row holds `last`.
fn over_counter(last: u64) -> World {
    let mut earlier = World::default();
    earlier
        .rows
        .insert(ORDINAL_KEY.to_string(), last.to_string().into_bytes());
    World::over(&earlier)
}

/// R-101 Q6: each `Start` reserves the next ordinal, and the counter row holds the last one. A session that is removed and
/// created again is a new instance with a new ordinal: no two instances of the data directory share one.
#[test]
fn each_start_reserves_a_new_ordinal_also_after_a_remove_and_a_recreate() {
    let mut w = World::default();
    w.running("a");
    w.running("b");
    assert_eq!(counter(&w).as_deref(), Some("2"));
    w.ok(Op::Stop { id: sid("a") });
    w.ok(Op::Remove { id: sid("a") });
    w.running("a");
    assert_eq!(launched_ordinals(&w), [1, 2, 3]);
    assert_eq!(counter(&w).as_deref(), Some("3"));
}

/// R-101 Q6: a new handle over the registry continues above its counter, so a later Core process never reuses an ordinal.
#[test]
fn a_new_handle_mints_above_the_registry_counter() {
    let mut first = World::default();
    first.running("a");
    first.running("b");
    let mut second = World::over(&first);
    second.running("c");
    assert_eq!(launched_ordinals(&second), [3]);
}

/// The lead's condition (3): an uncertain ordinal write fails the `Start` `RegistryFailed{uncertain: true}` with the session
/// `Created`, and the next `Start` reserves the next ordinal, never the same one. So does an uncertain write that happened.
#[test]
fn an_uncertain_ordinal_write_is_never_reused() {
    let mut w = World::default();
    w.ok(create("s1"));
    w.fail_row = Some(StorageError::Uncertain { errno: 5 });
    assert_eq!(
        start_error(&mut w, "s1"),
        ErrorCode::RegistryFailed { uncertain: true }
    );
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Created
    );
    assert_eq!(counter(&w), None, "the write did not happen");
    w.fail_row_after_write = Some(StorageError::Uncertain { errno: 5 });
    assert_eq!(
        start_error(&mut w, "s1"),
        ErrorCode::RegistryFailed { uncertain: true }
    );
    assert_eq!(counter(&w).as_deref(), Some("2"), "the write happened");
    w.ok(Op::Start { id: sid("s1") });
    assert_eq!(launched_ordinals(&w), [3]);
}

/// R-101 follow-up: past the largest ordinal, `Start` fails `RegistryFailed{uncertain: false}` with nothing written and no
/// row of the session; the next `Start`, and a `Start` of a later handle, fail the same way (permanent for the data
/// directory). The counter is set near its limit through the storage edge.
#[test]
fn ordinal_exhaustion_fails_every_later_start_with_nothing_written() {
    let mut w = over_counter(MAX_CURSOR_ORDINAL - 1);
    w.running("last");
    assert_eq!(launched_ordinals(&w), [MAX_CURSOR_ORDINAL]);
    w.ok(create("s1"));
    let writes = w.row_writes.len();
    w.engine.poll_events(64);
    for _ in 0..2 {
        assert_eq!(
            start_error(&mut w, "s1"),
            ErrorCode::RegistryFailed { uncertain: false }
        );
    }
    assert_eq!(w.row_writes.len(), writes, "nothing is written");
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Created
    );
    let events = w.engine.poll_events(64);
    assert!(
        events.iter().all(|e| matches!(e, Event::Completed { .. })),
        "only the completions are posted: {events:?}"
    );
    let mut later = World::over(&w);
    later.ok(create("s2"));
    assert_eq!(
        start_error(&mut later, "s2"),
        ErrorCode::RegistryFailed { uncertain: false }
    );
    assert_eq!(launched_ordinals(&later), Vec::<u64>::new());
}

/// The counter row decodes as a decimal number; no row is 1, and a row that is not a number is refused.
#[test]
fn the_counter_row_is_a_decimal_number() {
    use crate::session::next_ordinal;
    assert_eq!(next_ordinal(None), Some(1));
    assert_eq!(next_ordinal(Some(b"41")), Some(42));
    assert_eq!(next_ordinal(Some(b"x")), None);
    assert_eq!(next_ordinal(Some(b"")), None);
}

/// A counter row at the largest `u64` is exhaustion, never a wrap to a small ordinal.
#[test]
fn a_counter_at_the_largest_number_is_exhaustion_not_a_wrap() {
    let mut w = over_counter(u64::MAX);
    w.ok(create("s1"));
    assert_eq!(
        start_error(&mut w, "s1"),
        ErrorCode::RegistryFailed { uncertain: false }
    );
    assert_eq!(launched_ordinals(&w), Vec::<u64>::new());
}

fn client_input_ended(w: &mut World, route: RouteId) {
    w.worker_says(
        "s1",
        WorkerMsg::Observed {
            observation: Observation::ClientInputEnded { route },
        },
    );
}

/// Core ST-7, EV-6: the end of a client input's transaction on a live route of the session raises the keyed
/// `Activity{Client(route)}` edge again, and changes no revision.
#[test]
fn the_end_of_a_client_input_raises_the_activity_edge_and_changes_no_revision() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.unix = 5000;
    w.feed(Input::Clock(5000));
    w.engine.poll_events(64);
    let before = w.engine.terminal_state(&sid("s1")).unwrap().input_rev;
    client_input_ended(&mut w, route);
    let events = w.engine.poll_events(64);
    assert!(
        matches!(
            &events[..],
            [Event::Activity {
                source: ActivitySource::Client(r),
                at: 5000,
                ..
            }] if *r == route
        ),
        "{events:?}"
    );
    assert_eq!(
        w.engine.terminal_state(&sid("s1")).unwrap().input_rev,
        before
    );
}

/// Core EV-5 (#320 RF-PC-3): `RouteClosed` retired the route's key, so the end of a client input that completes after
/// the close raises no `Activity` for it. A route that the session never had raises none either.
#[test]
fn the_end_of_a_client_input_after_its_route_closed_raises_no_activity() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Detached,
            route_tag: None,
        },
    );
    let events = w.engine.poll_events(64);
    assert!(
        matches!(&events[..], [Event::RouteClosed { route: r, .. }] if *r == route),
        "{events:?}"
    );
    client_input_ended(&mut w, route);
    client_input_ended(&mut w, RouteId(99));
    let events = w.engine.poll_events(64);
    assert!(events.is_empty(), "{events:?}");
}

/// Core EV-5, IN-10 (#320 RF-PC-3, the same key): a client input that a worker reports for a route that Core closed, or
/// that the session never had, moves the client revision and raises no `Activity` for the retired key. Core can close a
/// route without the worker (`DetachLocal` while the link is down), so the report can follow the close.
#[test]
fn a_client_input_of_a_closed_route_moves_the_revision_and_raises_no_activity() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let route = super::losses::attach(&mut w);
    w.worker_says(
        "s1",
        WorkerMsg::RouteClosed {
            route,
            reason: RouteCloseReason::Detached,
            route_tag: None,
        },
    );
    w.engine.poll_events(64);
    for (r, rev) in [(route, 7), (RouteId(99), 8)] {
        w.worker_says(
            "s1",
            WorkerMsg::Observed {
                observation: Observation::ClientInput {
                    route: r,
                    input_rev: InputRev(rev),
                },
            },
        );
        let events = w.engine.poll_events(64);
        assert!(events.is_empty(), "{r:?}: {events:?}");
        assert_eq!(
            w.engine
                .terminal_state(&sid("s1"))
                .unwrap()
                .input_rev
                .client,
            InputRev(rev)
        );
    }
}
