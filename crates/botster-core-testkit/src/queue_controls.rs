//! The control-queue control of the testkit (`docs/core-testkit-controls.md`): `control_queue {session, on = true}` (Core
//! IN-6, steward ruling R-91). It acts at the testkit's link edge only, never in the host (plan 2.1): `on: true` makes the
//! edge report no room on the control link of the session's worker, so Core's own room check refuses a `cancel` `Full`;
//! `on: false` gives the room back and signals the host's wake, so the next pump finds room and Core posts
//! `SessionWritable` if a cancel was refused.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("control_queue", control_queue);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlQueue {
    session: SessionId,
    #[serde(default = "yes")]
    on: bool,
}

/// Core IN-6: the control queue of the session is full (`on: true`), or has room again (`on: false`).
fn control_queue(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let ControlQueue { session, on } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let (faults, wake) = harness
        .faults_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    let mut faults = lock(&faults);
    if on {
        faults.control_full.insert(row.instance);
    } else {
        faults.control_full.remove(&row.instance);
        wake.signal();
    }
    Ok(json!({}))
}

#[cfg(test)]
mod tests;
