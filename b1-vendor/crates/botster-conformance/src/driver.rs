//! The suite side of the engine (design 6.1): a [`StepDriver`] executes the step kinds that the engine does not own.

use crate::matcher::Bindings;
use serde_json::Value;
use std::time::{Duration, Instant};

/// The wall-clock deadline of one step against a real implementation (design 6.1, execution limits).
///
/// A driver that can block (a socket read, a process wait) must give up when the deadline passes: `exec` returns `StepError::Inconclusive`, and `next` returns `Poll::TimedOut`.
/// The engine also checks the deadline after each call, so a late answer is `inconclusive`, never `failed`.
#[derive(Debug, Clone, Copy)]
pub struct Deadline(Option<Instant>);

impl Deadline {
    /// No deadline: the run against a fake.
    pub fn none() -> Self {
        Deadline(None)
    }

    /// A deadline `timeout` from now, or none when `timeout` is `None`.
    pub fn after(timeout: Option<Duration>) -> Self {
        Deadline(timeout.map(|t| Instant::now() + t))
    }

    pub fn expired(&self) -> bool {
        self.0.is_some_and(|at| Instant::now() >= at)
    }

    /// The time left, or `None` when there is no deadline. Zero once it has passed.
    pub fn remaining(&self) -> Option<Duration> {
        self.0
            .map(|at| at.saturating_duration_since(Instant::now()))
    }
}

/// What a driver gives when the engine asks for the next relevant item.
///
/// "Relevant" is the suite's definition (for HC: frames on the named connection and stream).
#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    /// The next relevant item.
    Item(Value),
    /// No item is ready, and more work (a fake step, a pump) may produce one. The engine asks again, up to its limit.
    Pending,
    /// No item is ready and none can arrive without a new stimulus.
    Idle,
    /// The connection or route ended. The value describes the reason.
    Closed(Value),
    /// The deadline passed while the driver waited. The step is `inconclusive`.
    TimedOut,
}

/// Why a step did not complete.
#[derive(Debug, Clone, PartialEq)]
pub enum StepError {
    /// The subject cannot do what the step needs. The result is `unsupported_control`, never a pass.
    Unsupported { op: String },
    /// The step failed. Both values go to the report.
    Failed { expected: Value, received: Value },
    /// A runner execution limit expired. The run proved nothing (design 6.1).
    Inconclusive { limit: String },
    /// The transcript breaks a rule that the driver checks at run time, on every tier: the step proves nothing about the subject,
    /// and the transcript must change. `rule` names the ruling (for example `R-85`); `detail` says where.
    TranscriptDefect { rule: String, detail: Value },
}

/// R-85 (R-61's input rule for computed bytes): the byte offset of the first `0d` in the expected bytes of an input-log assertion,
/// given as hex. The terminal's `ICRNL` turns a written `0d` into `0a` on the real tier, so no input-log assertion may compare one,
/// whatever its source: a literal, a `let` value or an `oracle_encode` binding. Hex that does not decode has no byte to check.
pub fn input_log_0d(hex: &str) -> Option<usize> {
    let bytes = hex.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    bytes
        .chunks(2)
        .position(|pair| pair.eq_ignore_ascii_case(b"0d"))
}

/// The R-85 transcript defect of an input-log assertion whose expected bytes hold a `0d` at `offset`.
pub fn input_log_0d_defect(what: &str, hex: &str, offset: usize) -> StepError {
    StepError::TranscriptDefect {
        rule: "R-85".into(),
        detail: serde_json::json!({
            "assertion": what,
            "expected_hex": hex,
            "0d_at_byte": offset,
            "why": "an input-log assertion compares no 0d byte, from any source (R-85, R-61)",
        }),
    }
}

/// One subject under test, built for one seed.
pub trait StepDriver {
    /// True when the subject is a fake. A `structural` transcript is only checked structurally on a fake (design 5.1, 7.1).
    /// A real implementation keeps the default.
    fn is_fake(&self) -> bool {
        false
    }

    /// True when the subject advertises the feature. Used for `requires.features`.
    fn has_feature(&self, name: &str) -> bool;

    /// True when the subject implements the test control. Used for `requires.controls`.
    fn has_control(&self, op: &str) -> bool;

    /// Prepares the run from the transcript's steps, once for each seed: after the `requires.controls` check and before `setup`.
    /// An error fails the transcript at step 0. The default does nothing.
    fn start(&mut self, _steps: &[Value]) -> Result<(), StepError> {
        Ok(())
    }

    /// Applies the transcript `setup` (limits and a named fixture).
    fn setup(&mut self, setup: &Value) -> Result<(), StepError>;

    /// Executes one suite step. A step may bind variables. `Ok` means the step completed.
    ///
    /// A send-class step arrives with its variables substituted and its frame checked (see `stimulus`). Raw stimuli and raw wire bytes arrive unchanged.
    fn exec(
        &mut self,
        step: &Value,
        bindings: &mut Bindings,
        deadline: &Deadline,
    ) -> Result<(), StepError>;

    /// The next relevant item. `scope` is the `on` field of the waiting step, or `null`.
    fn next(&mut self, scope: &Value, deadline: &Deadline) -> Poll;
}
