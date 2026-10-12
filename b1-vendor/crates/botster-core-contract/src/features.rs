//! The names that `features()` reports (A2-6, A3-3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One feature name, spelled as in the A2-6 table.
///
/// Clause: Core A2-6, Core A3-3, Core SV-4, Core SV-8, Core DP-12, Core ST-6b.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[non_exhaustive]
#[repr(u8)]
pub enum Feature {
    #[serde(rename = "route_transport:stream")]
    RouteTransportStream = 1,
    #[serde(rename = "service_preamble_versions")]
    ServicePreambleVersions = 3,
    /// Optional (A2-6).
    #[serde(rename = "silence")]
    Silence = 4,
    /// Optional (A2-6).
    #[serde(rename = "size_policy_other")]
    SizePolicyOther = 5,
    /// Per worker (A2-6).
    #[serde(rename = "snapshot_graphics")]
    SnapshotGraphics = 6,
    /// Per worker (A2-6).
    #[serde(rename = "focus_report")]
    FocusReport = 7,
    /// Per platform (A2-6).
    #[serde(rename = "payload_dies_with_guardian")]
    PayloadDiesWithGuardian = 8,
    #[serde(rename = "bound:cpu_seconds")]
    BoundCpuSeconds = 9,
    #[serde(rename = "bound:open_files")]
    BoundOpenFiles = 10,
    #[serde(rename = "bound:processes")]
    BoundProcesses = 11,
    #[serde(rename = "bound:address_space_bytes")]
    BoundAddressSpaceBytes = 12,
    /// A route feature (A3-3).
    #[serde(rename = "output_dropped")]
    OutputDropped = 13,
    /// Per worker (A3-3).
    #[serde(rename = "notification_policy")]
    NotificationPolicy = 14,
}

/// The result of `features()`.
///
/// Clause: Core A2-6, Core SV-2, Core 9.3.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Features {
    pub names: BTreeSet<Feature>,
    pub service_preamble_versions: Vec<u8>,
}
