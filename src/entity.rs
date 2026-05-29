//! Entity frame contracts for cross-client state sync.

use serde::{Deserialize, Serialize};

/// Entity family/type name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityKind(pub String);

/// Entity id within a family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(pub String);

/// Shared entity-store frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityFrame {
    /// Replace the complete snapshot for an entity kind.
    Snapshot {
        /// Entity family.
        kind: EntityKind,
        /// Entity records.
        records: Vec<serde_json::Value>,
    },
    /// Insert or replace one entity record.
    Upsert {
        /// Entity family.
        kind: EntityKind,
        /// Entity id.
        id: EntityId,
        /// Entity record.
        record: serde_json::Value,
    },
    /// Patch one entity record.
    Patch {
        /// Entity family.
        kind: EntityKind,
        /// Entity id.
        id: EntityId,
        /// Patch payload.
        patch: serde_json::Value,
    },
    /// Remove one entity record.
    Remove {
        /// Entity family.
        kind: EntityKind,
        /// Entity id.
        id: EntityId,
    },
}
