//! The process controls of the testkit (`docs/core-testkit-controls.md`): they act at the testkit's process edge only
//! (`WorkerSpawner`, the worker's edges, the process cell), never in the worker machine or the host (plan 2.1).
//!
//! A control that the testkit cannot build yet is registered with a handler that returns the typed `Unsupported`, so a
//! transcript that uses it ends `unsupported_control` and the reason is written here.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use serde::Deserialize;
use serde_json::Value;

/// The process controls that the testkit cannot build yet, each with what it waits for.
const UNSUPPORTED: &[(&str, &str)] = &[
    // Real processes: the slow tier.
    ("host_crash", "a real host process (slow tier)"),
    ("reuse_pid", "real processes and pids (slow tier)"),
    ("bystander_alive", "real processes and pids (slow tier)"),
    // A hold of AD-7 step 3 (the identity write) needs a deferral in the host driver: the testkit takes the host's
    // `SpawnWorker` and `WriteRow` actions at once. The start holds of AD-7 step 4 and `payload_alive` are in `start_controls`.
    (
        "hold_start",
        "a deferral of the host's spawn in the host driver",
    ),
    (
        "release_start",
        "a deferral of the host's spawn in the host driver",
    ),
];

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("break_control", break_control);
    registry.register("lose_worker", lose_worker);
    for &(name, _waits_for) in UNSUPPORTED {
        registry.register(name, unsupported);
    }
}

fn unsupported(
    _harness: &mut TestkitHarness,
    _handle: &str,
    _args: &Value,
) -> Result<Value, ControlError> {
    Err(ControlError::Unsupported)
}

fn on() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreakControl {
    session: SessionId,
    #[serde(default = "on")]
    on: bool,
}

/// The control link of the session breaks (Core LC-5, A2-1): the worker's end of it closes at the process edge, so the host
/// reads its end, and the worker process keeps running with its payload (DP-8). The operations in flight and the next ones
/// on the link fail `WorkerLinkFailed`; a stop still ends the session by the worker's process identity.
fn break_control(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: BreakControl = parse(args)?;
    if !args.on {
        // No transcript mends a broken link, and a closed in-memory link does not reopen.
        return Err(ControlError::Bad(
            "a broken control link is not mended (`on: false`)".into(),
        ));
    }
    let row = session_row(harness, handle, &args.session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!(
            "the session {} has no worker process yet",
            args.session.0
        ))
    })?;
    harness
        .workers()
        .break_link(worker.identity())
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

/// The `reason` of `lose_worker`, in the control vocabulary (`fake-core` `control.rs`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LoseReason {
    WorkerGone,
    WorkerUnreachable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoseWorker {
    session: SessionId,
    reason: Option<LoseReason>,
}

/// The worker process of the session ends from outside Core, as a kill does (Core AD-2, IN-7): its exit goes to the host that
/// spawned it, and its link, endpoint and payload end with it. Core's own exit handling and adoption check decide the
/// session's state. Only `worker_gone` (the default) is a loss of the process; `worker_unreachable` (a live worker that no
/// host reaches) is not built here.
fn lose_worker(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: LoseWorker = parse(args)?;
    if let Some(LoseReason::WorkerUnreachable) = args.reason {
        return Err(ControlError::Unsupported);
    }
    let row = session_row(harness, handle, &args.session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!(
            "the session {} has no worker process",
            args.session.0
        ))
    })?;
    harness
        .workers()
        .lose_worker(worker.identity())
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests;
