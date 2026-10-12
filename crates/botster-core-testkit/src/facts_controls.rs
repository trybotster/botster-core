//! The input-facts control of the testkit (`docs/core-testkit-controls.md`): `input_records_cost`, the byte size of the
//! retained `input_records` list as the testkit's in-memory link carries it to the host (Core ST-7, 9B). A measurement under
//! the conditions of steward ruling R-14.3: it injects nothing, counts at the link boundary and never in worker code, and
//! adds no wait or synchronization point. It reads the last `ReadFacts` answer that the session's worker sent.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("input_records_cost", input_records_cost);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfSession {
    session: SessionId,
}

fn input_records_cost(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let OfSession { session } = parse(args)?;
    let row = session_row(harness, handle, &session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!(
            "the session {} has no worker process yet",
            session.0
        ))
    })?;
    let cost = harness
        .workers()
        .input_records_cost(worker.identity())
        .map_err(ControlError::Bad)?
        .ok_or_else(|| {
            ControlError::Bad(format!(
                "the worker of the session {} has answered no ReadFacts",
                session.0
            ))
        })?;
    Ok(json!({ "bytes": cost }))
}
