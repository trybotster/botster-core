//! The plain data of Core section 3: sizes, the spawn request, the session states and the exit.

use crate::features::Feature;
use crate::ids::{InputRev, ModelRev, SessionId};
use crate::util::lenient;
use botster_route_codec::prelude::ModeFlags;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Unix seconds from the injected clock. It stamps `at` fields only (TM-1).
///
/// Clause: Core TM-1, Core A2-4.
pub type UnixSeconds = u64;

/// The pixel size of one cell, from 1.
///
/// Clause: Core 3, Core EV-8a.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CellPx {
    #[schemars(range(min = 1))]
    pub width: u32,
    #[schemars(range(min = 1))]
    pub height: u32,
}

/// A terminal size.
///
/// Clause: Core 3, Core A2-1, Core SZ-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Size {
    pub rows: u32,
    pub cols: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell_px: Option<CellPx>,
}

/// One color.
///
/// Clause: Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// The colors that the shadow terminal answers with.
///
/// Clause: Core A2-1, Core EV-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ColorProfile {
    /// Exactly 256 entries when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 256, max = 256))]
    pub palette: Option<Vec<Rgb>>,
    pub foreground: Rgb,
    pub background: Rgb,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<Rgb>,
}

/// Which notifications in dropped output count (A3-1). The default is `All`.
///
/// Clause: Core A3-1.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum NotificationPolicy {
    #[default]
    All = 1,
    None = 2,
    Matching {
        patterns: Vec<String>,
    } = 3,
}

/// The size policy of a session.
///
/// Clause: Core DP-6.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum SizePolicy {
    #[default]
    Latest = 1,
    /// Needs the feature `size_policy_other`.
    Smallest = 2,
    /// Needs the feature `size_policy_other`.
    Largest = 3,
}

/// What a host asks Core to run.
///
/// Clause: Core 3, Core A2-1, Core A3-1, Core A3-3, Core DP-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnRequest {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    pub size: Size,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_profile: Option<ColorProfile>,
    /// The default is `All` (A3-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_policy: Option<NotificationPolicy>,
    /// DP-6 says the policy is also accepted in `Create`. The default is `Latest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_policy: Option<SizePolicy>,
}

/// Why a session payload ended. A host maps an unknown value to `Other`.
///
/// Clause: Core A2-1, Core EV-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum ExitCause {
    Normal = 1,
    Signal = 2,
    HostStop = 3,
    Killed = 4,
    #[serde(other)]
    Other = 255,
}

/// How a payload ended: death by a signal has `code: None` and `signal: Some(n)`.
///
/// Clause: Core 3, Core EV-4, Core A2-1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Exit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<i32>,
    pub cause: ExitCause,
}

/// Why a session or a service is `Lost`.
///
/// Clause: Core AD-2, Core SV-8, Core A2-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum LostReason {
    WorkerGone = 1,
    WorkerUnreachable = 2,
    WorkerVersion = 3,
    RegistryCorrupt = 4,
    StartInterrupted = 5,
    GuardianLost = 6,
    EpochExhausted = 7,
    #[serde(other)]
    Other = 255,
}

/// The state of a session as `get` and `SessionState` report it.
///
/// Clause: Core 3, Core A2-1, Core LC-9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum SessionState {
    Created = 1,
    Starting = 2,
    Running = 3,
    Stopping = 4,
    Exited(Exit) = 5,
    Lost(LostReason) = 6,
    Released = 7,
}

/// How a session ended, as `Stop` completes: `Exited(Exit)` or `Lost(reason)`.
///
/// Clause: Core A2-1, Core LC-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum SessionEnd {
    Exited(Exit) = 1,
    Lost(LostReason) = 2,
}

/// A signal for `Signal`. An unsupported value is `Unsupported`.
///
/// Clause: Core LC-6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
#[repr(u8)]
pub enum Signal {
    Term = 1,
    Kill = 2,
    Int = 3,
    Hup = 4,
    Other(i32) = 255,
}

/// Why `Start` failed. A host maps an unknown reason to `Other`.
///
/// Clause: Core A2-1, Core LC-4, Core AD-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[non_exhaustive]
#[repr(u8)]
pub enum StartFailReason {
    ExecFailed {
        errno: i32,
    } = 1,
    CwdMissing = 2,
    StartupTimeout = 3,
    WorkerFailed = 4,
    GuardianFailed = 5,
    #[serde(other)]
    Other = 255,
}

/// The record of `get(id)`.
///
/// Clause: Core LC-9, Core AD-4, Core A2-1, Core A2-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRecord {
    pub id: SessionId,
    pub state: SessionState,
    pub size: Size,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<Exit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_protocol: Option<u8>,
    /// The features of the session's worker. Absent for a session with no worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_features: Option<BTreeSet<Feature>>,
}

/// The input revisions of the two source classes, as last reported.
///
/// Clause: Core IN-10, Core ST-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InputRevs {
    pub client: InputRev,
    pub host: InputRev,
}

/// The result of `terminal_state(id)`. `focused` is absent until it is authoritative.
///
/// Clause: Core ST-4, Core EV-6, Core DP-12.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerminalState {
    pub size: Size,
    pub modes: ModeFlags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_output_at: Option<UnixSeconds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    pub model_rev: ModelRev,
    pub input_rev: InputRevs,
}

/// One row of `status()`.
///
/// Clause: Core LC-10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionStatus {
    pub id: SessionId,
    pub state: SessionState,
}

/// The result of `status()`: sessions and states only.
///
/// Clause: Core LC-10.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    pub sessions: Vec<SessionStatus>,
}

lenient! {
    ExitCause => ["Normal", "Signal", "HostStop", "Killed"],
    LostReason => ["WorkerGone", "WorkerUnreachable", "WorkerVersion", "RegistryCorrupt", "StartInterrupted", "GuardianLost", "EpochExhausted"],
    StartFailReason => ["ExecFailed", "CwdMissing", "StartupTimeout", "WorkerFailed", "GuardianFailed"],
}
