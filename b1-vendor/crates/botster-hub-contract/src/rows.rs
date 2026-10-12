//! The rows of the Hub-owned kinds (HC section 6, A.8, A2). Each struct is the `schema` of its kind (DS-1, DS-2).
//!
//! A field with no value is absent, never `null` (A.1). A reader ignores an unknown field (EN-6).

use crate::describe::KindDescriptor;
use crate::events::Severity;
use crate::refs::SessionRef;
use crate::wire::{lenient, open_enum, opt_no_null, OpenObject, Timestamp};
use botster_route_codec::prelude::{LifecycleClass, NotificationSource, SessionState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------- session

open_enum! {
    /// `activity` of a session: idle after the Hub parameter `activity_idle_ms` with no output.
    ///
    /// Clause: HC section 6 (`session`), parity CG-4.
    pub enum Activity {
        Active = "active",
        Idle = "idle",
    }
}

open_enum! {
    /// `interaction` of a session or a session type (HP CA-18).
    ///
    /// Clause: HC section 6, A2 H-21.
    pub enum Interaction {
        Interactive = "interactive",
        Noninteractive = "noninteractive",
    }
}

open_enum! {
    /// `presentation`: clients leave `plugin` and `hidden` sessions out of the default list. It never limits authority (G-18).
    ///
    /// Clause: HC section 6 (`session`).
    pub enum Presentation {
        Default = "default",
        Plugin = "plugin",
        Hidden = "hidden",
    }
}

/// `exit` of a session. It is always present: `{}` until the process ends, then `code`, `signal` or both as known.
///
/// Clause: HC A.8, steward ruling R-11.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionExit {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub code: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub signal: Option<i64>,
}

/// `size{rows, cols}`.
///
/// Clause: HC A.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSize {
    pub rows: u32,
    pub cols: u32,
}

/// `attention?: { since, source, title?, body? }`. It is never inside `metadata`.
///
/// Clause: HC SF-5d, steward ruling R-11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Attention {
    pub since: Timestamp,
    /// `bell`, `osc9` or `osc777`: the closed set of the codec (R-11).
    pub source: NotificationSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub body: Option<String>,
}

/// The repo source of a session type.
///
/// Clause: HC A.9 (`session_type` create).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoSource {
    pub target_id: String,
}

/// The source of a session type: `"device"` or `{ repo: { target_id } }`.
///
/// Clause: HC A.8, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TypeSource {
    Device,
    Repo(RepoSource),
}

/// `session_type{id, source}` of a session row.
///
/// Clause: HC A.8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeRef {
    pub id: String,
    pub source: TypeSource,
}

/// A `session` row: the one shape of the HC `session` kind and of HP SS-1, A2-28.
///
/// Required: `id`, `hub_id`, `label`, `state`, `lifecycle_class`, `exit`, `size`, `session_type`, `traits`, `restartable`, `activity`, `presentation`, `metadata`.
/// Optional, absent when there is no value (never `null`, A.1): `failure_reason`, `role`, `interaction`, `port`, `owner`, `worktree_id`, `branch`, `title`, `task`, `attention`.
/// A row also accepts descriptive fields that this contract adds later (EN-6).
///
/// Clause: HC section 6 (`session`), A.8, A.1, A2 H-20, steward ruling R-11.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRow {
    pub id: String,
    pub hub_id: String,
    pub label: String,
    pub state: SessionState,
    pub lifecycle_class: LifecycleClass,
    pub exit: SessionExit,
    pub size: SessionSize,
    pub session_type: SessionTypeRef,
    pub traits: Vec<String>,
    pub restartable: bool,
    pub activity: Activity,
    pub presentation: Presentation,
    /// A map of plugin namespace to a JSON object, written through HP `sessions.update`; `{}` when no plugin has written.
    pub metadata: BTreeMap<String, OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub failure_reason: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub role: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub interaction: Option<Interaction>,
    /// The allocated port when the session type asks for one (HP CA-20).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub port: Option<u16>,
    /// The key of the plugin that spawned the session.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub owner: Option<String>,
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
    pub branch: Option<String>,
    /// Only the terminal's latest title.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub task: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub attention: Option<Attention>,
    #[serde(flatten)]
    pub extra: OpenObject,
}

// ---------------------------------------------------------------- hub

open_enum! {
    /// `software.channel`.
    ///
    /// Clause: HC section 6 (`hub`).
    pub enum SoftwareChannel {
        Dev = "dev",
        Release = "release",
    }
}

