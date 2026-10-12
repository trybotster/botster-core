//! `CoreError { code, category, detail }` and every error code (9.3, A2-1, A2-10).

use crate::service::Bound;
use crate::types::StartFailReason;
use crate::util::lenient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The category of an error code (9.3).
///
/// Clause: Core 9.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum ErrorCategory {
    Caller = 1,
    State = 2,
    Fault = 3,
    Capacity = 4,
}

/// When a code arrives: at `begin` (sync), as `Completed{op, Err(code)}` (async), or either (ER-0).
///
/// Clause: Core 9.3 (ER-0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum Timing {
    Sync = 1,
    Async = 2,
    Either = 3,
}

/// The typed reason of an `Unsupported` that the contract names for `attach`.
///
/// Clause: Core OU-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum UnsupportedReason {
    TerminalFormat = 1,
    #[serde(other)]
    Other = 255,
}

/// Every error code of Core.
///
/// Clause: Core 9.3 (the table), Core A2-1, Core A2-10 (the removed and added codes).
///
/// Clause: Core 9.3, Core A2-1, Core A2-10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ErrorCode {
    UnknownSession = 1,
    UnknownRoute = 2,
    IdTooLong = 3,
    IdInUse = 4,
    WrongState = 5,
    SessionEnded = 6,
    MissingWorkerPath = 7,
    DataDirInUse = 8,
    StartFailed(StartFailReason) = 9,
    WorkerLinkFailed = 10,
    LaneFull = 11,
    /// Clause: Core A7-1, Core A15-1. `field` names the refused input (for example `route_limits.max_frame_bytes`, or `text`); it is absent when no clause names one.
    InvalidInput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    } = 12,
    PayloadTooLarge = 13,
    PendingLimit = 14,
    SessionLimit = 15,
    RouteLimit = 16,
    CaptureLimit = 17,
    ServiceLimit = 18,
    Cancelled = 19,
    CursorReadUnsupported = 20,
    UnknownCapture = 21,
    PageOutOfRange = 22,
    Unsupported {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        what: Option<UnsupportedReason>,
    } = 23,
    /// `field` names the refused limit.
    InvalidConfig {
        field: String,
    } = 24,
    /// Clause: Core AD-7 (`uncertain`).
    RegistryFailed {
        #[serde(default)]
        uncertain: bool,
    } = 25,
    SnapshotTooLarge = 26,
    BoundUnavailable {
        bound: Bound,
    } = 27,
    Internal = 28,
    UnknownService = 29,
}

impl ErrorCode {
    /// The category column of 9.3.
    ///
    /// Clause: Core 9.3, Core A2-1.
    pub fn category(&self) -> ErrorCategory {
        use ErrorCategory::*;
        match self {
            ErrorCode::UnknownSession
            | ErrorCode::UnknownRoute
            | ErrorCode::IdTooLong
            | ErrorCode::IdInUse
            | ErrorCode::MissingWorkerPath
            | ErrorCode::InvalidInput { .. }
            | ErrorCode::PayloadTooLarge
            | ErrorCode::CursorReadUnsupported
            | ErrorCode::UnknownCapture
            | ErrorCode::PageOutOfRange
            | ErrorCode::Unsupported { .. }
            | ErrorCode::InvalidConfig { .. }
            | ErrorCode::BoundUnavailable { .. }
            | ErrorCode::UnknownService => Caller,
            ErrorCode::WrongState
            | ErrorCode::SessionEnded
            | ErrorCode::DataDirInUse
            | ErrorCode::Cancelled => State,
            ErrorCode::StartFailed(_)
            | ErrorCode::WorkerLinkFailed
            | ErrorCode::RegistryFailed { .. }
            | ErrorCode::Internal => Fault,
            ErrorCode::LaneFull
            | ErrorCode::PendingLimit
            | ErrorCode::SessionLimit
            | ErrorCode::RouteLimit
            | ErrorCode::CaptureLimit
            | ErrorCode::ServiceLimit
            | ErrorCode::SnapshotTooLarge => Capacity,
        }
    }

    /// The Sync column of 9.3. A code of `open` counts as sync.
    ///
    /// Clause: Core 9.3, Core A2-1.
    pub fn timing(&self) -> Timing {
        match self {
            ErrorCode::WrongState | ErrorCode::Internal => Timing::Either,
            ErrorCode::SessionEnded
            | ErrorCode::StartFailed(_)
            | ErrorCode::WorkerLinkFailed
            | ErrorCode::Cancelled
            | ErrorCode::CursorReadUnsupported
            | ErrorCode::RegistryFailed { .. }
            | ErrorCode::SnapshotTooLarge => Timing::Async,
            _ => Timing::Sync,
        }
    }
}

/// An error of Core.
///
/// Clause: Core 3, Core 9.3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CoreError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub detail: String,
}

impl CoreError {
    /// An error with the category that 9.3 gives the code.
    ///
    /// Clause: Core 9.3.
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        CoreError {
            category: code.category(),
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.detail)
    }
}

impl std::error::Error for CoreError {}

lenient! {
    UnsupportedReason => ["terminal_format"],
}
