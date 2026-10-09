//! The program controls of the testkit (`docs/core-testkit-controls.md`): `pty_output` and `pty_blocked` act at the program
//! edge of a session's payload only (Core A5-1), never in the worker machine or the host (plan 2.1).

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::program::ProgramControl;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_core_host::driver::HostWake;
use botster_route_codec::prelude::hex_decode;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("pty_output", pty_output);
    registry.register("pty_blocked", pty_blocked);
}

/// The program edge of the payload of `session` on `handle`, and the wake of the host that owns its worker.
fn program_of(
    harness: &TestkitHarness,
    handle: &str,
    session: &SessionId,
) -> Result<(ProgramControl, Option<Arc<dyn HostWake>>), ControlError> {
    let row = session_row(harness, handle, session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!(
            "the session {} has no worker process yet",
            session.0
        ))
    })?;
    harness
        .workers()
        .program_edge(worker.identity())
        .map_err(ControlError::Bad)
}

/// True while the payload of `session` on `handle` runs (`payload_alive`). After its exit, its PTY stays readable until the
/// reap, but the program writes nothing more.
fn program_alive(
    harness: &TestkitHarness,
    handle: &str,
    session: &SessionId,
) -> Result<bool, ControlError> {
    let row = session_row(harness, handle, session)?;
    Ok(row
        .worker
        .is_some_and(|worker| harness.workers().payload_alive(worker.identity())))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PtyOutput {
    session: SessionId,
    bytes_hex: String,
}

/// The payload of the session writes these bytes to its terminal (Core ST-1, OU-7). They are plain output: the worker reads
/// them as the program's own output, in reads whose sizes the scheduler chooses. The host is woken, as the real PTY's
/// readiness wakes a real host (TM-6). An empty, odd-length or non-hex `bytes_hex` is `Bad`, and so is a payload that has
/// exited.
fn pty_output(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: PtyOutput = parse(args)?;
    let bytes =
        hex_decode(&args.bytes_hex).map_err(|e| ControlError::Bad(format!("bytes_hex: {e}")))?;
    if bytes.is_empty() {
        return Err(ControlError::Bad("bytes_hex: no bytes".into()));
    }
    let (program, wake) = program_of(harness, handle, &args.session)?;
    if !program_alive(harness, handle, &args.session)? {
        return Err(ControlError::Bad(format!(
            "the payload of the session {} has exited: it writes nothing more",
            args.session.0
        )));
    }
    program.write(&bytes);
    if let Some(wake) = wake {
        wake.signal();
    }
    Ok(Value::Null)
}

fn on() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PtyBlocked {
    session: SessionId,
    #[serde(default = "on")]
    on: bool,
}

/// While on, the payload's terminal takes no write: every write returns `WouldBlock` (Core AM-2, DP-9, A5-3). `on: false`
/// lets writes proceed again, and also lifts a `pty_accept` limit; it wakes the host, as the PTY's write readiness wakes a
/// real host. `on: false` on a terminal that is not blocked is a no-op, so a transcript may release without a block.
fn pty_blocked(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: PtyBlocked = parse(args)?;
    let (program, wake) = program_of(harness, handle, &args.session)?;
    program.set_blocked(args.on);
    if let (false, Some(wake)) = (args.on, wake) {
        wake.signal();
    }
    Ok(Value::Null)
}

#[cfg(test)]
mod tests;