/// `software{revision, channel}`: the commit sha for `dev`, the version for `release`.
///
/// Clause: HC section 6 (`hub`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Software {
    pub revision: String,
    pub channel: SoftwareChannel,
}

/// A live process identity that is not known to equal the installed one. `channel` and `commit` are absent when unknown.
///
/// Clause: HC section 6 (`hub` `components`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunningIdentity {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub channel: Option<SoftwareChannel>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub commit: Option<String>,
    pub count: u64,
}

/// The installed identity of one component.
///
/// Clause: HC section 6 (`hub` `components`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Component {
    pub channel: SoftwareChannel,
    pub commit: String,
    /// An absolute path, only for a `dev` build, shown to operators only.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub checkout: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub running: Option<Vec<RunningIdentity>>,
}

/// `components: { hub, worker, plugin_hosts, mcp_bridge }`.
///
/// Clause: HC section 6 (`hub`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Components {
    pub hub: Component,
    pub worker: Component,
    pub plugin_hosts: Component,
    pub mcp_bridge: Component,
}

/// `{ reason }`, the data of a failed state.
///
/// Clause: HC A.1b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReasonData {
    pub reason: String,
}

/// `update.state` of the `hub` row.
///
/// Clause: HC A2 H-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum UpdateState {
    Idle,
    Checking,
    Available,
    Downloading,
    Installing,
    Failed(ReasonData),
    #[serde(other)]
    Other,
}

lenient! { UpdateState => ["idle", "checking", "available", "downloading", "installing", "failed"] }

/// `{ version }` of an available update.
///
/// Clause: HC A2 H-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AvailableUpdate {
    pub version: String,
}

/// `update` of the `hub` row.
///
/// Clause: HC A2 H-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateStatus {
    pub state: UpdateState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub available: Option<AvailableUpdate>,
}

open_enum! {
    /// `hub.state`: `recovering` while it adopts workers.
    ///
    /// Clause: HC section 6 (`hub`), KR-1.
    pub enum HubRunState {
        Starting = "starting",
        Recovering = "recovering",
        Ready = "ready",
    }
}

/// The `hub` row (one row, `id` = `hub_id`).
///
/// Clause: HC section 6 (`hub`), SF-5b, PR-0, A2 CF-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HubRow {
    pub id: String,
    pub software: Software,
    /// The session-worker protocol that this Hub speaks (Core AD-4).
    pub worker_protocol: u16,
    pub components: Components,
    pub pid: u32,
    pub uptime_seconds: u64,
    /// Operators only.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub data_root: Option<String>,
    pub sessions: u64,
    pub plugins: u64,
    pub update: UpdateStatus,
    /// The VAPID public key: base64url without padding of the uncompressed P-256 point (A2 G-20).
    pub push_public_key: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub web_origin: Option<String>,
    pub state: HubRunState,
}

// ---------------------------------------------------------------- session_type, spawn_target, worktree

open_enum! {
    /// `notifications.mode` (SF-5c).
    ///
    /// Clause: HC SF-5c.
    pub enum NotificationMode {
        All = "all",
        None = "none",
        Matching = "matching",
    }
}

/// `notifications` of a session type (SF-5c, HP CA-22).
///
/// Clause: HC SF-5c.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Notifications {
    pub mode: NotificationMode,
    /// Case-sensitive literal substrings.
    pub patterns: Vec<String>,
}

/// The `session_type` row.
///
/// Clause: HC section 6 (`session_type`), MC-1c, SF-5c.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeRow {
    pub id: String,
    pub source: TypeSource,
    pub label: String,
    pub description: String,
    pub editable: bool,
    /// The element type is not fixed by HC.
    pub targets: Vec<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub port_env: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub notifications: Option<Notifications>,
}

open_enum! {
    /// `spawn_target.kind`; it is also a create input.
    ///
    /// Clause: HC A2 H-7.
    pub enum TargetKind {
        Directory = "directory",
        Git = "git",
    }
}

/// `git: { repo, current_branch?, default_branch? }`. `repo` is `owner/name`, never a path or a credential.
///
/// Clause: HC section 6 (`spawn_target`), G-24.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitInfo {
    pub repo: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub current_branch: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub default_branch: Option<String>,
}

/// The `spawn_target` row.
///
/// Clause: HC section 6 (`spawn_target`), A2 H-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnTargetRow {
    pub id: String,
    pub label: String,
    pub path: String,
    pub kind: TargetKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub git: Option<GitInfo>,
}

