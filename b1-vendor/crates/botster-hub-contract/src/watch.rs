//! Watch frames (HC section 5): the baseline, deltas, batches and the end of a watch.
//!
//! The layout of the sub-frame tag is pending steward ruling H2: a `watch` frame carries `frame` as its sub-frame tag.

use crate::wire::{open_enum, opt_no_null};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

open_enum! {
    /// `snapshot_begin.reason`: absent for the first baseline, and one of these for a resync (A.2).
    ///
    /// Clause: HC A.2, WT-4.
    pub enum SnapshotReason {
        Overflow = "overflow",
        SourceReset = "source_reset",
        OwnerReloaded = "owner_reloaded",
    }
}

open_enum! {
    /// `watch_closed.reason` (A.2).
    ///
    /// Clause: HC A.2, WT-5, BD-2.
    pub enum WatchClosedReason {
        OwnerUnloaded = "owner_unloaded",
        OwnerFailed = "owner_failed",
        KindRemoved = "kind_removed",
        PermissionRevoked = "permission_revoked",
        ItemTooLarge = "item_too_large",
    }
}

/// One change of a delta or a batch.
///
/// Clause: HC WT-2, WT-3.
// pending steward ruling H2: the `frame` tag of a batch change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Change {
    Upsert { id: String, value: Value },
    Patch { id: String, patch: Value },
    Remove { id: String },
}

/// The frames of one watch. Frames of one watch are ordered by the transport and carry no sequence number (WT-2).
///
/// Clause: HC WT-1 to WT-5, EN-2.
// pending steward ruling H2: the `frame` tag of the watch sub-frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum WatchFrame {
    SnapshotBegin {
        watch_id: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "opt_no_null"
        )]
        reason: Option<SnapshotReason>,
    },
    SnapshotItems {
        watch_id: String,
        items: Vec<Value>,
    },
    SnapshotEnd {
        watch_id: String,
    },
    Upsert {
        watch_id: String,
        id: String,
        value: Value,
    },
    Patch {
        watch_id: String,
        id: String,
        patch: Value,
    },
    Remove {
        watch_id: String,
        id: String,
    },
    BatchBegin {
        watch_id: String,
    },
    BatchItems {
        watch_id: String,
        changes: Vec<Change>,
    },
    BatchEnd {
        watch_id: String,
    },
    WatchClosed {
        watch_id: String,
        reason: WatchClosedReason,
    },
}
