//! Entity family declarations, session-family frames and entity-provider payloads.
//!
//! Clause: HP EN-1, EN-2, EN-4, ST-7, EV-6, A2-5, A2-7, A2-28, A5-1.

use crate::common::OpenObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A declared entity family (HP EN-1). `kind` is the family name in the plugin's own namespace, `schema_version` is `M.m`.
/// EN-1 names `actions` ("argument and result schemas and required permissions") without field names, so each action is an open object.
/// Other keys are accepted: the contract states no more fields here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityFamilyDecl {
    pub kind: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub schema: Value,
    pub schema_version: String,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub verbs: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_vec_object()))]
    pub actions: Option<Vec<OpenObject>>,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

/// What an `entity_provider` handler is asked for (HP A2-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRequest {
    Snapshot,
    Page,
}

/// `Invoke.payload` of an `entity_provider` handler (HP A2-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityProviderPayload {
    pub kind: String,
    pub request: ProviderRequest,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cursor: Option<String>,
}

/// One entity record: the row's `id` and the record body. The family schema validates `entity` only (HP A5-1, EN-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityRecord {
    pub id: String,
    pub entity: OpenObject,
}

/// `Result.value` of an `entity_provider` handler (HP A2-5, A5-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EntityProviderResult {
    pub records: Vec<EntityRecord>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub cursor: Option<String>,
}

/// The change type of `entity.publish` (HP EN-4, A2-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublishType {
    Upsert,
    Patch,
    Remove,
}

/// The status of `entity.publish` (HP EN-4, A2-7). The plugin does nothing for `pending_gap` or `resync_scheduled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    Accepted,
    PendingGap,
    ResyncScheduled,
}

/// The operation of a session-family delta (HP A2-28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeltaOp {
    Upsert,
    Remove,
}

/// The frames of `hub/session_family` (HP EV-6, A2-28). A `session` is a row of the Hub-client `session` resource. The Hub's internal generation is not exposed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "frame", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum SessionFamilyFrame {
    SnapshotBegin {},
    SnapshotChunk {
        sessions: Vec<crate::rows::SessionRow>,
    },
    SnapshotEnd {},
    Delta {
        op: DeltaOp,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::common::opt_no_null"
        )]
        session: Option<crate::rows::SessionRow>,
        session_id: String,
    },
}