open_enum! {
    /// `worktree.state`.
    ///
    /// Clause: HC A2 H-7.
    pub enum WorktreeState {
        Creating = "creating",
        Ready = "ready",
        Deleting = "deleting",
        Failed = "failed",
        Missing = "missing",
    }
}

/// The `worktree` row. `active_sessions` lets a client warn before a delete.
///
/// Clause: HC section 6 (`worktree`), KR-1, A2 H-7, A6-2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeRow {
    pub id: String,
    pub target_id: String,
    pub path: String,
    /// Absent when HEAD is detached (HC A6-2).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub branch: Option<String>,
    /// The commit HEAD points at, as a full hexadecimal object name (HC A6-2).
    pub head: String,
    pub state: WorktreeState,
    pub active_sessions: Vec<SessionRef>,
}

// ---------------------------------------------------------------- package, catalog, quarantine

open_enum! {
    /// `package.kind` and the kind of a catalog entry (HP PM-11).
    ///
    /// Clause: HC section 6 (`package`, `catalog`).
    pub enum PackageKind {
        Plugin = "plugin",
        AgentDefinition = "agent_definition",
        Accessory = "accessory",
    }
}

open_enum! {
    /// `package.state`.
    ///
    /// Clause: HC section 6 (`package`).
    pub enum PackageState {
        Enabled = "enabled",
        Disabled = "disabled",
        Quarantined = "quarantined",
        Failed = "failed",
    }
}

/// `degraded { interface }` data.
///
/// Clause: HC section 6 (`package` `status`), HP PD-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DegradedData {
    pub interface: String,
}

/// `package.status`, the install lifecycle (HP PM-10).
///
/// Clause: HC section 6 (`package`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", remote = "Self")]
pub enum PackageStatus {
    Installed,
    Outdated,
    Errored,
    Live,
    Pinned,
    Staged,
    Degraded(DegradedData),
    #[serde(other)]
    Other,
}

lenient! { PackageStatus => ["installed", "outdated", "errored", "live", "pinned", "staged", "degraded"] }

/// A provided interface (`name@version`).
///
/// Clause: HC section 6 (`package` `provides`), HP PD-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provides {
    pub interface: String,
}

open_enum! {
    /// `binding.state` of a requirement.
    ///
    /// Clause: HC section 6 (`package` `requires`), HP PD-8.
    pub enum BindingState {
        Available = "available",
        Unavailable = "unavailable",
        Unmet = "unmet",
    }
}

/// The provider bound to a requirement.
///
/// Clause: HC section 6 (`package` `requires`), HP PD-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequirementBinding {
    pub interface_version: String,
    pub provider: String,
    pub provider_version: String,
    pub state: BindingState,
}

/// A required interface.
///
/// Clause: HC section 6 (`package` `requires`), HP PD-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Requires {
    pub interface: String,
    pub range: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub optional: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub binding: Option<RequirementBinding>,
}

/// A git source: the URL, an optional ref and an optional package root inside the repository.
///
/// Clause: HC A1 (`package` `source`), PKC-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitSource {
    pub url: String,
    #[serde(
        default,
        rename = "ref",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub git_ref: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub path: Option<String>,
}

/// The source of a package: `{kind: local, path}`, `{kind: git, url, ref?, path?}`, or the catalog entry it came from.
///
/// Clause: HC section 6 (`package`), A1, A.9 (`package` create).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PackageSource {
    /// A `local` path is shown to operators only.
    Local { path: String },
    Git {
        url: String,
        #[serde(
            default,
            rename = "ref",
            skip_serializing_if = "Option::is_none",
            deserialize_with = "opt_no_null"
        )]
        git_ref: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "opt_no_null"
        )]
        path: Option<String>,
    },
    Catalog {
        catalog_id: String,
        entry_id: String,
    },
}

/// `lock` of a package.
///
/// Clause: HC section 6 (`package`), HP PM-3, A1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageLock {
    pub resolved_commit: String,
    pub content_hash: String,
    pub locked_at: Timestamp,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub source_path: Option<String>,
}

open_enum! {
    /// A notice reaction's delivery channel.
    ///
    /// Clause: HC SF-5.
    pub enum Delivery {
        InApp = "in_app",
        Push = "push",
    }
}

/// A declared notice reaction, shown for display only.
///
/// Clause: HC SF-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NoticeReaction {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub subject_scope: String,
    pub text_pointer: String,
    pub ttl_ms: u64,
    pub severity: Severity,
    pub delivery: Vec<Delivery>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub url_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub tag_pointer: Option<String>,
}

