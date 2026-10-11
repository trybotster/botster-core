//! The route-transport controls of the testkit (`docs/core-testkit-controls.md`): they act at the host's edge of the
//! hand-over (`link_send_descriptor`) and at the in-memory route streams, never in the worker machine or the host (plan 2.1).
//! `route_reports` reads the host's `link_recv` edge ([`crate::route_reports`]).

use crate::controls::{parse, session_row, ControlRegistry};
use crate::core::HandoffHold;
use crate::harness::{HostFaults, TestkitHarness};
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{RouteId, SessionId};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("fail_handoff", fail_handoff);
    registry.register("hold_handoff", hold_handoff);
    registry.register("release_handoff", release_handoff);
    registry.register("route_reports", route_reports);
    for &name in crate::route_client::ROUTE_CONTROLS {
        registry.register(name, route_only);
    }
}

/// A control of a route's stream runs at the route's client end (`TestkitRoute::control`): the runner gives it there when the
/// step names its `route`. The harness lists it, so that the runner knows that the testkit has it.
fn route_only(
    _harness: &mut TestkitHarness,
    _handle: &str,
    _args: &Value,
) -> Result<Value, ControlError> {
    Err(ControlError::Bad(
        "a control of a route's stream needs the step key `route`".into(),
    ))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The testkit has no panic that leaves its state half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The controls of this module take no argument.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfRoute {
    session: SessionId,
    route: RouteId,
}

/// Steward ruling R-66: `{activity_before_close, closed}`, what the host of `handle` consumed of the worker's reports of
/// `route` in `session`. It reads the edge's record and never waits (R-14.3); a report that waits for the driver's next call
/// signals the host's wake, so that the next pump consumes it.
fn route_reports(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfRoute { session, route } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let (reports, wake) = harness
        .reports_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    let seen = lock(&reports).answer(&row.instance, route, || wake.signal());
    Ok(json!({ "activity_before_close": seen.activity_before_close, "closed": seen.closed }))
}

/// The faults of the host of `handle`.
fn faults(harness: &TestkitHarness, handle: &str) -> Result<HostFaults, ControlError> {
    harness
        .faults_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))
}

/// The next hand-over of a stream endpoint to a worker fails at the edge (Core DP-2): the link takes nothing, the driver
/// closes the endpoint, and the route closes `HandoffFailed`.
fn fail_handoff(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoArgs {} = parse(args)?;
    let (faults, _) = faults(harness, handle)?;
    lock(&faults).fail_handoff = true;
    Ok(json!({}))
}

/// The edge holds the next hand-over of a stream endpoint to a worker until `release_handoff` (Core OU-9, A5-1). The held
/// call returns `Blocked`, so `attach` returns and the worker has no transport for the route.
fn hold_handoff(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoArgs {} = parse(args)?;
    let (faults, _) = faults(harness, handle)?;
    let mut faults = lock(&faults);
    if faults.handoff_hold != HandoffHold::Off {
        return Err(ControlError::Bad(
            "a hand-over is already held (`hold_handoff` twice)".into(),
        ));
    }
    faults.handoff_hold = HandoffHold::Armed;
    Ok(json!({}))
}

