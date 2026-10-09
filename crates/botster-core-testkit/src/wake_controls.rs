//! The wake and quiet controls of the testkit (`docs/core-testkit-controls.md`): `edges_quiet`, the fence of `await_quiet`
//! (Core A5-2), and `no_spurious_wakes` (Core TH-2, A5-2).

use crate::controls::{parse, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("edges_quiet", edges_quiet);
    registry.register("no_spurious_wakes", no_spurious_wakes);
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoSpuriousWakes {}

/// From now on, the wake edge of every `Core` of the run fires only for a signal, so a `wait_wake` that gives `TimedOut` is
/// sound (Core TH-2 allows a spurious `Woken`, and A5-2 seeds it). The run has one scheduler, so the handle names no single
/// edge.
fn no_spurious_wakes(
    harness: &mut TestkitHarness,
    _handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoSpuriousWakes {} = parse(args)?;
    harness
        .workers()
        .scheduler()
        .set_overrides(|o| o.no_spurious_wakes = true);
    Ok(Value::Null)
}

#[cfg(test)]
mod tests;