/// The `package` row. `id` is the package key (HP CP-8).
///
/// Clause: HC section 6 (`package`), A1, A.1b, A7-2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageRow {
    pub id: String,
    pub name: String,
    pub kind: PackageKind,
    pub state: PackageState,
    pub status: PackageStatus,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub provides: Option<Vec<Provides>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub requires: Option<Vec<Requires>>,
    pub version: String,
    pub source: PackageSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub lock: Option<PackageLock>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub pinned_ref: Option<String>,
    /// Grants that wait for approval (HP PM-8). The element type is HP's.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub pending_grants: Option<Vec<Value>>,
    pub configuration: Value,
    /// The entrypoints of the package. The element type is HP's.
    pub entrypoints: Vec<Value>,
    pub notice_reactions: Vec<NoticeReaction>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub target_id: Option<String>,
    /// The recorded static origin (HP A2-42), shown to operators only; absent otherwise (HC A7-2).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub static_origin: Option<String>,
}

/// The status of a catalog entry: `available`, or the status of the installed package.
///
/// Clause: HC section 6 (`catalog`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EntryStatus {
    Available(AvailableTag),
    Installed(PackageStatus),
}

/// The word `available`.
///
/// Clause: HC section 6 (`catalog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AvailableTag {
    Available,
}

/// One catalog entry. Two entries may name the same `url` and `ref` with different `path`s (HP PK-7).
///
/// Clause: HC section 6 (`catalog`), A1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogEntry {
    pub id: String,
    pub kind: PackageKind,
    pub name: String,
    pub description: String,
    pub source: GitSource,
    pub status: EntryStatus,
}

/// The `catalog` row. Its content is untrusted data and never runs.
///
/// Clause: HC section 6 (`catalog`), A2 (CLI OI-9: exactly these fields).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogRow {
    pub id: String,
    pub url: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub refreshed_at: Option<Timestamp>,
    pub entries: Vec<CatalogEntry>,
}

open_enum! {
    /// `quarantine.reason`.
    ///
    /// Clause: HC A2 H-19.
    pub enum QuarantineReason {
        CrashLoop = "crash_loop",
        HashMismatch = "hash_mismatch",
        AdmissionFailed = "admission_failed",
        BootstrapFailed = "bootstrap_failed",
    }
}

/// The `quarantine` row.
///
/// Clause: HC section 6 (`quarantine`), A2 H-19.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QuarantineRow {
    pub id: String,
    pub package: String,
    pub reason: QuarantineReason,
    pub since: Timestamp,
}

// ---------------------------------------------------------------- navigation, view

open_enum! {
    /// `target.kind` of a navigation entry.
    ///
    /// Clause: HC section 6 (`navigation`).
    pub enum NavTargetKind {
        View = "view",
        Entrypoint = "entrypoint",
    }
}

/// A navigation target: a view or an entrypoint, with an optional preset selection (slot to row key).
///
/// Clause: HC section 6 (`navigation`), A2 G-17, A.9 (`resolve_route`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NavTarget {
    pub kind: NavTargetKind,
    pub id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub select: Option<BTreeMap<String, String>>,
}

/// The `navigation` row. `badge` is the ui-contract badge (UI-NB-1).
///
/// Clause: HC section 6 (`navigation`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NavigationRow {
    pub id: String,
    pub package: String,
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub icon: Option<String>,
    pub order: i64,
    pub target: NavTarget,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub badge: Option<botster_ui_contract::prelude::Badge>,
}

open_enum! {
    /// `view.component`; `embed` needs the feature `ui_embed`.
    ///
    /// Clause: HC section 6 (`view`), section 7b.
    pub enum ViewComponent {
        List = "list",
        Table = "table",
        Detail = "detail",
        Form = "form",
        Actions = "actions",
        StatusBadge = "status_badge",
        Embed = "embed",
    }
}

/// What a view binds: a kind, an optional filter and the fields it shows. A field has a stable `field_id` (SF-1).
///
/// Clause: HC section 6 (`view`), SF-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewBinds {
    pub kind: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub filter: Option<OpenObject>,
    pub fields: Vec<Value>,
}

/// What a client sees of a `secret: true` field: never its value, only whether it is set.
///
/// Clause: HC SF-3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SecretDisplay {
    pub set: bool,
}

