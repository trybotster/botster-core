//! The adoption controls of the testkit (`docs/core-testkit-controls.md`): a worker that withholds its control link
//! (`withhold_control_link`, Core A6-1), an impostor at a worker's endpoint (`impostor_worker`, Core A10-1, A11-1), the
//! signals that the impostor received (`signals_received`), a damaged registry row (`corrupt_registry_row`, Core A10-2), and a
//! worker that announces a protocol that Core cannot adopt (`announce_protocol`, Core A6-2, AD-4).
//! They act at the testkit's process and storage edges only, never in the worker machine or the host (plan 2.1): Core's own
//! handshake check, deadline and decoder decide each outcome.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::worker::{ImpostorField, ImpostorFrame, ImpostorPlan, InstanceKey};
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_core_edges::edges::GroupSignal;
use botster_core_host::session::{row_key, Row, RowWorker};
use botster_core_link::proof::token_from_hex;
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("withhold_control_link", withhold_control_link);
    registry.register("impostor_worker", impostor_worker);
    registry.register("signals_received", signals_received);
    registry.register("corrupt_registry_row", corrupt_registry_row);
    registry.register("announce_protocol", announce_protocol);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfSession {
    session: SessionId,
}

/// The endpoint of the session's worker: the data directory of the handle's host and the instance of the stored row.
fn endpoint_key(
    harness: &TestkitHarness,
    handle: &str,
    row: &Row,
) -> Result<InstanceKey, ControlError> {
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    Ok(InstanceKey {
        dir: dir.to_string(),
        instance: row.instance.clone(),
    })
}

/// The live worker of the session keeps running, and it takes no host's connection: the next adoption ends at Core's own
/// deadline as `Lost(WorkerUnreachable)` (Core A6-1, AD-2). The row must name a worker.
fn withhold_control_link(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    if row.worker.is_none() {
        return Err(ControlError::Bad(format!(
            "the row of the session {} names no worker",
            session.0
        )));
    }
    let key = endpoint_key(harness, handle, &row)?;
    harness.workers().withhold(key).map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

/// The field that `impostor_worker` gets wrong.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Field {
    Token,
    Instance,
}

/// A scripted frame of Core A11-1.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Script {
    CleanupDeleted,
    StateExited,
    Notification,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Impostor {
    session: SessionId,
    field: Field,
    #[serde(default)]
    script: Vec<Script>,
}

/// The plan of a stand-in at the endpoint of the session of `row`: the recorded token and the recorded worker identity, from
/// the stored row. Both tiers build it the same way (steward ruling R-49).
pub(crate) fn plan_of(
    row: &Row,
    field: ImpostorField,
    script: Vec<ImpostorFrame>,
) -> Result<ImpostorPlan, ControlError> {
    let session = &row.id;
    let worker = row.worker.map(RowWorker::identity).ok_or_else(|| {
        ControlError::Bad(format!(
            "the row of the session {} names no worker",
            session.0
        ))
    })?;
    let token = row
        .token
        .as_deref()
        .and_then(token_from_hex)
        .ok_or_else(|| {
            ControlError::Bad(format!("the row of the session {} has no token", session.0))
        })?;
    Ok(ImpostorPlan {
        session: session.clone(),
        field,
        script,
        token,
        worker,
    })
}

/// The arguments of `impostor_worker`: the session, the field that the impostor gets wrong, and its script.
pub(crate) fn impostor_args(
    args: &Value,
) -> Result<(SessionId, ImpostorField, Vec<ImpostorFrame>), ControlError> {
    let Impostor {
        session,
        field,
        script,
    } = parse(args)?;
    let field = match field {
        Field::Token => ImpostorField::Token,
        Field::Instance => ImpostorField::Instance,
    };
    let script = script
        .into_iter()
        .map(|s| match s {
            Script::CleanupDeleted => ImpostorFrame::CleanupDeleted,
            Script::StateExited => ImpostorFrame::StateExited,
            Script::Notification => ImpostorFrame::Notification,
        })
        .collect();
    Ok((session, field, script))
}

