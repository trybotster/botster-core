//! Rows that this contract carries from the Hub-client contract: the stated fields are typed and required fields are required.
//!
//! Clause: HP SS-1, Appendix A "Row schemas (by reference)", A2-16, CA-13, CA-20, SA-1; the fields come from the Hub-client `session`, `session_type`
//! rows (section 6). A row also accepts descriptive fields that the Hub-client contract adds (HP section 18: unknown descriptive fields are ignored).

use crate::common::OpenObject;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `session` row is the Hub-client `session` kind. It is defined once, in `botster-hub-contract` (steward ruling R-11), and re-exported here with its parts.
pub use botster_hub_contract::prelude::SessionRow;

/// `interaction` of a session or a session type (HC section 6, HP CA-18).
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Interaction {
    Interactive,
    Noninteractive,
}

/// `presentation` of a session or a session type (HC section 6, HP CA-18).
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    Default,
    Plugin,
    Hidden,
}

/// The source of a session type (Hub-client `session_type` row, A.9): `"device"` or `{repo: {target_id}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SessionTypeSource {
    Device(DeviceSource),
    Repo { repo: RepoSource },
}

/// The word `"device"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DeviceSource {
    #[serde(rename = "device")]
    Device,
}

/// `{target_id}` of a repo-sourced session type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoSource {
    pub target_id: String,
}

/// A `session_type` row (HP Appendix A rows, A2-16). `context_keys`, `allowed_environment_overrides`, `interaction` and `presentation` are the A2-16 additions for `session_types.show`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeRow {
    pub id: String,
    pub source: SessionTypeSource,
    pub label: String,
    pub description: String,
    pub editable: bool,
    /// The targets the type is available on. The Hub-client row names the field without its element type.
    pub targets: Vec<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub port_env: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub notifications: Option<crate::manifest::Notifications>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub context_keys: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub allowed_environment_overrides: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<Interaction>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub presentation: Option<Presentation>,
    #[serde(flatten)]
    pub extra: OpenObject,
}

/// `kind` of a spawn target (HP A2-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Git,
    Directory,
}

/// `git: {repo, current_branch?, default_branch?}` of a git target (HP CA-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitInfo {
    pub repo: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub current_branch: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub default_branch: Option<String>,
}

/// A spawn target row: `id`, `label`, `enabled`, `kind`, `root`, `metadata`, and `git` for a git target (HP Appendix A rows, A2-16, CA-13). No absolute host path by default (CA-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnTargetRow {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub kind: TargetKind,
    pub root: String,
    pub metadata: OpenObject,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub git: Option<GitInfo>,
    #[serde(flatten)]
    pub extra: OpenObject,
}

/// `status` of a worktree (HP A2-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    Creating,
    Ready,
    Occupied,
    Deleting,
    Failed,
}

/// A worktree row: `worktree_id`, `target_id`, `label`, `path`, `status`, `git?`, `metadata` (HP Appendix A rows, A2-16, CA-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeRow {
    pub worktree_id: String,
    pub target_id: String,
    pub label: String,
    pub path: String,
    pub status: WorktreeStatus,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub git: Option<GitInfo>,
    pub metadata: OpenObject,
    #[serde(flatten)]
    pub extra: OpenObject,
}
