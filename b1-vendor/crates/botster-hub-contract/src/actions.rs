//! Arguments and results of the fixed actions and of `create` (HC A.9, A.9b, A2 part 1 and part 2).
//!
//! `operation_id` and `idempotency_key` are request fields (EN-1, EN-7b), never arguments. An accepted action answers `{ operation }`, the full operation row (A.7b).

use crate::refs::SessionRef;
use crate::rows::*;
use crate::wire::{nullable_string, open_enum, opt_no_null, present_or_null, OpenObject};
use botster_route_codec::prelude::{HistoryUnavailable, RouteLimits};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The result of an `accepted` action: the full `operation` row at admission.
///
/// Clause: HC A.7b, EN-7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub operation: OperationRow,
}

/// The empty arguments or result: `{}`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Empty {}

// ---------------------------------------------------------------- hub

open_enum! {
    /// `check_update.state`: `ahead` means that the running version is newer than the latest release.
    ///
    /// Clause: HC A2 H-3.
    pub enum CheckState {
        UpToDate = "up_to_date",
        Ahead = "ahead",
        Available = "available",
    }
}

/// The `reason` values of a failed `check_update` (A2 H-3). The set is non-exhaustive (EN-6).
///
/// Clause: HC A2 H-3.
pub const CHECK_UPDATE_FAILURE_REASONS: &[&str] = &["check_failed", "offline", "dev_build"];

/// `check_update` result. A failed check is `state: { failed: { reason } }` (`check_failed`, `offline`, `dev_build`).
///
/// Clause: HC A.9, A2 H-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CheckUpdateState {
    Known(CheckState),
    Failed { failed: ReasonData },
}

/// The result of `check_update`.
///
/// Clause: HC A.9, A2 H-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckUpdateResult {
    pub state: CheckUpdateState,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub available: Option<AvailableUpdate>,
}

/// The result of `diagnostics`: one opaque value, never parsed by a client (P3).
///
/// Clause: HC A.9, P3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticsResult {
    pub report: Value,
}

/// Arguments of `swap` and `swap_preflight`. `target_worker_protocol` is a u16 (H-26).
///
/// Clause: HC A.9, A2 H-26.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SwapArgs {
    pub target_worker_protocol: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub drop_unadoptable: Option<bool>,
}

/// Arguments of `swap_preflight`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SwapPreflightArgs {
    pub target_worker_protocol: u16,
}

/// A session whose worker a Hub of the target protocol could not adopt.
///
/// Clause: HC A.9 (`swap_preflight`), Core AD-4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Unadoptable {
    pub session: SessionRef,
    pub worker_protocol: u16,
}

/// A session whose worker protocol is unknown.
///
/// Clause: HC A.9 (`swap_preflight`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UnknownProtocol {
    pub session: SessionRef,
}

/// The `detail` of the `swap` refusal `wrong_state { reason: unadoptable_sessions }`: the unadoptable sessions and the sessions
/// whose worker protocol is unknown.
///
/// Clause: HC A.9 (`swap`), A2 A.2 (`swap`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UnadoptableDetail {
    pub sessions: Vec<SessionRef>,
    pub unknown: Vec<SessionRef>,
}

/// The result of `swap_preflight`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SwapPreflightResult {
    pub unadoptable: Vec<Unadoptable>,
    pub unknown: Vec<UnknownProtocol>,
}

/// Arguments of `issue_pairing`.
///
/// Clause: HC PR-0, A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct IssuePairingArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub ttl_seconds: Option<u64>,
}

/// The result of `issue_pairing`. `expires_at` is Unix seconds (PR-7b).
///
/// Clause: HC PR-0, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssuePairingResult {
    pub url: String,
    pub qr_text: String,
    pub expires_at: u64,
}

/// A `{ consumer, interface }` pair.
///
/// Clause: HC A.9 (`upgrade`, `upgrade_packages`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConsumerInterface {
    pub consumer: String,
    pub interface: String,
}

/// A `{ interface, provider }` binding.
///
/// Clause: HC A.9 (`package` install, `upgrade`), HP PD-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InterfaceBinding {
    pub interface: String,
    pub provider: String,
}

/// A `{ consumer, interface, provider }` binding of a whole-set upgrade.
///
/// Clause: HC A.9 (`upgrade_packages`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConsumerBinding {
    pub consumer: String,
    pub interface: String,
    pub provider: String,
}

/// Arguments of `hub.upgrade_packages`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct UpgradePackagesArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub bindings: Option<Vec<ConsumerBinding>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub accept_breaking: Option<Vec<ConsumerInterface>>,
}

/// An ambiguous requirement and its candidate providers.
///
/// Clause: HC A.9 (`preview_package_upgrades`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Ambiguity {
    pub consumer: String,
    pub interface: String,
    pub candidates: Vec<String>,
}

