//! The result of one transcript (design 6.1). Only `passed` is a pass.

use serde::Serialize;
use serde_json::Value;

/// The result of one transcript over its seeds.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    /// A step did not match. `seed` is the seed that failed.
    Failed {
        step: usize,
        expected: Value,
        received: Value,
        seed: u64,
    },
    /// A runner execution limit expired. The run proved nothing.
    Inconclusive {
        step: usize,
        limit: String,
        seed: u64,
    },
    /// The subject does not advertise a feature that the transcript requires.
    NotApplicable {
        feature: String,
    },
    /// The subject does not implement a control that the transcript needs.
    UnsupportedControl {
        op: String,
    },
}

impl Outcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, Outcome::Passed)
    }
}
