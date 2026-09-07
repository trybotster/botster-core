//! One-line structured failure record shared by every harness layer.
//!
//! Each layer keeps its own record type; this is the Rust one. It prints the
//! agreed field names on one line so a failure from Core, Hub, or TUI reads
//! the same in a log. There is no serialized schema.

use std::fmt;
use std::time::{Duration, Instant};

use botster_terminal_protocol::TerminalKind;

/// A bounded, one-line description of a failed harness step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepFailure {
    /// Harness layer that produced the record, for example `core`.
    pub layer: &'static str,
    /// Step name inside that layer, for example `attach`.
    pub step: &'static str,
    /// Session the step acted on, or empty.
    pub session_id: String,
    /// Subscription the step acted on, or empty.
    pub subscription_id: String,
    /// Attachment generation, or 0 when none applies.
    pub generation: u64,
    /// Stream epoch observed when the step failed.
    pub stream_epoch: u32,
    /// Deadline the step was given, in milliseconds.
    pub deadline_ms: u64,
    /// Time spent in the step, in milliseconds.
    pub elapsed_ms: u64,
    /// The last frame kinds seen on the route, oldest first, at most 16.
    pub last_kinds: Vec<TerminalKind>,
    /// Free-text cause. Short, no payload bytes.
    pub cause: String,
}

impl StepFailure {
    /// Build a record with no route context. Callers fill route fields in.
    #[must_use]
    pub fn new(layer: &'static str, step: &'static str, cause: impl Into<String>) -> Self {
        Self {
            layer,
            step,
            session_id: String::new(),
            subscription_id: String::new(),
            generation: 0,
            stream_epoch: 0,
            deadline_ms: 0,
            elapsed_ms: 0,
            last_kinds: Vec::new(),
            cause: cause.into(),
        }
    }

    /// Attach route identity.
    #[must_use]
    pub fn on_route(
        mut self,
        session_id: &str,
        subscription_id: &str,
        generation: u64,
        stream_epoch: u32,
    ) -> Self {
        self.session_id = session_id.to_string();
        self.subscription_id = subscription_id.to_string();
        self.generation = generation;
        self.stream_epoch = stream_epoch;
        self
    }

    /// Record the wait budget and how much of it elapsed.
    #[must_use]
    pub fn with_timing(mut self, deadline: Duration, started: Instant) -> Self {
        self.deadline_ms = deadline.as_millis().min(u128::from(u64::MAX)) as u64;
        self.elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        self
    }

    /// Record the last frame kinds seen on the route.
    #[must_use]
    pub fn with_last_kinds(mut self, kinds: impl IntoIterator<Item = TerminalKind>) -> Self {
        self.last_kinds = kinds.into_iter().collect();
        self
    }
}

impl fmt::Display for StepFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "layer={} step={} session_id={} subscription_id={} generation={} stream_epoch={} deadline_ms={} elapsed_ms={} last_kinds=[",
            self.layer,
            self.step,
            self.session_id,
            self.subscription_id,
            self.generation,
            self.stream_epoch,
            self.deadline_ms,
            self.elapsed_ms,
        )?;
        for (index, kind) in self.last_kinds.iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{kind:?}")?;
        }
        write!(f, "] cause={}", self.cause)
    }
}

impl std::error::Error for StepFailure {}