/// One breaking pair of an upgrade preview: a consumer requirement whose declared `range` does not contain the new `interface_version`.
///
/// Clause: HC A7-1, HP A8-3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BreakingEntry {
    pub consumer: String,
    pub interface: String,
    pub range: String,
    pub interface_version: String,
}

/// The result of `preview_upgrade`.
///
/// Clause: HC A.9 (`package`), HP PM-7, PD, HC A7-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpgradePreview {
    pub old_commit: String,
    pub new_commit: String,
    pub old_hash: String,
    pub new_hash: String,
    pub changed_files: Value,
    pub manifest_diff: Value,
    pub grant_diff: Value,
    /// Always present; empty when nothing breaks (HC A7-1).
    pub breaking: Vec<BreakingEntry>,
}

/// One preview of `hub.preview_package_upgrades`: the package, and the `preview_upgrade` result.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackagePreview {
    pub package: String,
    #[serde(flatten)]
    pub preview: UpgradePreview,
}

/// The result of `hub.preview_package_upgrades`. `breaking` is the list for the whole set.
///
/// Clause: HC A.9, HC A7-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PreviewPackageUpgradesResult {
    pub previews: Vec<PackagePreview>,
    pub ambiguous: Vec<Ambiguity>,
    pub breaking: Vec<BreakingEntry>,
}

/// The result of `reload_config`.
///
/// Clause: HC CF-1, A2 part 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReloadConfigResult {
    pub applied: Vec<String>,
    pub restart_required: Vec<String>,
}

// ---------------------------------------------------------------- session

/// A freeform command spawn.
///
/// Clause: HC A.9 (`session` create), section 6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandSpec {
    pub argv: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub cwd: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub env: Option<BTreeMap<String, String>>,
}

/// A requested session size.
///
/// Clause: HC A.9 (`session` create).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequestedSize {
    pub rows: u32,
    pub cols: u32,
}

/// `create(session, { session_type_id, … })`. `worktree_id` implies its target; a different `target_id` is `invalid_arguments`.
///
/// Clause: HC A.9, A2 part 1 (`agent_scopes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateType {
    pub session_type_id: String,
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
    pub prompt: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub context: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub agent_scopes: Option<Vec<String>>,
}

/// `create(session, { command, … })`: freeform spawn, permission `session.create_command`.
///
/// Clause: HC A.9, section 6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateCommand {
    pub command: CommandSpec,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub size: Option<RequestedSize>,
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
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub agent_scopes: Option<Vec<String>>,
}

/// The two forms of `create(session, …)`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SessionCreate {
    Command(SessionCreateCommand),
    Type(SessionCreateType),
}

/// The word `starting`, the state in the create receipt.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StartingTag {
    Starting,
}

/// The result of `create(session)`: the receipt of a durable accepted operation.
///
/// Clause: HC section 6 (session create), A.9, A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateReceipt {
    pub operation_id: String,
    pub session: SessionRef,
    pub state: StartingTag,
}

/// The merge patch of `update(session)`: `label` and `task`. `null` deletes a member (A.1, VB-4).
///
/// Clause: HC A.9, A.1, KR-1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct SessionPatch {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_or_null"
    )]
    #[schemars(schema_with = "nullable_string")]
    pub label: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_or_null"
    )]
    #[schemars(schema_with = "nullable_string")]
    pub task: Option<Option<String>>,
}

open_enum! {
    /// The history that an attach asks for.
    ///
    /// Clause: HC TS-1, A.9.
    pub enum AttachHistory {
        None = "none",
        Screen = "screen",
        Full = "full",
    }
}

/// Arguments of `attach`.
///
/// Clause: HC TS-1, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttachArgs {
    pub history: AttachHistory,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub replace_route: Option<String>,
    /// Default true.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub answers_queries: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub binding: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub route_features: Option<Vec<String>>,
    /// Whether the route may send input (codec revision 30 `attach` `writable?`, A2 part 2).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub writable: Option<bool>,
}

/// `transport = { binding, … }`: the binding name and that binding's connect information (section 9).
///
/// Clause: HC TS-1, TB-L2, TB-W2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transport {
    pub binding: String,
    /// `local-stream`: the Hub's listener.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub socket_path: Option<String>,
    /// `local-stream`: unguessable, single use.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub route_token: Option<String>,
    /// Other fields of a binding that this version does not know.
    #[serde(flatten)]
    pub extra: OpenObject,
}

/// The result of `attach`.
///
/// Clause: HC TS-1, A.9.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AttachResult {
    pub route_id: String,
    pub transport: Transport,
    pub route_limits: RouteLimits,
}

/// Arguments of `detach`.
///
/// Clause: HC TS-10, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetachArgs {
    pub route_id: String,
}

/// Arguments of `read_screen`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ReadScreenArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub history: Option<bool>,
}

