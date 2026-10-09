//! The wake and quiet controls of the testkit (`docs/core-testkit-controls.md`): `edges_quiet`, the fence of `await_quiet`
//! (Core A5-2), and `no_spurious_wakes` (Core TH-2, A5-2), which waits.

use crate::controls::{parse, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("edges_quiet", edges_quiet);
    // It needs a measured notion of a spurious wake (a `Woken` while the host has no work), checked at every wait and at the
    // end of the run, as agreed with P6; until then it is `Unsupported`, never a constant.
    registry.register("no_spurious_wakes", unsupported);
}

fn unsupported(
    _harness: &mut TestkitHarness,
    _handle: &str,
    _args: &Value,
) -> Result<Value, ControlError> {
    Err(ControlError::Unsupported)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgesQuiet {}

/// `{quiet}`: true when no edge toward the host of `handle` holds a report that it has not consumed (Core A5-2). It reads the
/// state of the edges and changes nothing: it never polls Core, so queued events stay, and it never moves the clock.
fn edges_quiet(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let EdgesQuiet {} = parse(args)?;
    let table = harness
        .processes_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    Ok(json!({ "quiet": harness.workers().edges_quiet(table) }))
}

#[cfg(test)]
mod tests;
