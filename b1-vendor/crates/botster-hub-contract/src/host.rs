//! Shapes that the Hub writes for a session or reads from its configuration, outside the wire: the session context file (HC MC-1c)
//! and a trusted-host entry of `hub.toml` (HC A2 CF-2, HP TR-1).

use crate::refs::SessionRef;
use crate::wire::opt_no_null;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The file at `BOTSTER_CONTEXT_FILE`: a UTF-8 JSON object that the Hub completes before the program starts. `session_type_id` and
/// `session_type_dir` are present only for a session-type spawn. A command spawn has neither, and `context` is empty. No field is
/// ever substituted.
///
/// Clause: HC MC-1c.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionContextFile {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub prompt: Option<String>,
    /// The spawn's entries by bare name (the session type's `context_keys`).
    pub context: BTreeMap<String, String>,
    pub session: SessionRef,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub session_type_id: Option<String>,
    /// The directory of the session type's definition files.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub session_type_dir: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub target_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub worktree_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub worktree_path: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub branch: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub repo: Option<String>,
}

/// One entry of the `[network] trusted_hosts` key of `hub.toml`.
///
/// Clause: HC A2 CF-2, HP TR-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrustedHost {
    pub path: String,
    pub sha256: String,
}