/// The cursor of a read screen.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScreenCursor {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
}

/// The result of `read_screen`. `model_rev` is Core's opaque token, compared for equality only.
///
/// Clause: HC A.9, A2 H-31, HP SA-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadScreenResult {
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub cursor: Option<ScreenCursor>,
    pub model_rev: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub history_unavailable: Option<HistoryUnavailable>,
}

/// Arguments of `safe_write`.
///
/// Clause: HC MC-4, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SafeWriteArgs {
    pub text: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub submit: Option<bool>,
}

/// Arguments of `write`.
///
/// Clause: HC MC-4, A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WriteArgs {
    pub bytes_b64: String,
}

/// The typed result of `write`, as HP RW-2 defines it, tagged by `state`. Each state has its own required counts. The RW-2 refusals are errors with their `effect`.
///
/// Clause: HC A.9 (`write`), HP RW-2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WriteResult {
    /// Every accepted byte was written.
    Written {
        accepted_bytes: u64,
        written_bytes: u64,
    },
    /// The write stopped part of the way; both counts are known.
    Partial {
        accepted_bytes: u64,
        written_bytes: u64,
    },
    /// The outcome is not known; at most this many bytes were written.
    Unknown { max_written_bytes: u64 },
}

// ---------------------------------------------------------------- session_type, spawn_target, worktree

open_enum! {
    /// `lifecycle` of a session type; it sets the row's `restartable` (H-21).
    ///
    /// Clause: HC A2 H-21.
    pub enum Lifecycle {
        Restartable = "restartable",
        OneShot = "one_shot",
    }
}

/// The create fields of a session type, also the result of `definition`. `update` is a merge patch of these.
///
/// Clause: HC A.9, A2 H-21, A2 part 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeDefinition {
    pub id: String,
    pub source: TypeSource,
    pub label: String,
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub lifecycle: Option<Lifecycle>,
    /// An executable path, or a name found on the session's `PATH`.
    pub command: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub traits: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub allowed_environment_overrides: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub context_keys: Option<Vec<String>>,
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
    /// An init-script body that the Hub stores and runs as `command`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub script: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub agent_scopes: Option<Vec<String>>,
}

/// Arguments of `session_type.resolve`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveArgs {
    pub target_id: String,
}

/// A `{ code, message }` diagnostic or problem.
///
/// Clause: HC A.9, A2 H-31.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CodeMessage {
    pub code: String,
    pub message: String,
}

/// The result of `session_type.resolve`.
///
/// Clause: HC A.9, A2 H-31.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveResult {
    pub available: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub diagnostics: Option<Vec<CodeMessage>>,
    pub context_keys: Vec<String>,
}

/// The create and update fields of a spawn target.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpawnTargetInput {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub id: Option<String>,
    pub label: String,
    pub path: String,
    pub kind: TargetKind,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub enabled: Option<bool>,
}

/// The result of `spawn_target.validate`: problems are in the result, not an error. `problems[].code` is non-exhaustive (`path_missing`, `not_a_directory`, `not_git`, `unreadable`).
///
/// Clause: HC A.9, A2 A.9b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidateResult {
    pub valid: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub problems: Option<Vec<CodeMessage>>,
}

/// Arguments of `spawn_target.browse`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BrowseArgs {
    pub path: String,
}

/// One directory entry of `browse`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
}

/// The result of `browse`: directories only.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BrowseResult {
    pub entries: Vec<BrowseEntry>,
}

/// Arguments of `create(worktree)`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeCreateArgs {
    pub target_id: String,
    pub branch: String,
}

/// Arguments of `delete(worktree)`.
///
/// Clause: HC A.9, VB-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeDeleteArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub force: Option<bool>,
}

// ---------------------------------------------------------------- package, catalog

/// Arguments of `create(package)`, the install.
///
/// Clause: HC A.9, A1, HP PM-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageInstallArgs {
    pub source: PackageSource,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub enable: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub bindings: Option<Vec<InterfaceBinding>>,
}

/// Arguments of `delete(package)`, the removal.
///
/// Clause: HC A.9, VB-5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct PackageRemoveArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub purge: Option<bool>,
}

/// Arguments of `package.upgrade`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct PackageUpgradeArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub bindings: Option<Vec<InterfaceBinding>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub accept_breaking: Option<Vec<ConsumerInterface>>,
}

/// Arguments of `package.pin`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PinArgs {
    #[serde(rename = "ref")]
    pub git_ref: String,
}

/// Arguments of `package.approve_grants`: exactly the pending grants.
///
/// Clause: HC A.9, HP PM-8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApproveGrantsArgs {
    pub grants: Vec<Value>,
}

/// The merge patch of `update(package)`.
///
/// Clause: HC A.9, A2 part 1 (`webhook_secret`, SF-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackagePatch {
    pub configuration: OpenObject,
}