/// The edge delivers the held hand-over (Core OU-9, A5-2). The host's wake is signalled, and the next pump writes the held
/// frame with its endpoint first, then the frames after it on that link, in order. The worker receives the endpoint in a
/// later pump.
fn release_handoff(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoArgs {} = parse(args)?;
    let (faults, wake) = faults(harness, handle)?;
    lock(&faults).handoff_hold = HandoffHold::Off;
    wake.signal();
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_conformance::{CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef};
    use botster_core_contract::prelude::Wake;
    use std::time::Duration;

    fn opened() -> TestkitHarness {
        let mut harness = TestkitHarness::new(0);
        harness
            .open(&OpenSpec {
                handle: "h".into(),
                data_dir: DataDirRef("d".into()),
                worker: Some(WorkerRef {
                    build: WorkerBuild::Current,
                    file_name: None,
                }),
                limits: json!({}),
            })
            .expect("a Core");
        harness
    }

    fn run(harness: &mut TestkitHarness, op: &str, args: Value) -> Result<Value, ControlError> {
        harness.control("h", op, &args)
    }

    fn bad(result: Result<Value, ControlError>) -> String {
        match result {
            Err(ControlError::Bad(why)) => why,
            other => panic!("{other:?}"),
        }
    }

    /// The hand-over controls set the switches of the handle's edges; a second hold is refused, and the release clears the
    /// hold and signals the host's wake (Core OU-9, DP-2, A5-2).
    #[test]
    fn the_handoff_controls_set_the_edge_switches_and_the_release_wakes_the_host() {
        let mut harness = opened();
        let (faults, wake) = harness.faults_of("h").unwrap();
        run(&mut harness, "fail_handoff", json!({"op": "fail_handoff"})).unwrap();
        assert!(lock(&faults).fail_handoff);
        run(&mut harness, "hold_handoff", json!({})).unwrap();
        assert_eq!(lock(&faults).handoff_hold, HandoffHold::Armed);
        assert!(bad(run(&mut harness, "hold_handoff", json!({}))).contains("already held"));
        wake.drain();
        assert_eq!(wake.wait(Duration::ZERO), Wake::TimedOut);
        run(&mut harness, "release_handoff", json!({})).unwrap();
        assert_eq!(lock(&faults).handoff_hold, HandoffHold::Off);
        assert_eq!(
            wake.wait(Duration::ZERO),
            Wake::Woken,
            "the release signals the wake"
        );
        run(&mut harness, "hold_handoff", json!({})).unwrap();
        assert!(bad(run(&mut harness, "fail_handoff", json!({"x": 1}))).contains("arguments"));
        assert!(bad(harness.control("other", "fail_handoff", &json!({}))).contains("not open"));
    }

    /// Steward ruling R-66: `route_reports` answers from the host's record for the session's instance. A received report
    /// counts only after the driver's next call on its link. (The wake while a report waits is `RouteReports::answer`'s
    /// test: a wait on the testkit's wake also steps the in-process workers, so it cannot show which call signalled.)
    #[test]
    fn route_reports_answers_what_the_host_consumed_for_the_session_instance() {
        use botster_core_conformance::normalize_op;
        use botster_core_contract::prelude::*;
        use botster_core_host::LinkId;
        use botster_core_link::frame::{encode_frame, FrameType, DEFAULT_MAX_PAYLOAD};
        use botster_core_link::hello::Hello;
        use botster_core_link::msg::WorkerMsg;
        use botster_core_link::proof::{token_proof, TOKEN_LEN};
        let mut harness = TestkitHarness::new(0);
        let mut core = harness
            .open(&OpenSpec {
                handle: "h".into(),
                data_dir: DataDirRef("d".into()),
                worker: Some(WorkerRef {
                    build: WorkerBuild::Current,
                    file_name: None,
                }),
                limits: json!({}),
            })
            .expect("a Core");
        let args = json!({"session": "s1", "route": 2});
        assert!(bad(run(&mut harness, "route_reports", args.clone())).contains("no row"));
        let mut create =
            json!({"Create": {"session": "s1", "request": {"program": [{"hold": {}}]}}});
        normalize_op(&mut create, &harness.probe_binary());
        core.begin(serde_json::from_value(create).unwrap()).unwrap();
        let (reports, _) = harness.reports_of("h").unwrap();
        // The pumps read the worker's frames; the driver's next call consumes what the last one received.
        let at = std::time::Instant::now();
        for _ in 0..50 {
            core.pump(Now {
                monotonic: at,
                unix: 1_000_000,
            });
        }
        let instance = session_row(&harness, "h", &SessionId("s1".into()))
            .unwrap()
            .instance;
        let answer =
            |harness: &mut TestkitHarness| run(harness, "route_reports", args.clone()).unwrap();
        let none = json!({"activity_before_close": false, "closed": false});
        assert_eq!(answer(&mut harness), none);
        // A link of another instance, then one of the session's instance, each with r2's close.
        let frame = |kind: FrameType, payload: &[u8]| {
            let mut out = Vec::new();
            encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut out).unwrap();
            out
        };
        let link_bytes = |instance: &InstanceId| {
            let mut hello = Vec::new();
            Hello {
                protocol: 1,
                proof: token_proof(&[3u8; TOKEN_LEN], instance, 1),
                instance: instance.clone(),
                host_epoch: 1,
            }
            .encode(&mut hello)
            .unwrap();
            let mut close = Vec::new();
            WorkerMsg::RouteClosed {
                route: RouteId(2),
                reason: RouteCloseReason::Detached,
                route_tag: None,
            }
            .encode(&mut close);
            [
                frame(FrameType::HELLO, &hello),
                frame(FrameType::WORKER_MSG, &close),
            ]
            .concat()
        };
        let (other, own) = (LinkId(901), LinkId(902));
        lock(&reports).received(other, &link_bytes(&InstanceId("9-9".into())));
        lock(&reports).call(other);
        assert_eq!(answer(&mut harness), none, "another instance");
        lock(&reports).received(own, &link_bytes(&instance));
        assert_eq!(answer(&mut harness), none, "received, not consumed");
        lock(&reports).call(own);
        assert_eq!(
            answer(&mut harness),
            json!({"activity_before_close": false, "closed": true})
        );
        assert!(
            bad(run(&mut harness, "route_reports", json!({"session": "s1"}))).contains("arguments")
        );
        assert!(bad(harness.control("other", "route_reports", &args)).contains("not open"));
    }

    /// The harness lists the controls of a route's stream, and refuses one whose step names no route.
    #[test]
    fn a_route_stream_control_needs_its_route() {
        let mut harness = opened();
        for &op in crate::route_client::ROUTE_CONTROLS {
            assert!(harness.has_control(op), "{op}");
            assert!(
                bad(run(&mut harness, op, json!({}))).contains("`route`"),
                "{op}"
            );
        }
    }
}
