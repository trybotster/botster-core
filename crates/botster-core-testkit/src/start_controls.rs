//! The start controls of the testkit (`docs/core-testkit-controls.md`): the stored row of a session (`registry_row`), the
//! hold of one step of its start (`hold_start_at`, `release_start_at`), and whether its payload runs (`payload_alive`).
//! They read the storage edge and act at the program edge of the in-process worker, never in the worker machine or the host
//! (plan 2.1), so the order of AD-7 that a transcript observes is the host's own.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::worker::StartKey;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{InstanceId, SessionId, SessionState};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("registry_row", registry_row);
    registry.register("hold_start_at", hold_start_at);
    registry.register("release_start_at", release_start_at);
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
    Ok(json!({
        "state": row.state,
        "identity_recorded": row.worker.is_some(),
        "labels": row.labels,
    }))
}

/// The start of the session `instance` on the host of `handle`: its data directory and the instance. Core ID-1 scopes an
/// instance to its registry, so two directories of one run can have the same one.
fn start_key(
    harness: &TestkitHarness,
    handle: &str,
    instance: InstanceId,
) -> Result<StartKey, ControlError> {
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    Ok(StartKey {
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
    let HoldStartAt { session, before } = parse(args)?;
    match before {
        Before::Payload => {}
        // No minimum id holds these steps yet.
        Before::Identity | Before::Running => return Err(ControlError::Unsupported),
    }
    let row = session_row(harness, handle, &session)?;
    let key = start_key(harness, handle, row.instance.clone())?;
    if row.state != SessionState::Created {
        return Err(ControlError::Bad(format!(
            "the start of {} has begun: its row is {:?}",
            session.0, row.state
        )));
    }
    harness
        .workers()
        .hold_start(key)
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
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
