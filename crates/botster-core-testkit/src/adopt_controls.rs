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
use botster_core_edges::edges::{GroupSignal, ProcessIdentity};
use botster_core_host::session::{row_key, Row, RowWorker};
use botster_core_link::proof::{token_from_hex, TOKEN_LEN};
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

/// The endpoint, the recorded token and the recorded worker identity of the session: what a stand-in at its endpoint needs.
fn stand_in(
    harness: &TestkitHarness,
    handle: &str,
    session: &SessionId,
) -> Result<(InstanceKey, [u8; TOKEN_LEN], ProcessIdentity), ControlError> {
    let row = session_row(harness, handle, session)?;
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
    Ok((endpoint_key(harness, handle, &row)?, token, worker))
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
    let Impostor {
        session,
        field,
        script,
    } = parse(args)?;
    let (key, token, worker) = stand_in(harness, handle, &session)?;
    let plan = ImpostorPlan {
        session: session.clone(),
        field: match field {
            Field::Token => ImpostorField::Token,
            Field::Instance => ImpostorField::Instance,
        },
        script: script
            .into_iter()
            .map(|s| match s {
                Script::CleanupDeleted => ImpostorFrame::CleanupDeleted,
                Script::StateExited => ImpostorFrame::StateExited,
                Script::Notification => ImpostorFrame::Notification,
            })
            .collect(),
        token,
        worker,
    };
    harness
        .workers()
        .impostor(key, plan)
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
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
    let AnnounceProtocol { session, protocol } = parse(args)?;
    let current = botster_worker_core::WORKER_PROTOCOL;
    // `{T, T-1}`, with no `T-1` below 1 (A6-2). An adoptable worker of another protocol is a real worker of that protocol,
    // which no build of the testkit has.
    if (current.saturating_sub(1).max(1)..=current).contains(&protocol) {
        return Err(ControlError::Unsupported);
    }
    let (key, token, worker) = stand_in(harness, handle, &session)?;
    let plan = ImpostorPlan {
        session,
        field: ImpostorField::Protocol(protocol),
        script: Vec::new(),
        token,
        worker,
    };
    harness
        .workers()
        .impostor(key, plan)
        .map_err(ControlError::Bad)?;
    Ok(Value::Null)
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
    let names: Vec<&str> = signals
        .iter()
        .map(|s| match s {
            GroupSignal::EndPayload => "end_payload",
            GroupSignal::Term => "term",
            GroupSignal::Kill => "kill",
        })
        .collect();
    Ok(json!({ "signals": names }))
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