/// Installs the plan of a stand-in for `session` at the testkit's endpoint of its worker.
fn set_impostor(
    harness: &mut TestkitHarness,
    handle: &str,
    session: &SessionId,
    field: ImpostorField,
    script: Vec<ImpostorFrame>,
) -> Result<Value, ControlError> {
    let row = session_row(harness, handle, session)?;
    let plan = plan_of(&row, field, script)?;
    let key = endpoint_key(harness, handle, &row)?;
    harness
        .workers()
        .impostor(key, plan)
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
}

/// At the next adoption, the session's endpoint is answered by a process that is not its worker, with a wrong token or a
/// wrong `InstanceId` (Core A10-1). Its scripted frames follow its hello in the same write, so Core meets them after its check
/// rejected the handshake (Core A11-1; steward ruling R-42, contracts `main` `c2df50c`). The real worker keeps running and is
/// never connected.
fn impostor_worker(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let (session, field, script) = impostor_args(args)?;
    set_impostor(harness, handle, &session, field, script)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnnounceProtocol {
    session: SessionId,
    protocol: u8,
}

/// At the next adoption, the session's endpoint answers the host's hello with the recorded token and instance, and announces
/// `protocol` (Core A6-2, AD-4). The real worker keeps running and is never connected. The testkit has no worker of another
/// protocol, so the control takes only a protocol that Core must not adopt: one outside `{T, T-1}`, with `T` the worker
/// protocol of this build. Core's own version check decides the outcome.
fn announce_protocol(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let (session, protocol) = announced_args(args)?;
    set_impostor(
        harness,
        handle,
        &session,
        ImpostorField::Protocol(protocol),
        Vec::new(),
    )
}

/// The arguments of `announce_protocol`: the session and a protocol that Core must not adopt.
///
/// # Errors
/// `Unsupported` for a protocol in `{T, T-1}`: an adoptable worker of another protocol is a real worker of that protocol,
/// which no build has.
pub(crate) fn announced_args(args: &Value) -> Result<(SessionId, u8), ControlError> {
    let AnnounceProtocol { session, protocol } = parse(args)?;
    let current = botster_worker_core::WORKER_PROTOCOL;
    // `{T, T-1}`, with no `T-1` below 1 (A6-2).
    if (current.saturating_sub(1).max(1)..=current).contains(&protocol) {
        return Err(ControlError::Unsupported);
    }
    Ok((session, protocol))
}

/// `{signals}`: the signals that Core sent, in order, to the process that its AD-6 check refused, the impostor of
/// `impostor_worker` (Core AD-6, R-14.3). It reads a count at the edge and adds no synchronization point.
fn signals_received(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    // The row may be gone (`Remove`), so the impostor is found by its directory and session.
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    let signals = harness
        .workers()
        .impostor_signals(dir, &session)
        .ok_or_else(|| {
            ControlError::Bad(format!("no impostor answers for the session {}", session.0))
        })?;
    let names: Vec<&str> = signals.iter().copied().map(signal_name).collect();
    Ok(json!({ "signals": names }))
}

/// The name of a group signal in the result of `signals_received`.
pub(crate) fn signal_name(signal: GroupSignal) -> &'static str {
    match signal {
        GroupSignal::EndPayload => "end_payload",
        GroupSignal::Term => "term",
        GroupSignal::Kill => "kill",
    }
}

/// The storage edge damages the stored bytes of the session's row (Core A10-2). The bytes are the ones that Core's encoder
/// wrote, cut to their first half, and Core's own decoder must reject them.
fn corrupt_registry_row(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?
        .to_string();
    let key = row_key(&session);
    if !harness.directories().damage_row(&dir, &key) {
        return Err(ControlError::Bad(format!(
            "no row of the session {}",
            session.0
        )));
    }
    let bytes = harness.directories().row(&dir, &key).unwrap_or_default();
    if Row::decode(&session, &bytes).is_some() {
        return Err(ControlError::Bad(format!(
            "the damaged row of the session {} still decodes",
            session.0
        )));
    }
    Ok(Value::Null)
}

#[cfg(test)]
mod tests;