/// Arguments of `package.enable`: the filesystem root bindings.
///
/// Clause: HC A2 part 1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct EnableArgs {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub fs_roots: Option<BTreeMap<String, String>>,
}

/// Arguments of an entrypoint action.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntrypointArgs {
    pub entrypoint_id: String,
}

/// Arguments of `package.open_asset`: exactly one of `asset_id` and `name`. Both or neither is `invalid_arguments` (A2 A.9b).
///
/// Clause: HC A.9, A2 A.9b, HP AS-1 to AS-4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum OpenAssetArgs {
    ById { asset_id: String },
    ByName { name: String },
}

impl<'de> Deserialize<'de> for OpenAssetArgs {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default, deserialize_with = "opt_no_null")]
            asset_id: Option<String>,
            #[serde(default, deserialize_with = "opt_no_null")]
            name: Option<String>,
        }
        match Raw::deserialize(d)? {
            Raw {
                asset_id: Some(asset_id),
                name: None,
            } => Ok(OpenAssetArgs::ById { asset_id }),
            Raw {
                asset_id: None,
                name: Some(name),
            } => Ok(OpenAssetArgs::ByName { name }),
            _ => Err(serde::de::Error::custom("exactly one of asset_id and name")),
        }
    }
}

impl JsonSchema for OpenAssetArgs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "OpenAssetArgs".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "object",
            "properties": { "asset_id": { "type": "string" }, "name": { "type": "string" } },
            "oneOf": [
                { "required": ["asset_id"], "not": { "required": ["name"] } },
                { "required": ["name"], "not": { "required": ["asset_id"] } }
            ]
        })
    }
}

/// The result of `open_asset`: a bearer capability URL. `expires_at` is Unix seconds.
///
/// Clause: HC A.9, UI-EM-7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenAssetResult {
    pub url: String,
    pub expires_at: u64,
}

/// Arguments of `create(catalog)`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogCreateArgs {
    pub url: String,
}

// ---------------------------------------------------------------- the rest

open_enum! {
    /// The decision of `quarantine.resolve`.
    ///
    /// Clause: HC A.9.
    pub enum Decision {
        Release = "release",
        Remove = "remove",
    }
}

/// Arguments of `quarantine.resolve`.
///
/// Clause: HC A.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveDecisionArgs {
    pub decision: Decision,
}

/// Arguments of `navigation.resolve_route`: the part after `/hubs/<hub_id>`.
///
/// Clause: HC A.9, A2 G-17.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveRouteArgs {
    pub route_path: String,
}

/// The result of `resolve_route`.
///
/// Clause: HC A.9, A2 G-17.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveRouteResult {
    pub target: NavTarget,
}

/// The Web Push keys.
///
/// Clause: HC SF-5b, A2 H-15.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushKeys {
    pub p256dh: String,
    pub auth: String,
}

/// Arguments of `create(push_subscription)`. The endpoint is never returned.
///
/// Clause: HC SF-5b, A2 H-15.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushSubscriptionCreateArgs {
    pub endpoint: String,
    pub keys: PushKeys,
}

// ---------------------------------------------------------------- operation results (A.7b)

/// `result` of a `hub.update` operation.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateResult {
    pub version: String,
}

/// The `state` values of a [`PackageOutcome`].
///
/// Clause: HC A.7b, A2 A.7b.
pub const PACKAGE_OUTCOME_STATES: &[&str] = &["succeeded", "failed"];

/// One package outcome of a bulk package operation.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageOutcome {
    pub package: String,
    /// `succeeded` or `failed`.
    pub state: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub reason: Option<String>,
}

/// `result` of `hub.upgrade_packages` and `hub.rollback_packages`.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BulkPackagesResult {
    pub packages: Vec<PackageOutcome>,
}

/// `result` of `package.install`: the package key and the admitted manifest version, on every end that admitted a package.
///
/// Clause: HC A.7b, HC A6-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageInstallResult {
    pub package: String,
    pub version: String,
}

/// `result` of `package.upgrade`.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageUpgradeResult {
    pub package: String,
    pub version: String,
    pub resolved_commit: String,
}

/// `result` of `package.rollback`.
///
/// Clause: HC A.7b.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackageRollbackResult {
    pub package: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub resolved_commit: String,
}

/// `result` of `spawn_target.rescan_packages`: the instances found and removed.
// pending steward ruling H3: the layout of the rescan result (`found` and `removed` in `row.result`).
///
/// Clause: HC A.7b, A2 part 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RescanResult {
    pub found: Vec<String>,
    pub removed: Vec<String>,
}

/// `whoami {}` of the MCP binding.
///
/// Clause: HC MC-2, A2 H-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiResult {
    pub caller: SessionRef,
    pub scopes: Vec<String>,
}
