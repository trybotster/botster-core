//! Entity frame contracts for cross-client state sync.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

/// Entity family/type name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityKind(pub String);

impl EntityKind {
    /// Return the raw entity type string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for EntityKind {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for EntityKind {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Entity id within a family.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub String);

impl EntityId {
    /// Return the raw entity id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for EntityId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for EntityId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Shared entity-store frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntityFrame {
    /// Replace the complete snapshot for an entity family.
    #[serde(rename = "entity_snapshot")]
    Snapshot {
        /// Entity family.
        #[serde(rename = "entity_type")]
        entity_type: EntityKind,
        /// Monotonic sequence for the entity family.
        snapshot_seq: u64,
        /// Entity records.
        items: Vec<Value>,
    },
    /// Replace records matching a top-level scope without resetting the family baseline.
    #[serde(rename = "entity_scoped_snapshot")]
    ScopedSnapshot {
        /// Entity family.
        #[serde(rename = "entity_type")]
        entity_type: EntityKind,
        /// Monotonic sequence for the scoped hydration response.
        snapshot_seq: u64,
        /// Exact top-level field matches that define the replacement scope.
        scope: Map<String, Value>,
        /// Entity records for the scope.
        items: Vec<Value>,
    },
    /// Insert or replace one entity record.
    #[serde(rename = "entity_upsert")]
    Upsert {
        /// Entity family.
        #[serde(rename = "entity_type")]
        entity_type: EntityKind,
        /// Monotonic sequence for the entity family.
        snapshot_seq: u64,
        /// Entity id.
        id: EntityId,
        /// Entity record.
        entity: Value,
    },
    /// Patch one entity record with top-level replacement semantics.
    #[serde(rename = "entity_patch")]
    Patch {
        /// Entity family.
        #[serde(rename = "entity_type")]
        entity_type: EntityKind,
        /// Monotonic sequence for the entity family.
        snapshot_seq: u64,
        /// Entity id.
        id: EntityId,
        /// Sparse top-level patch payload.
        patch: Value,
    },
    /// Remove one entity record.
    #[serde(rename = "entity_remove")]
    Remove {
        /// Entity family.
        #[serde(rename = "entity_type")]
        entity_type: EntityKind,
        /// Monotonic sequence for the entity family.
        snapshot_seq: u64,
        /// Entity id.
        id: EntityId,
    },
}

impl EntityFrame {
    /// Return this frame's entity family.
    pub fn entity_type(&self) -> &EntityKind {
        match self {
            Self::Snapshot { entity_type, .. }
            | Self::ScopedSnapshot { entity_type, .. }
            | Self::Upsert { entity_type, .. }
            | Self::Patch { entity_type, .. }
            | Self::Remove { entity_type, .. } => entity_type,
        }
    }
}

/// Result of applying an entity frame to a store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityApplyStatus {
    /// The frame changed store state.
    Applied,
    /// The frame was stale for the store's current sequence gate.
    DroppedStale,
    /// The frame was valid but had no target row to mutate.
    Noop,
}

/// Entity contract or store application error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntityError {
    /// Entity type is neither a reserved built-in nor a plugin-owned namespaced family.
    #[error("invalid entity type: {0}")]
    InvalidEntityType(String),
    /// Entity type belongs to a different plugin namespace.
    #[error("entity type {entity_type} is not owned by plugin {owner_plugin}")]
    PluginNamespaceMismatch {
        /// Entity type that failed validation.
        entity_type: String,
        /// Expected plugin owner.
        owner_plugin: String,
    },
    /// Plugin entity families must use the default id field.
    #[error("plugin entity type {entity_type} must use id_field=\"id\"")]
    InvalidPluginIdField {
        /// Entity type that failed validation.
        entity_type: String,
    },
    /// A record is not a JSON object.
    #[error("entity record for {entity_type} must be a JSON object")]
    InvalidRecordShape {
        /// Entity type that failed validation.
        entity_type: String,
    },
    /// A record is missing its id or has a non-string/empty id.
    #[error("entity record for {entity_type} requires non-empty string id field {id_field}")]
    InvalidRecordId {
        /// Entity type that failed validation.
        entity_type: String,
        /// Field used to extract the id.
        id_field: String,
    },
    /// A patch payload is not a JSON object.
    #[error("entity patch for {entity_type} must be a JSON object")]
    InvalidPatch {
        /// Entity type that failed validation.
        entity_type: String,
    },
    /// A scoped snapshot has no usable scope.
    #[error("entity scoped snapshot for {entity_type} requires a non-empty object scope")]
    InvalidScope {
        /// Entity type that failed validation.
        entity_type: String,
    },
}