/// The `view` row. The declarative document is owned by ui-contract (section 7b).
///
/// Clause: HC section 6 (`view`), section 7b, SF-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewRow {
    /// `<package>/<view>`.
    pub id: String,
    pub component: ViewComponent,
    pub binds: ViewBinds,
    /// The actions that the view offers, each with a stable `action_id` (SF-1).
    pub actions: Vec<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub placement: Option<botster_ui_contract::prelude::Placement>,
    /// The ui-contract view document.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub document: Option<botster_ui_contract::prelude::ViewDocument>,
}

// ---------------------------------------------------------------- plugin_log

open_enum! {
    /// `plugin_log` level (HP `log.write`).
    ///
    /// Clause: HC A2 H-33.
    pub enum LogLevel {
        Debug = "debug",
        Info = "info",
        Warn = "warn",
        Error = "error",
    }
}

open_enum! {
    /// A marker row's `gap`: `evicted` (retention dropped older records) or `rate_limited` (the source dropped records).
    ///
    /// Clause: HC section 6 (`plugin_log`).
    pub enum LogGap {
        Evicted = "evicted",
        RateLimited = "rate_limited",
    }
}

/// A log record.
///
/// Clause: HC section 6 (`plugin_log`), KR-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LogRecord {
    /// Opaque, ordered within a package.
    pub id: String,
    pub package: String,
    /// Changes at each plugin load.
    pub load_id: String,
    pub at: Timestamp,
    pub level: LogLevel,
    pub message: String,
    pub fields: OpenObject,
}

/// A marker row: a gap, or a restart.
///
/// Clause: HC section 6 (`plugin_log`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum LogMarker {
    Gap {
        gap: LogGap,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "opt_no_null"
        )]
        count: Option<u64>,
    },
    Restarted {
        restarted: String,
    },
}

/// The `plugin_log` row: a record, or a marker.
///
/// Clause: HC section 6 (`plugin_log`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PluginLogRow {
    Record(LogRecord),
    Marker(LogMarker),
}

// ---------------------------------------------------------------- operation

/// The state names that carry data: a `reason` (A.7b, HP SW-2).
///
/// Clause: HC A.7b.
pub const OPERATION_STATES_WITH_REASON: &[&str] = &["failed", "abandoned", "cancelled", "unknown"];

/// An operation state. Unit states are strings and states with data are one-key objects (A.1b). The set is non-exhaustive and per kind;
/// `terminal` tells a client whether to keep watching. A known state with data carries a `reason`. An unknown name may carry any object (EN-6).
///
/// Clause: HC section 6 (`operation`), A.7b, A.1b, EN-6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationState {
    pub name: String,
    /// The `reason` of `failed`, `abandoned`, `cancelled` and `unknown`.
    pub reason: Option<String>,
    /// The data of a state that this version does not know.
    pub data: Option<OpenObject>,
}

/// The state names that this version knows (A.7b, HP SW-2, CA-16).
///
/// Clause: HC A.7b.
pub const OPERATION_STATE_NAMES: &[&str] = &[
    "queued",
    "running",
    "succeeded",
    "failed",
    "cancelled",
    "starting",
    "installed",
    "typing",
    "delivered",
    "abandoned",
    "replaced",
    "unknown",
];

impl Serialize for OperationState {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = BTreeMap::new();
        match (&self.reason, &self.data) {
            (Some(reason), _) => {
                m.insert(&self.name, serde_json::json!({ "reason": reason }));
            }
            (None, Some(data)) => {
                m.insert(&self.name, Value::Object(data.clone()));
            }
            (None, None) => return s.serialize_str(&self.name),
        }
        m.serialize(s)
    }
}

impl<'de> Deserialize<'de> for OperationState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Value::deserialize(d)? {
            Value::String(name) => Ok(OperationState {
                name,
                reason: None,
                data: None,
            }),
            Value::Object(map) if map.len() == 1 => {
                let (name, data) = map.into_iter().next().expect("one key");
                let Value::Object(data) = data else {
                    return Err(serde::de::Error::custom("the data of a state is an object"));
                };
                if OPERATION_STATES_WITH_REASON.contains(&name.as_str()) {
                    let data: ReasonData = serde_json::from_value(Value::Object(data))
                        .map_err(serde::de::Error::custom)?;
                    Ok(OperationState {
                        name,
                        reason: Some(data.reason),
                        data: None,
                    })
                } else {
                    Ok(OperationState {
                        name,
                        reason: None,
                        data: Some(data),
                    })
                }
            }
            _ => Err(serde::de::Error::custom(
                "a state is a name or a one-key object",
            )),
        }
    }
}

