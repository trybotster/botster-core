//! Readiness-gated write state machine.

use botster_core::{ModeFlags, PromptMarkPayload};
use serde::{Deserialize, Serialize};

/// Explicit delivery states for guarded daemon writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardedWriteDeliveryState {
    /// Daemon accepted the typed request.
    Accepted,
    /// Write was queued or deferred because evidence is absent or retryable.
    Deferred,
    /// Write was rejected because the target or evidence is unsafe.
    Rejected,
    /// Bytes were injected into the existing PTY input path.
    Written,
    /// Downstream delivery proof exists.
    Delivered,
    /// Downstream acknowledgement proof exists.
    Acknowledged,
}

/// Daemon decision for a guarded write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedWriteDecision {
    /// Readiness evidence allowed injection.
    Write,
    /// Evidence is absent or not yet sufficient.
    Defer {
        /// Human-readable reason for deferral.
        reason: String,
    },
    /// Evidence proves the write is unsafe or target is invalid.
    Reject {
        /// Human-readable reason for rejection.
        reason: String,
    },
}

/// Safe-write signal known by core.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeWriteIndicator {
    /// No discrete safe-write source was available.
    #[default]
    Absent,
    /// A core-owned signal explicitly permits writing.
    Safe,
    /// A core-owned signal explicitly blocks writing.
    Unsafe,
}

/// Prompt-related readiness evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptEvidence {
    /// Last prompt mark observed by the session process.
    pub last_prompt: Option<PromptMarkPayload>,
    /// Whether the host has reason to believe the session is waiting.
    pub waiting_for_answer: Option<bool>,
}

/// Snapshot or screen-state evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEvidence {
    /// Whether any terminal snapshot has been observed.
    pub snapshot_available: bool,
    /// Whether a plain screen read has been observed.
    pub screen_available: bool,
}

/// Composite daemon-owned readiness evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessEvidence {
    /// Terminal mode flags reported by the session process.
    pub mode_flags: Option<ModeFlags>,
    /// Prompt semantics reported by the session process.
    pub prompt: PromptEvidence,
    /// Snapshot/screen availability known to core.
    pub snapshot: SnapshotEvidence,
    /// Explicit safe-write signal, if one exists.
    pub safe_write: SafeWriteIndicator,
}

impl ReadinessEvidence {
    /// Evidence that a session is ready for a plain PTY write.
    #[must_use]
    pub fn ready(mode_flags: ModeFlags) -> Self {
        Self {
            mode_flags: Some(mode_flags),
            prompt: PromptEvidence {
                last_prompt: None,
                waiting_for_answer: Some(true),
            },
            snapshot: SnapshotEvidence {
                snapshot_available: true,
                screen_available: true,
            },
            safe_write: SafeWriteIndicator::Safe,
        }
    }
}

/// Evaluate readiness evidence without guessing missing signals.
#[must_use]
pub fn decide_guarded_write(evidence: &ReadinessEvidence) -> GuardedWriteDecision {
    match evidence.safe_write {
        SafeWriteIndicator::Unsafe => {
            return GuardedWriteDecision::Reject {
                reason: "safe-write evidence is unsafe".to_string(),
            }
        }
        SafeWriteIndicator::Absent => {
            return GuardedWriteDecision::Defer {
                reason: "safe-write evidence is absent".to_string(),
            }
        }
        SafeWriteIndicator::Safe => {}
    }

    let Some(mode_flags) = &evidence.mode_flags else {
        return GuardedWriteDecision::Defer {
            reason: "mode flags are absent".to_string(),
        };
    };

    if !mode_flags.cursor_visible {
        return GuardedWriteDecision::Defer {
            reason: "cursor is not visible".to_string(),
        };
    }

    if !evidence.snapshot.snapshot_available && !evidence.snapshot.screen_available {
        return GuardedWriteDecision::Defer {
            reason: "terminal snapshot and screen evidence are absent".to_string(),
        };
    }

    if evidence.prompt.waiting_for_answer == Some(false) {
        return GuardedWriteDecision::Reject {
            reason: "prompt evidence says the session is not waiting".to_string(),
        };
    }

    GuardedWriteDecision::Write
}
