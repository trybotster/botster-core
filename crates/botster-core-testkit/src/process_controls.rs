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
    // The adoption handshake of the in-process worker (#176).
    (
        "impostor_worker",
        "the adoption endpoint of the in-process worker",
    ),
    (
        "signals_received",
        "the adoption endpoint of the in-process worker",
    ),
    (
        "withhold_control_link",
        "the adoption endpoint of the in-process worker",
    ),
    (
        "announce_protocol",
        "the adoption endpoint of the in-process worker",
    ),
    // Real processes: the slow tier.
    ("host_crash", "a real host process (slow tier)"),
    ("reuse_pid", "real processes and pids (slow tier)"),
    ("bystander_alive", "real processes and pids (slow tier)"),
    // Built when an id that uses them can pass: a2_1 and ad_2 need the route data plane (P4a) and the service lanes, ad_1
    // and ad_7 need adoption and the scheduler's start holds.
    ("hold_start", "a minimum id that it can flip"),
    ("release_start", "a minimum id that it can flip"),
    ("lose_worker", "a minimum id that it can flip"),
    ("payload_alive", "a minimum id that it can flip"),
];

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("break_control", break_control);
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

#[cfg(test)]
mod tests;