impl JsonSchema for OperationState {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "OperationState".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                { "type": "string" },
                { "type": "object", "minProperties": 1, "maxProperties": 1,
                  "propertyNames": { "enum": OPERATION_STATES_WITH_REASON },
                  "additionalProperties": { "type": "object", "properties": { "reason": { "type": "string" } }, "required": ["reason"] } },
                { "type": "object", "minProperties": 1, "maxProperties": 1,
                  "propertyNames": { "not": { "enum": OPERATION_STATES_WITH_REASON } },
                  "additionalProperties": { "type": "object" } }
            ]
        })
    }
}

/// `progress` of a generic operation: `{ message, fraction? }`, `fraction` in 0..1.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Progress {
    pub message: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub fraction: Option<f64>,
}

/// The wire form of `OperationRow`. The reader checks the `result` of a known kind after it reads the members (A.7b).
#[derive(Serialize, Deserialize, JsonSchema)]
struct OperationRowWire {
    id: String,
    /// Non-exhaustive: an unknown kind is shown as "other" and the client waits for `terminal`.
    kind: String,
    state: OperationState,
    /// Whether the state is final.
    terminal: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    progress: Option<Progress>,
    /// The per-kind result of A.7b.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    result: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    error: Option<crate::error::Error>,
}

/// The `operation` row. A known kind's `result` has its A.7b type, in the reader and in the schema. An unknown kind keeps an opaque result (EN-6).
///
/// Clause: HC section 6 (`operation`), A.7b, EN-7, EN-6.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OperationRowWire")]
pub struct OperationRow {
    pub id: String,
    /// Non-exhaustive: an unknown kind is shown as "other" and the client waits for `terminal`.
    pub kind: String,
    pub state: OperationState,
    /// Whether the state is final.
    pub terminal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
    /// The per-kind result of A.7b.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::error::Error>,
}

impl TryFrom<OperationRowWire> for OperationRow {
    type Error = String;
    fn try_from(w: OperationRowWire) -> Result<Self, String> {
        let row = OperationRow {
            id: w.id,
            kind: w.kind,
            state: w.state,
            terminal: w.terminal,
            progress: w.progress,
            result: w.result,
            error: w.error,
        };
        row.check_result()
            .map_err(|e| format!("result of {}: {e}", row.kind))?;
        Ok(row)
    }
}

impl OperationRow {
    /// Checks the `result` of a known kind against its A.7b type. An unknown kind, and a row with no `result`, pass (EN-6).
    ///
    /// Clause: HC A.7b, EN-6.
    pub fn check_result(&self) -> Result<(), serde_json::Error> {
        match (
            crate::registry::operation_kind(&self.kind).and_then(|k| k.check_result),
            &self.result,
        ) {
            (Some(check), Some(result)) => check(result),
            _ => Ok(()),
        }
    }
}

impl JsonSchema for OperationRow {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "OperationRow".into()
    }
    /// The members of the row, and for each known kind with a result type, a rule that `result` has that type.
    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let mut schema = <OperationRowWire as JsonSchema>::json_schema(g);
        let rules: Vec<Value> = crate::registry::OPERATION_KINDS
            .iter()
            .filter_map(|k| {
                let reference = k.result?(g);
                Some(serde_json::json!({
                    "if": { "properties": { "kind": { "const": k.kind } }, "required": ["kind"] },
                    "then": { "properties": { "result": reference } }
                }))
            })
            .collect();
        if let Some(o) = schema.as_object_mut() {
            o.insert("allOf".into(), Value::Array(rules));
        }
        schema
    }
}

// ---------------------------------------------------------------- credentials, push, route, kind

/// The `client_credential` row.
///
/// Clause: HC section 6 (`client_credential`), PR-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientCredentialRow {
    /// The `credential_id`.
    pub id: String,
    pub created_at: Timestamp,
    pub last_seen_at: Timestamp,
}

/// The `push_subscription` row. The endpoint is never returned.
///
/// Clause: HC section 6 (`push_subscription`), SF-5b, A2 H-15.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushSubscriptionRow {
    pub id: String,
    pub credential_id: String,
    pub created_at: Timestamp,
}

/// The `route` row, limited to the owning caller.
///
/// Clause: HC section 6 (`route`), A.3, BP-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteRow {
    pub id: String,
    pub session: SessionRef,
    /// An open string (TB-0).
    pub binding: String,
    pub since: Timestamp,
}

/// The `kind` row is the `KindDescriptor` (DS-1, section 6).
///
/// Clause: HC section 6 (`kind`), DS-3.
pub type KindRow = KindDescriptor;
