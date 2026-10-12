//! The operations of `begin` (A2-1, A3-3) and their completion values.

use crate::error::CoreError;
use crate::ids::{ClientId, InputCursor, OpId, RouteId, ServiceId, SessionId};
use crate::input::{Guard, InputPayload, InputResult};
use crate::read::{Capture, Cursor, Facts, Modes, Screen};
use crate::route::DetachReason;
use crate::service::{ServiceEnd, ServiceLimits, SpawnReport};
use crate::types::{
    ColorProfile, NotificationPolicy, SessionEnd, SessionRecord, Signal, Size, SizePolicy,
    SpawnRequest,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The result of `Resize`: the size applied, or the later resize that replaced it.
///
/// Clause: Core SZ-3, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum ResizeResult {
    Applied { actual: Size } = 1,
    Superseded { by: OpId } = 2,
}

/// Why uploaded files are still on disk after a `Remove`. A path list belongs to `DeleteFailed` only, so no path is ever claimed for
/// `OutcomeUnknown`.
///
/// Clause: Core A6-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum NotDeleted {
    /// The cleanup completed. `paths` are exactly the files that could not be deleted.
    DeleteFailed { paths: Vec<String> } = 1,
    /// The cleanup did not complete with a trusted result. Any number of files may remain.
    OutcomeUnknown = 2,
}

/// The outcome of the upload cleanup at `Remove`.
///
/// Clause: Core A6-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum UploadsOutcome {
    /// The cleanup completed and every uploaded file is gone, or there were none.
    Deleted = 1,
    NotDeleted(NotDeleted) = 2,
}

/// The `Ok` value of `Remove`.
///
/// Clause: Core A6-3, Core A6-4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct RemoveReport {
    pub uploads: UploadsOutcome,
}

/// One variant per row of the A2-1 table and of A3-3, with the table's argument names.
///
/// Clause: Core A2-1, Core A3-3, Core 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum Op {
    Create {
        session: SessionId,
        request: SpawnRequest,
    } = 1,
    Start {
        id: SessionId,
    } = 2,
    Stop {
        id: SessionId,
    } = 3,
    Signal {
        id: SessionId,
        sig: Signal,
    } = 4,
    Remove {
        id: SessionId,
    } = 5,
    StopAll = 6,
    UpdateMetadata {
        id: SessionId,
        labels: BTreeMap<String, String>,
    } = 7,
    SetSizePolicy {
        session: SessionId,
        policy: SizePolicy,
    } = 8,
    SetColorProfile {
        session: SessionId,
        profile: ColorProfile,
    } = 9,
    SetOutputTap {
        session: SessionId,
        enabled: bool,
    } = 10,
    ReadScreen {
        session: SessionId,
        history: bool,
    } = 11,
    ReadCursor {
        session: SessionId,
    } = 12,
    ReadModeFlags {
        session: SessionId,
    } = 13,
    CaptureSnapshot {
        session: SessionId,
        owner: ClientId,
    } = 14,
    ReadFacts {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<InputCursor>,
    } = 15,
    Resize {
        session: SessionId,
        size: Size,
    } = 16,
    WriteInput {
        session: SessionId,
        payload: InputPayload,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        guard: Option<Guard>,
    } = 17,
    Detach {
        route: RouteId,
        reason: DetachReason,
    } = 18,
    AdoptAll = 20,
    Adopt {
        id: SessionId,
    } = 21,
    SpawnService {
        service_id: ServiceId,
        argv: Vec<String>,
        env: BTreeMap<String, String>,
        cwd: String,
        limits: ServiceLimits,
    } = 22,
    StopService {
        id: ServiceId,
    } = 23,
    RemoveService {
        id: ServiceId,
    } = 24,
    EndEpoch {
        service: ServiceId,
    } = 25,
    SetNotificationPolicy {
        session: SessionId,
        policy: NotificationPolicy,
    } = 26,
}

/// The `Ok` value of a completion: the `Ok` column of the A2-1 table.
///
/// Clause: Core A2-1, Core A3-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum OpOutput {
    /// `()`: `Signal`, `StopAll`, `UpdateMetadata`, `SetSizePolicy`, `SetColorProfile`, `SetOutputTap`,
    /// `Detach`, `AdoptAll`, `RemoveService`, `EndEpoch` and `SetNotificationPolicy`.
    Unit = 1,
    /// `Create`, `Start` and `Adopt`.
    Record(SessionRecord) = 2,
    /// `Stop`.
    End(SessionEnd) = 3,
    Screen(Screen) = 4,
    Cursor(Cursor) = 5,
    Modes(Modes) = 6,
    Capture(Capture) = 7,
    Facts(Facts) = 8,
    Resize(ResizeResult) = 9,
    Input(InputResult) = 10,
    /// `SpawnService`.
    Spawn(SpawnReport) = 12,
    /// `StopService` (A16-1).
    ServiceEnd(ServiceEnd) = 13,
    /// `Remove` (A6-4).
    RemoveReport(RemoveReport) = 14,
}

/// The result of a completed operation.
///
/// Clause: Core 2, Core A2-1, Core AM-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum OpResult {
    Ok(OpOutput) = 1,
    Err(CoreError) = 2,
}