/// Reusable validation helpers for entity family contracts.
pub struct EntityContract;

impl EntityContract {
    /// Return whether an entity type is reserved for core built-ins.
    pub fn is_reserved_builtin(entity_type: &str) -> bool {
        matches!(
            entity_type,
            "session"
                | "workspace"
                | "spawn_target"
                | "worktree"
                | "hub"
                | "connection_code"
                | "template"
                | "session_action"
        )
    }

    /// Return whether an entity type is plugin namespaced.
    pub fn is_plugin_entity_type(entity_type: &str) -> bool {
        let Some((plugin, family)) = entity_type.split_once('.') else {
            return false;
        };

        !plugin.is_empty() && !family.is_empty()
    }

    /// Validate an entity type, optionally requiring a matching plugin owner.
    pub fn validate_entity_type(
        entity_type: &EntityKind,
        owner_plugin: Option<&str>,
    ) -> Result<(), EntityError> {
        let raw = entity_type.as_str();

        if Self::is_reserved_builtin(raw) {
            return Ok(());
        }

        if !Self::is_plugin_entity_type(raw) {
            return Err(EntityError::InvalidEntityType(raw.to_string()));
        }

        if let Some(owner_plugin) = owner_plugin {
            let prefix = raw
                .split_once('.')
                .map(|(prefix, _)| prefix)
                .unwrap_or_default();
            if prefix != owner_plugin {
                return Err(EntityError::PluginNamespaceMismatch {
                    entity_type: raw.to_string(),
                    owner_plugin: owner_plugin.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Return the default record id field for an entity type.
    pub fn default_id_field(entity_type: &str) -> &'static str {
        match entity_type {
            "session" => "session_uuid",
            "workspace" => "workspace_id",
            "spawn_target" => "target_id",
            "worktree" => "worktree_path",
            "hub" | "connection_code" => "hub_id",
            _ => "id",
        }
    }

    /// Validate an id-field registration for an entity type.
    pub fn validate_id_field(entity_type: &EntityKind, id_field: &str) -> Result<(), EntityError> {
        Self::validate_entity_type(entity_type, None)?;

        if Self::is_plugin_entity_type(entity_type.as_str()) && id_field != "id" {
            return Err(EntityError::InvalidPluginIdField {
                entity_type: entity_type.as_str().to_string(),
            });
        }

        Ok(())
    }

    /// Extract a non-empty string id from a record using the entity type's default id field.
    pub fn extract_record_id(
        entity_type: &EntityKind,
        record: &Value,
    ) -> Result<EntityId, EntityError> {
        let id_field = Self::default_id_field(entity_type.as_str());
        Self::extract_record_id_with_field(entity_type, record, id_field)
    }

    /// Extract a non-empty string id from a record using an explicit id field.
    pub fn extract_record_id_with_field(
        entity_type: &EntityKind,
        record: &Value,
        id_field: &str,
    ) -> Result<EntityId, EntityError> {
        Self::validate_id_field(entity_type, id_field)?;

        let Some(object) = record.as_object() else {
            return Err(EntityError::InvalidRecordShape {
                entity_type: entity_type.as_str().to_string(),
            });
        };

        match object.get(id_field).and_then(Value::as_str) {
            Some(id) if !id.is_empty() => Ok(EntityId(id.to_string())),
            _ => Err(EntityError::InvalidRecordId {
                entity_type: entity_type.as_str().to_string(),
                id_field: id_field.to_string(),
            }),
        }
    }
}

/// Ordered JSON entity records for one entity family.
#[derive(Debug, Clone, Default)]
pub struct EntityStore {
    snapshot_seq: u64,
    order: Vec<EntityId>,
    records: HashMap<EntityId, Value>,
}

impl EntityStore {
    /// Build an empty entity store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the current whole-family sequence gate.
    pub fn snapshot_seq(&self) -> u64 {
        self.snapshot_seq
    }

    /// Return the number of stored records.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Return whether no records are currently stored.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Return one stored record by id.
    pub fn get(&self, id: &EntityId) -> Option<&Value> {
        self.records.get(id)
    }

    /// Iterate records in insertion/snapshot order.
    pub fn iter(&self) -> impl Iterator<Item = (&EntityId, &Value)> {
        self.order
            .iter()
            .filter_map(|id| self.records.get(id).map(|record| (id, record)))
    }

    /// Apply a frame for this entity family.
    pub fn apply_frame(&mut self, frame: &EntityFrame) -> Result<EntityApplyStatus, EntityError> {
        match frame {
            EntityFrame::Snapshot {
                entity_type,
                snapshot_seq,
                items,
            } => self.apply_snapshot(entity_type, *snapshot_seq, items),
            EntityFrame::ScopedSnapshot {
                entity_type,
                snapshot_seq,
                scope,
                items,
            } => self.apply_scoped_snapshot(entity_type, *snapshot_seq, scope, items),
            EntityFrame::Upsert {
                entity_type,
                snapshot_seq,
                id,
                entity,
            } => self.apply_upsert(entity_type, *snapshot_seq, id, entity),
            EntityFrame::Patch {
                entity_type,
                snapshot_seq,
                id,
                patch,
            } => self.apply_patch(entity_type, *snapshot_seq, id, patch),
            EntityFrame::Remove {
                entity_type,
                snapshot_seq,
                id,
            } => self.apply_remove(entity_type, *snapshot_seq, id),
        }
    }

    /// Apply a full-family authoritative snapshot.
    pub fn apply_snapshot(
        &mut self,
        entity_type: &EntityKind,
        snapshot_seq: u64,
        items: &[Value],
    ) -> Result<EntityApplyStatus, EntityError> {
        EntityContract::validate_entity_type(entity_type, None)?;

        if snapshot_seq < self.snapshot_seq {
            return Ok(EntityApplyStatus::DroppedStale);
        }

        let mut next_order = Vec::with_capacity(items.len());
        let mut next_records = HashMap::with_capacity(items.len());
        for item in items {
            let id = EntityContract::extract_record_id(entity_type, item)?;
            if !next_records.contains_key(&id) {
                next_order.push(id.clone());
            }
            next_records.insert(id, item.clone());
        }

        self.snapshot_seq = snapshot_seq;
        self.order = next_order;
        self.records = next_records;

        Ok(EntityApplyStatus::Applied)
    }

    /// Apply a scoped partial replacement without advancing the whole-family gate.
    pub fn apply_scoped_snapshot(
        &mut self,
        entity_type: &EntityKind,
        snapshot_seq: u64,
        scope: &Map<String, Value>,
        items: &[Value],
    ) -> Result<EntityApplyStatus, EntityError> {
        EntityContract::validate_entity_type(entity_type, None)?;

        if scope.is_empty() {
            return Err(EntityError::InvalidScope {
                entity_type: entity_type.as_str().to_string(),
            });
        }

        if snapshot_seq < self.snapshot_seq {
            return Ok(EntityApplyStatus::DroppedStale);
        }

        let mut replacement_ids = Vec::with_capacity(items.len());
        let mut replacements = HashMap::with_capacity(items.len());
        for item in items {
            let id = EntityContract::extract_record_id(entity_type, item)?;
            if !replacement_ids.contains(&id) {
                replacement_ids.push(id.clone());
            }
            replacements.insert(id, item.clone());
        }

        let matching_ids: Vec<EntityId> = self
            .iter()
            .filter(|(_, record)| record_matches_scope(record, scope))
            .map(|(id, _)| id.clone())
            .collect();
        for id in matching_ids {
            self.records.remove(&id);
        }
        self.order.retain(|id| self.records.contains_key(id));

        for id in replacement_ids {
            if !self.records.contains_key(&id) {
                self.order.push(id.clone());
            }
            if let Some(record) = replacements.remove(&id) {
                self.records.insert(id, record);
            }
        }

        Ok(EntityApplyStatus::Applied)
    }

    /// Apply a single-record upsert.
    pub fn apply_upsert(
        &mut self,
        entity_type: &EntityKind,
        snapshot_seq: u64,
        id: &EntityId,
        entity: &Value,
    ) -> Result<EntityApplyStatus, EntityError> {
        EntityContract::validate_entity_type(entity_type, None)?;

        if snapshot_seq <= self.snapshot_seq {
            return Ok(EntityApplyStatus::DroppedStale);
        }

        let record_id = EntityContract::extract_record_id(entity_type, entity)?;
        if &record_id != id {
            return Err(EntityError::InvalidRecordId {
                entity_type: entity_type.as_str().to_string(),
                id_field: EntityContract::default_id_field(entity_type.as_str()).to_string(),
            });
        }

        self.insert_record(id.clone(), entity.clone());
        self.snapshot_seq = snapshot_seq;

        Ok(EntityApplyStatus::Applied)
    }

    /// Apply a top-level patch to one record.
    pub fn apply_patch(
        &mut self,
        entity_type: &EntityKind,
        snapshot_seq: u64,
        id: &EntityId,
        patch: &Value,
    ) -> Result<EntityApplyStatus, EntityError> {
        EntityContract::validate_entity_type(entity_type, None)?;

        if snapshot_seq <= self.snapshot_seq {
            return Ok(EntityApplyStatus::DroppedStale);
        }

        let Some(patch) = patch.as_object() else {
            return Err(EntityError::InvalidPatch {
                entity_type: entity_type.as_str().to_string(),
            });
        };

        let Some(Value::Object(record)) = self.records.get_mut(id) else {
            self.snapshot_seq = snapshot_seq;
            return Ok(EntityApplyStatus::Noop);
        };

        for (key, value) in patch {
            record.insert(key.clone(), value.clone());
        }
        self.snapshot_seq = snapshot_seq;

        Ok(EntityApplyStatus::Applied)
    }

    /// Apply a single-record removal.
    pub fn apply_remove(
        &mut self,
        entity_type: &EntityKind,
        snapshot_seq: u64,
        id: &EntityId,
    ) -> Result<EntityApplyStatus, EntityError> {
        EntityContract::validate_entity_type(entity_type, None)?;

        if snapshot_seq <= self.snapshot_seq {
            return Ok(EntityApplyStatus::DroppedStale);
        }

        let removed = self.records.remove(id).is_some();
        if removed {
            self.order.retain(|existing_id| existing_id != id);
        }
        self.snapshot_seq = snapshot_seq;

        Ok(if removed {
            EntityApplyStatus::Applied
        } else {
            EntityApplyStatus::Noop
        })
    }

    fn insert_record(&mut self, id: EntityId, record: Value) {
        if !self.records.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.records.insert(id, record);
    }
}

/// Generic collection of entity stores keyed by entity family.
#[derive(Debug, Clone, Default)]
pub struct EntityStores {
    stores: HashMap<EntityKind, EntityStore>,
}

impl EntityStores {
    /// Build an empty store collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a protocol frame to the matching family store.
    pub fn apply_frame(&mut self, frame: &EntityFrame) -> Result<EntityApplyStatus, EntityError> {
        let entity_type = frame.entity_type().clone();
        EntityContract::validate_entity_type(&entity_type, None)?;

        self.stores
            .entry(entity_type)
            .or_default()
            .apply_frame(frame)
    }

    /// Return the store for one entity family.
    pub fn get(&self, entity_type: &EntityKind) -> Option<&EntityStore> {
        self.stores.get(entity_type)
    }

    /// Return the store for one entity family, creating it if needed.
    pub fn get_mut(&mut self, entity_type: EntityKind) -> &mut EntityStore {
        self.stores.entry(entity_type).or_default()
    }
}

fn record_matches_scope(record: &Value, scope: &Map<String, Value>) -> bool {
    let Some(record) = record.as_object() else {
        return false;
    };

    scope
        .iter()
        .all(|(key, expected_value)| record.get(key) == Some(expected_value))
}
