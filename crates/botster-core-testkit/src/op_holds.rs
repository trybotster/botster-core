//! `scheduler_hold` and `scheduler_release` (`docs/core-testkit-controls.md`, Core A5-2; steward ruling R-88, group E): the
//! scheduler of one host holds the progress of a named operation until the release.
//!
//! The driver asks its scheduler `Scheduler::held` with the ops that a step of ready work serves. A held step is not runnable
//! work: it does not run, and it does not keep `PumpReport.more` true, so `pump_idle` ends while the op waits. The release
//! signals the host's wake, and a later pump runs the step.
//!
//! Op ids are numbered per host, so each host has its own [`OpHolds`]: the testkit's `SimEdges` scheduler answers from it,
//! and on the real tier the edge tap's scheduler does. The layer in front of the driver records each op that Core admits,
//! and whether a clause fixes its timing (R-20): such an op is never deferred, so a hold of it is refused.

use crate::controls::ControlRegistry;
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The ops of one host that a control holds, and what Core admitted.
#[derive(Debug, Default)]
pub struct OpHolds {
    held: BTreeSet<u64>,
    /// Every op that Core admitted on this host.
    begun: BTreeSet<u64>,
    /// The admitted ops whose timing a clause fixes (R-20): Core never defers them.
    fixed: BTreeSet<u64>,
}

/// The holds of one host, shared by its scheduler and its controls.
pub type SharedHolds = Arc<Mutex<OpHolds>>;

pub(crate) fn lock(holds: &Mutex<OpHolds>) -> MutexGuard<'_, OpHolds> {
    holds.lock().unwrap_or_else(PoisonError::into_inner)
}

impl OpHolds {
    /// Records an op that Core admitted, and whether a clause fixes its timing.
    pub fn begun(&mut self, op: OpId, fixed_timing: bool) {
        self.begun.insert(op.0);
        if fixed_timing {
            self.fixed.insert(op.0);
        }
    }

    /// `Scheduler::held`: whether a control holds one of the ops that a step serves. It draws nothing.
    pub fn holds_any(&self, ops: &[u64]) -> bool {
        ops.iter().any(|op| self.held.contains(op))
    }

    /// `scheduler_hold`: refused for an op whose timing a clause fixes; `Bad` for an op that Core did not admit, or one that is
    /// held already.
    fn hold(&mut self, op: OpId) -> Result<(), ControlError> {
        if !self.begun.contains(&op.0) {
            return Err(ControlError::Bad(format!(
                "scheduler_hold: Core admitted no op {} on this handle",
                op.0
            )));
        }
        if self.fixed.contains(&op.0) {
            return Err(ControlError::Refused(json!({"reason": "fixed_timing"})));
        }
        if !self.held.insert(op.0) {
            return Err(ControlError::Bad(format!(
                "scheduler_hold: op {} is held already",
                op.0
            )));
        }
        Ok(())
    }

    /// `scheduler_release`: `Bad` for an op that is not held.
    fn release(&mut self, op: OpId) -> Result<(), ControlError> {
        if !self.held.remove(&op.0) {
            return Err(ControlError::Bad(format!(
                "scheduler_release: op {} is not held",
                op.0
            )));
        }
        Ok(())
    }
}

/// The testkit tier's controls. The real tier serves the same two from the edge tap's holds (`real.rs`).
pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("scheduler_hold", scheduler_hold);
    registry.register("scheduler_release", scheduler_release);
}

fn holds_of(
    harness: &TestkitHarness,
    handle: &str,
) -> Result<
    (
        SharedHolds,
        std::sync::Arc<dyn botster_core_host::driver::HostWake>,
    ),
    ControlError,
> {
    harness
        .holds_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))
}

fn scheduler_hold(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    hold(&holds_of(harness, handle)?.0, args)
}

fn scheduler_release(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let (holds, wake) = holds_of(harness, handle)?;
    release(&holds, args, || wake.signal())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForOp {
    for_op: OpId,
}

/// `scheduler_hold {for_op}` on the holds of the handle's host. It sets the hold only; nothing runs.
pub(crate) fn hold(holds: &Mutex<OpHolds>, args: &Value) -> Result<Value, ControlError> {
    let ForOp { for_op } = crate::controls::parse(args)?;
    lock(holds).hold(for_op)?;
    Ok(Value::Null)
}

/// `scheduler_release {for_op}`: ends the hold, then signals the host's wake (`signal`), so that a later pump runs the step.
pub(crate) fn release(
    holds: &Mutex<OpHolds>,
    args: &Value,
    signal: impl FnOnce(),
) -> Result<Value, ControlError> {
    let ForOp { for_op } = crate::controls::parse(args)?;
    lock(holds).release(for_op)?;
    signal();
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad(result: Result<Value, ControlError>) -> bool {
        matches!(result, Err(ControlError::Bad(_)))
    }

    /// A hold names an admitted op; it holds only that op; a release ends it once and signals the wake once.
    #[test]
    fn a_hold_holds_its_op_until_the_release() {
        let holds = Mutex::new(OpHolds::default());
        lock(&holds).begun(OpId(3), false);
        assert!(bad(hold(&holds, &json!({"for_op": 4}))), "not admitted");
        assert_eq!(hold(&holds, &json!({"for_op": 3})), Ok(Value::Null));
        assert!(bad(hold(&holds, &json!({"for_op": 3}))), "held already");
        assert!(lock(&holds).holds_any(&[1, 3]));
        assert!(!lock(&holds).holds_any(&[1, 2]));
        assert!(
            !lock(&holds).holds_any(&[]),
            "a step with no op is never held"
        );
        let mut signals = 0;
        assert_eq!(
            release(&holds, &json!({"for_op": 3}), || signals += 1),
            Ok(Value::Null)
        );
        assert_eq!(signals, 1);
        assert!(!lock(&holds).holds_any(&[3]));
        assert!(bad(release(&holds, &json!({"for_op": 3}), || signals += 1)));
        assert_eq!(signals, 1, "a refused release signals nothing");
    }

    /// R-20: an op whose timing a clause fixes is never deferred, so a hold of it is refused with a typed value.
    #[test]
    fn a_hold_of_a_fixed_timing_op_is_refused() {
        let holds = Mutex::new(OpHolds::default());
        lock(&holds).begun(OpId(5), true);
        assert_eq!(
            hold(&holds, &json!({"for_op": 5})),
            Err(ControlError::Refused(json!({"reason": "fixed_timing"})))
        );
        assert!(!lock(&holds).holds_any(&[5]));
    }

    #[test]
    fn an_unknown_argument_is_bad() {
        let holds = Mutex::new(OpHolds::default());
        assert!(bad(hold(&holds, &json!({"op": 1}))));
        assert!(bad(hold(&holds, &json!({"for_op": 1, "more": true}))));
    }
}
