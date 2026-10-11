//! The program controls of the testkit (`docs/core-testkit-controls.md`, `docs/fake-core-notes.md`): `pty_output`,
//! `pty_blocked`, `pty_queued`, `pty_input`, `pty_chunk`, `pty_accept` and `pty_fail_after` act at the program edge of a session's payload
//! only (Core A5-1), never in the worker machine or the host (plan 2.1).

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::program::ProgramControl;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_core_host::driver::HostWake;
use botster_route_codec::prelude::{hex_decode, HexBytes};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("pty_output", pty_output);
    registry.register("pty_blocked", pty_blocked);
    registry.register("pty_queued", pty_queued);
    registry.register("pty_input", pty_input);
    registry.register("pty_chunk", pty_chunk);
    registry.register("pty_accept", pty_accept);
    registry.register("pty_fail_after", pty_fail_after);
    registry.register("program_ready", program_ready);
}

/// The session's program has finished its terminal setup (`program_ready`, steward ruling R-60). The testkit's program edge
/// has no real terminal and no setup, so the answer is true at once, for a session that the handle's registry has.
fn program_ready(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Session = parse(args)?;
    session_row(harness, handle, &args.session)?;
    Ok(json!({ "ready": true }))
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
    #[serde(default)]
    kernel_bytes: Option<usize>,
}

/// While on, the payload's terminal takes no write: every write returns `WouldBlock` (Core AM-2, DP-9, A5-3). With
/// `kernel_bytes: n`, the terminal first takes up to n bytes into a queue that the program does not read, as a real PTY's kernel
/// buffer does (steward ruling R-74; `pty_queued` reports the queue). `on: false` lets writes proceed again, delivers the queue
/// to the program in order, and also lifts a `pty_accept` limit; it wakes the host, as the PTY's write readiness wakes a real
/// host. `on: false` on a terminal that is not blocked is a no-op, so a transcript may release without a block. `kernel_bytes`
/// with `on: false` is `Bad`.
fn pty_blocked(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: PtyBlocked = parse(args)?;
    if let (false, Some(_)) = (args.on, args.kernel_bytes) {
        return Err(ControlError::Bad(
            "kernel_bytes: only a block (on: true) takes it".into(),
        ));
    }
    let (program, wake) = program_of(harness, handle, &args.session)?;
    match args.kernel_bytes {
        Some(bytes) => program.block_with_kernel(bytes),
        None => program.set_blocked(args.on),
    }
    if let (false, Some(wake)) = (args.on, wake) {
        wake.signal();
    }
    Ok(Value::Null)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    session: SessionId,
}

/// The bytes of PTY input that wait in front of the program, which it has not read: the queue of `pty_blocked` with
/// `kernel_bytes` (steward ruling R-74). A read: it changes nothing.
fn pty_queued(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Session = parse(args)?;
    let (program, _) = program_of(harness, handle, &args.session)?;
    Ok(json!({ "bytes": program.queued() }))
}

/// Every byte of PTY input that the payload's program edge took, in order: the observer of the input controls.
fn pty_input(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Session = parse(args)?;
    let (program, _) = program_of(harness, handle, &args.session)?;
    Ok(json!({ "bytes": { "$bytes_hex": HexBytes(program.input_log()).to_hex() } }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bytes {
    session: SessionId,
    bytes: usize,
}

/// At most `bytes` bytes of input reach the PTY in each step (a pump of the host), so one host write reaches the PTY in
/// pieces across pumps (Core AM-2, IN-6). A cap of zero would let no input through at all, so it is `Bad`.
fn pty_chunk(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Bytes = parse(args)?;
    if args.bytes == 0 {
        return Err(ControlError::Bad(
            "bytes: a cap of 0 lets no input through".into(),
        ));
    }
    let (program, _) = program_of(harness, handle, &args.session)?;
    program.input_chunk(Some(args.bytes));
    Ok(Value::Null)
}

/// The program edge takes at most `bytes` more bytes of input, then none until `pty_blocked` with `on: false`; a write that
/// straddles the limit has its accepted prefix written (Core A5-2, IN-2: `Partial`).
fn pty_accept(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Bytes = parse(args)?;
    let (program, _) = program_of(harness, handle, &args.session)?;
    program.accept_at_most(args.bytes);
    Ok(Value::Null)
}

/// The program edge takes `bytes` more bytes of input, then the next write fails with an OS error (Core IN-2: `Failed`
/// with exact counts).
fn pty_fail_after(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Bytes = parse(args)?;
    let (program, _) = program_of(harness, handle, &args.session)?;
    program.fail_after(args.bytes);
    Ok(Value::Null)
}

#[cfg(test)]
mod tests;
