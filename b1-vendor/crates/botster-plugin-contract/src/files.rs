//! The package data files, the catalog document, package sources and the lock record.
//!
//! Clause: HP PK-1 to PK-9, PM-1 to PM-3, PM-11, A2-40, A2-44, A2-45, PD-3.

use crate::manifest::{InteractionClass, Notifications};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `lifecycle` of a session type (Hub-client A.9 and its amendment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Restartable,
    OneShot,
}

/// `botster-agent.json` (HP PK-6, A2-40): exactly the fields of the client `session_type` create except `source` (Hub-client A.9, with `agent_scopes?` from its amendment 2), plus `script_file?`.
/// `script_file` is a path relative to the package root whose content is the `script`. `session_type_dir` is the package root (A2-45).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    /// An editor hint that is accepted and ignored (HP A2-43).
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub schema_ref: Option<String>,
    pub id: String,
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub role: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<InteractionClass>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub lifecycle: Option<Lifecycle>,
    pub command: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub traits: Option<Vec<String>>,
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
    pub context_keys: Option<Vec<String>>,
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
    pub notifications: Option<Notifications>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub script: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub agent_scopes: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub script_file: Option<String>,
}

/// The only role of an accessory (HP A2-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessoryRole {
    Accessory,
}

/// `botster-accessory.json` (HP PK-6, A2-40): the fields of `AgentDefinition` with `role` fixed to `accessory`.
/// An accessory is a session type for a non-agent helper session (a dev server, a shell).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Accessory {
    /// An editor hint that is accepted and ignored (HP A2-43).
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub schema_ref: Option<String>,
    pub id: String,
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub role: Option<AccessoryRole>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<InteractionClass>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub lifecycle: Option<Lifecycle>,
    pub command: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub traits: Option<Vec<String>>,
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
    pub context_keys: Option<Vec<String>>,
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
    pub notifications: Option<Notifications>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub script: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub agent_scopes: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub script_file: Option<String>,
    /// The legacy marker of an accessory (HP A2-40, CA-20, LB-19): it is `allocate_port`, one to one. See [`Accessory::allocate_port`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub port_forward: Option<bool>,
}

impl Accessory {
    /// Whether the session type allocates a port per run: the legacy `port_forward` marker maps to `allocate_port` one to one (HP A2-40, CA-20, LB-19).
    pub fn allocate_port(&self) -> bool {
        self.port_forward.unwrap_or(false)
    }
}

/// The kind of a catalog entry and of a package (HP PM-2, PK-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Plugin,
    AgentDefinition,
    Accessory,
}

/// The source of a catalog entry: a git URL, an optional ref and an optional `path` (HP PM-2, PK-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogSource {
    pub url: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "ref",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub git_ref: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub path: Option<String>,
}

/// A catalog entry (HP PM-2, A2-44): `id` is unique in the document and matches `[a-z0-9-]+`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogEntry {
    #[schemars(regex(pattern = r"^[a-z0-9-]+$"))]
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    pub description: String,
    pub source: CatalogSource,
}

/// The catalog document (HP PM-2, A2-44): served over `https` as `application/json`. An unknown key is ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Catalog {
    /// The integer `1` (HP A2-44).
    #[schemars(range(min = 1, max = 1))]
    pub catalog_version: u32,
    pub entries: Vec<CatalogEntry>,
}

/// A package source (HP PM-1, PK-5). A `local` source names the package root and has no `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageSource {
    Local {
        path: String,
    },
    Git {
        url: String,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            rename = "ref",
            deserialize_with = "crate::common::opt_no_null"
        )]
        git_ref: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::common::opt_no_null"
        )]
        path: Option<String>,
    },
    Catalog {
        catalog_id: String,
        entry_id: String,
    },
}

/// A resolved interface binding in a lock record (HP PM-3, PD-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRequire {
    pub interface: String,
    pub interface_version: String,
    pub provider: String,
    pub provider_version: String,
    pub interface_hash: String,
}

/// The lock record of an installed package (HP PM-3, PK-5). The lock is Hub state, not an interchange file (A2-50); the type names its fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockRecord {
    pub package: String,
    pub version: String,
    pub source: PackageSource,
    pub source_path: Option<String>,
    pub resolved_commit: Option<String>,
    pub content_hash: String,
    pub manifest_hash: String,
    pub approved_grants: Vec<crate::frames::Grant>,
    pub resolved_requires: Vec<ResolvedRequire>,
    /// Unix milliseconds (HP WF-5).
    pub locked_at: u64,
}

/// An interface document (HP PD-3, A2-49): at `interfaces/<name>@<MAJOR.MINOR.PATCH>.json` in the provider package. A v1 interface is calls only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InterfaceDocument {
    pub name: String,
    pub version: String,
    pub calls: Vec<crate::manifest::InterfaceExport>,
    pub rules: Value,
}
