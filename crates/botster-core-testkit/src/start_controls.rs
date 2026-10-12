//! The start controls of the testkit (`docs/core-testkit-controls.md`): the stored row of a session (`registry_row`), the
//! hold of one step of its start (`hold_start_at`, `release_start_at`), the hold of the worker's control link at the host's
//! accept (`hold_start`, `release_start`), whether a hold has stopped its step (`start_held`), and whether its payload runs
//! (`payload_alive`).
//! They read the storage edge and act at the program edge of the in-process worker or at the host's link edge, never in the
//! worker machine or the host (plan 2.1), so the order of AD-7 that a transcript observes is the host's own.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::{HostFaults, TestkitHarness};
use crate::worker::InstanceKey;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{InstanceId, SessionId, SessionState};
use botster_core_host::session::Row;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard, PoisonError};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The testkit has no panic that leaves its state half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("registry_row", registry_row);
    registry.register("hold_start_at", hold_start_at);
    registry.register("release_start_at", release_start_at);
    registry.register("hold_start", hold_start);
    registry.register("release_start", release_start);
    registry.register("start_held", start_held);
    registry.register("payload_alive", payload_alive);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfSession {
    session: SessionId,
}

/// `{state, identity_recorded, labels}`: the session's row as the storage edge holds it, decoded by the host's own decoder
/// (Core AD-7, AD-1). It reads what is stored and never writes.
fn registry_row(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    Ok(row_report(&row))
}

/// The answer of `registry_row` for a stored row, on both tiers.
pub(crate) fn row_report(row: &Row) -> Value {
    json!({
        "state": row.state,
        "identity_recorded": row.worker.is_some(),
        "labels": row.labels,
    })
}

/// The start of the session `instance` on the host of `handle`: its data directory and the instance. Core ID-1 scopes an
/// instance to its registry, so two directories of one run can have the same one.
fn start_key(
    harness: &TestkitHarness,
    handle: &str,
    instance: InstanceId,
) -> Result<InstanceKey, ControlError> {
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    Ok(InstanceKey {
        dir: dir.to_string(),
        instance,
    })
}

/// The step of AD-7 that `hold_start_at` holds.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Before {
    /// Step 3, the write of the worker identity.
    Identity,
    /// Step 4, the order to launch the payload.
    Payload,
    /// The step after the payload's launch: the `Running` row and the completion of `Start`.
    Running,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HoldStartAt {
    session: SessionId,
    before: Before,
}

/// Holds one step of the session's `Start` until `release_start_at` (Core AD-7, A5-2). With `before: payload`, the worker
/// takes the host's `Launch` and spawns no payload: the row and the worker identity are durable, and no payload runs. The
/// hold is set before the `Start`, so the session must still be `Created`.
fn hold_start_at(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let session = held_session(args)?;
    let row = session_row(harness, handle, &session)?;
    let key = start_key(harness, handle, row.instance.clone())?;
    not_begun(&session, &row)?;
    harness
        .workers()
        .hold_start(key)
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

/// The session of a `hold_start_at`, on both tiers: only the hold `before: payload` has a form.
pub(crate) fn held_session(args: &Value) -> Result<SessionId, ControlError> {
    let HoldStartAt { session, before } = parse(args)?;
    match before {
        Before::Payload => Ok(session),
        // No minimum id holds these steps yet.
        Before::Identity | Before::Running => Err(ControlError::Unsupported),
    }
}

/// The hold is set before the `Start`, so the session's stored row must still be `Created`.
pub(crate) fn not_begun(session: &SessionId, row: &Row) -> Result<(), ControlError> {
    if row.state == SessionState::Created {
        return Ok(());
    }
    Err(ControlError::Bad(format!(
        "the start of {} has begun: its row is {:?}",
        session.0, row.state
    )))
}

/// Ends the hold of `hold_start_at`: the step runs at the worker's next turn (Core AD-7, A5-2).
fn release_start_at(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let key = start_key(harness, handle, row.instance)?;
    harness
        .workers()
        .release_start(&key, row.worker.map(|w| w.identity()))
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

/// Core LC-4, A2-1 (steward ruling R-71, the form of the real tier's `hold_start`): holds the session's start at the host's
/// accept of the worker's control link. Core spawns the worker (AD-7 step 2); the worker connects and writes its hello; the
/// edge keeps that link from `accept_link`, so Core sees no link of the worker and the session stays `Starting`. The worker's
/// own startup clock runs during the hold. The hold is set before the `Start`.
fn hold_start(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    not_begun(&session, &row)?;
    let (faults, _) = host_faults(harness, handle)?;
    let mut faults = lock(&faults);
    if faults.start_holds.contains_key(&row.instance) {
        return Err(ControlError::Bad(format!(
            "the start of {} is already held",
            session.0
        )));
    }
    faults.start_holds.insert(row.instance, None);
    Ok(Value::Null)
}

/// Ends the hold of `hold_start`: the host's wake is signalled, and its next `accept_link` takes the held link, with every
/// byte that the worker wrote on it, in order. An armed hold is disarmed.
fn release_start(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let (faults, wake) = host_faults(harness, handle)?;
    if lock(&faults).start_holds.remove(&row.instance).is_none() {
        return Err(ControlError::Bad(format!(
            "no hold of the start of {}",
            session.0
        )));
    }
    wake.signal();
    Ok(Value::Null)
}

fn host_faults(harness: &TestkitHarness, handle: &str) -> Result<HostFaults, ControlError> {
    harness
        .faults_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))
}

/// `{held}`: whether the hold of `hold_start_at` or `hold_start` has stopped its step (steward ruling R-59): the scheduler has
/// the held step due, which is the worker's spawn of the payload, kept by the hold; or the edge holds the worker's control
/// link. False while the hold is only armed, and when no hold of the session is set. It reads the holds and never waits
/// (R-14.3).
fn start_held(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let (faults, _) = host_faults(harness, handle)?;
    let link_held = matches!(lock(&faults).start_holds.get(&row.instance), Some(Some(_)));
    let key = start_key(harness, handle, row.instance)?;
    Ok(json!({ "held": link_held || harness.workers().start_held(&key) }))
}

/// `{alive}`: whether the session's payload runs now, as the process edge sees it (Core EV-5(c), AD-7). The payload ends
/// when its process ends, before the worker or the host takes its exit. A session whose row names no worker has no payload.
fn payload_alive(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let alive = row
        .worker
        .is_some_and(|w| harness.workers().payload_alive(w.identity()));
    Ok(json!({ "alive": alive }))
}

#[cfg(test)]
mod tests;
