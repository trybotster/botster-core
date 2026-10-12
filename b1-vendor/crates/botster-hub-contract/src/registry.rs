//! The registry of the Hub-owned kinds: for each kind, its verbs, actions, effects and the types of their arguments and results.
//!
//! This is the data that `describe` reports for a Hub kind (DS-1) and that `schemas/hub-client/kinds/<kind>.schema.json` carries. It holds no Hub logic.
//! A permission name is listed only where the contract names it. Permissions that HC leaves unnamed are `None`.

use crate::actions::*;
use crate::describe::EffectClass::{self, Accepted, Committed, Immediate, Read};
use crate::rows::*;
use schemars::{JsonSchema, Schema, SchemaGenerator};

/// Registers a type in the generator and returns its schema (a `$ref` for a struct).
pub type SchemaFn = fn(&mut SchemaGenerator) -> Schema;

fn of<T: JsonSchema>(g: &mut SchemaGenerator) -> Schema {
    g.subschema_for::<T>()
}

/// A verb of a kind.
///
/// Clause: HC section 6, A.7, DS-1.
pub struct VerbSpec {
    pub verb: &'static str,
    pub effect: EffectClass,
    pub permission: Option<&'static str>,
    /// The `args` of the request, when the verb takes a typed value (`create`, `update`, `delete`).
    pub args: Option<SchemaFn>,
    /// The `ok` value, when it is not the row (`create` that answers a receipt).
    pub result: Option<SchemaFn>,
    /// The specific failures (A.9b): `code` or `code:reason`.
    pub reasons: &'static [&'static str],
    pub clause: &'static str,
}

impl VerbSpec {
    /// The verb with its A.9b failures.
    const fn fails(self, reasons: &'static [&'static str]) -> VerbSpec {
        VerbSpec { reasons, ..self }
    }
}

/// An action of a kind.
///
/// Clause: HC A.7, A.9, A.9b, DS-1.
pub struct ActionSpec {
    pub name: &'static str,
    pub effect: EffectClass,
    pub durable_operation: bool,
    pub permission: Option<&'static str>,
    /// The specific failures (A.9b): `code` or `code:reason`.
    pub reasons: &'static [&'static str],
    pub args: SchemaFn,
    pub result: SchemaFn,
    pub clause: &'static str,
}

/// A Hub-owned kind.
///
/// Clause: HC section 6, A.7, A.9, DS-1.
pub struct KindSpec {
    pub kind: &'static str,
    /// `"M.m"` (section 12, Versioning).
    pub schema_version: &'static str,
    pub core_fields: &'static [&'static str],
    pub verbs: &'static [VerbSpec],
    pub actions: &'static [ActionSpec],
    pub row: SchemaFn,
    pub clause: &'static str,
}

impl KindSpec {
    /// Whether the kind has `watch`.
    ///
    /// Clause: HC section 6, DS-1.
    pub fn watchable(&self) -> bool {
        self.verbs.iter().any(|v| v.verb == "watch")
    }
}

const fn v(verb: &'static str, effect: EffectClass, clause: &'static str) -> VerbSpec {
    VerbSpec {
        verb,
        effect,
        permission: None,
        args: None,
        result: None,
        reasons: &[],
        clause,
    }
}

const fn vt(
    verb: &'static str,
    effect: EffectClass,
    args: Option<SchemaFn>,
    result: Option<SchemaFn>,
    clause: &'static str,
) -> VerbSpec {
    VerbSpec {
        verb,
        effect,
        permission: None,
        args,
        result,
        reasons: &[],
        clause,
    }
}

macro_rules! act {
    ($name:literal, $effect:expr, $durable:expr, $perm:expr, [$($r:literal),*], $args:ty, $res:ty, $clause:literal) => {
        ActionSpec { name: $name, effect: $effect, durable_operation: $durable, permission: $perm, reasons: &[$($r),*], args: of::<$args>, result: of::<$res>, clause: $clause }
    };
}

const READS: [VerbSpec; 3] = [
    v("list", Read, "VB-1"),
    v("get", Read, "VB-2"),
    v("watch", Read, "WT-1"),
];

const HUB: KindSpec = KindSpec {
    kind: "hub",
    schema_version: "1.0",
    core_fields: &[
        "id",
        "software",
        "worker_protocol",
        "components",
        "pid",
        "uptime_seconds",
        "data_root",
        "sessions",
        "plugins",
        "update",
        "push_public_key",
        "web_origin",
        "state",
    ],
    verbs: &[v("get", Read, "VB-2"), v("watch", Read, "WT-1")],
    actions: &[
        act!(
            "check_update",
            Read,
            false,
            None,
            [],
            Empty,
            CheckUpdateResult,
            "A.7, A.9, H-3"
        ),
        act!(
            "update",
            Accepted,
            true,
            None,
            [
                "wrong_state:up_to_date",
                "wrong_state:offline",
                "wrong_state:dev_build",
                "wrong_state:update_in_progress",
                "wrong_state:swap_in_progress"
            ],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "shutdown",
            Accepted,
            false,
            None,
            [
                "wrong_state:swap_in_progress",
                "wrong_state:update_in_progress",
                "wrong_state:package_operation_in_progress"
            ],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "restart",
            Accepted,
            false,
            None,
            [
                "wrong_state:swap_in_progress",
                "wrong_state:update_in_progress",
                "wrong_state:package_operation_in_progress"
            ],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "issue_pairing",
            Immediate,
            false,
            Some("hub.pair"),
            [
                "wrong_state:no_web_origin",
                "invalid_arguments:ttl_out_of_range"
            ],
            IssuePairingArgs,
            IssuePairingResult,
            "PR-0, A.7, A.9b"
        ),
        act!(
            "diagnostics",
            Read,
            false,
            Some("hub.diagnostics"),
            [],
            Empty,
            DiagnosticsResult,
            "A.7, A.9"
        ),
        act!(
            "swap_preflight",
            Read,
            false,
            None,
            ["invalid_arguments:unknown_worker_protocol"],
            SwapPreflightArgs,
            SwapPreflightResult,
            "A.7, A.9, A.9b"
        ),
        act!(
            "swap",
            Accepted,
            false,
            None,
            [
                "wrong_state:unadoptable_sessions",
                "wrong_state:swap_in_progress",
                "wrong_state:update_in_progress",
                "wrong_state:package_operation_in_progress",
                "invalid_arguments:unknown_worker_protocol"
            ],
            SwapArgs,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "preview_package_upgrades",
            Read,
            false,
            None,
            [],
            Empty,
            PreviewPackageUpgradesResult,
            "A.7, A.9"
        ),
        act!(
            "upgrade_packages",
            Accepted,
            true,
            None,
            [
                "conflict:requirement_conflict",
                "conflict:ambiguous_provider",
                "conflict:requirement_missing",
                "conflict:breaking_not_accepted",
                "wrong_state:package_operation_in_progress"
            ],
            UpgradePackagesArgs,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "rollback_packages",
            Accepted,
            true,
            None,
            [
                "wrong_state:no_history",
                "wrong_state:package_operation_in_progress"
            ],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "reload_config",
            Committed,
            false,
            None,
            ["invalid_arguments:config_invalid"],
            Empty,
            ReloadConfigResult,
            "CF-1, A.9b"
        ),
    ],
    row: of::<HubRow>,
    clause: "section 6 (hub)",
};

const SESSION: KindSpec = KindSpec {
    kind: "session",
    schema_version: "1.0",
    core_fields: &[
        "id",
        "hub_id",
        "label",
        "state",
        "lifecycle_class",
        "exit",
        "failure_reason",
        "size",
        "session_type",
        "role",
        "traits",
        "interaction",
        "restartable",
        "port",
        "owner",
        "worktree_id",
        "branch",
        "title",
        "task",
        "activity",
        "attention",
        "presentation",
        "metadata",
    ],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        VerbSpec {
            verb: "update",
            effect: Committed,
            permission: Some("session.update"),
            args: Some(of::<SessionPatch>),
            result: None,
            reasons: &["conflict", "invalid_arguments"],
            clause: "A.7, A.9, KR-1",
        },
        v("delete", Committed, "A.7, A.2c").fails(&["wrong_state"]),
        vt(
            "create",
            Accepted,
            Some(of::<SessionCreate>),
            Some(of::<SessionCreateReceipt>),
            "A.7, A.9, section 6 (session create)",
        )
        .fails(&[
            "wrong_state:swap_in_progress",
            "internal:credential_unavailable",
            "invalid_arguments:target_mismatch",
            "wrong_state:session_type_unavailable",
            "invalid_arguments:scope_widens",
            "permission_denied",
            "invalid_arguments:argv_empty",
            "invalid_arguments:cwd_not_allowed",
        ]),
    ],
    actions: &[
        act!(
            "attach",
            Immediate,
            false,
            None,
            [
                "unsupported:binding_auth_unavailable",
                "unsupported:terminal_format",
                "wrong_state:session_not_running",
                "not_found:route",
                "unsupported:binding",
                "busy"
            ],
            AttachArgs,
            AttachResult,
            "TS-1, A.7, A.9b"
        ),
        act!(
            "detach",
            Immediate,
            false,
            None,
            ["not_found:route"],
            DetachArgs,
            Empty,
            "TS-10, A.7, A.9b"
        ),
        act!(
            "stop",
            Accepted,
            false,
            None,
            ["wrong_state:not_running"],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "restart",
            Accepted,
            true,
            None,
            [
                "wrong_state:not_ended",
                "wrong_state:not_restartable",
                "wrong_state:record_unavailable",
                "wrong_state:environment_not_retained",
                "wrong_state:not_ready",
                "wrong_state:swap_in_progress"
            ],
            Empty,
            Receipt,
            "section 6 (restart), A.7, A.9b"
        ),
        act!(
            "read_screen",
            Read,
            false,
            None,
            ["wrong_state:no_screen_model"],
            ReadScreenArgs,
            ReadScreenResult,
            "A.7, A.9, A.9b"
        ),
        act!(
            "clear_attention",
            Committed,
            false,
            None,
            [],
            Empty,
            Empty,
            "SF-5d, A.7, A.9b"
        ),
        act!(
            "safe_write",
            Accepted,
            false,
            Some("session.safe_write"),
            [
                "wrong_state:session_ended",
                "invalid_arguments:text_too_long",
                "invalid_arguments:text_empty",
                "busy"
            ],
            SafeWriteArgs,
            Receipt,
            "MC-4, A.7, A.9b"
        ),
        act!(
            "write",
            Immediate,
            false,
            Some("session.write"),
            [
                "wrong_state:session_ended",
                "busy",
                "invalid_arguments:bytes_too_long",
                "invalid_arguments:bytes_empty",
                "invalid_arguments:bytes_invalid"
            ],
            WriteArgs,
            WriteResult,
            "MC-4, A.7, A.9b"
        ),
    ],
    row: of::<SessionRow>,
    clause: "section 6 (session)",
};

const SESSION_TYPE: KindSpec = KindSpec {
    kind: "session_type",
    schema_version: "1.0",
    core_fields: &[
        "id",
        "source",
        "label",
        "description",
        "editable",
        "targets",
        "port_env",
        "notifications",
    ],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        vt(
            "create",
            Committed,
            Some(of::<SessionTypeDefinition>),
            None,
            "A.7, A.9",
        )
        .fails(&[
            "conflict",
            "invalid_arguments:port_env_invalid",
            "invalid_arguments:scope_widens",
            "unsupported:repo_source_is_discovered",
        ]),
        vt("update", Committed, None, None, "A.7, A.9").fails(&[
            "invalid_arguments:port_env_invalid",
            "invalid_arguments:scope_widens",
            "wrong_state:not_editable",
            "unsupported:repo_source_is_discovered",
        ]),
        v("delete", Committed, "A.7").fails(&["wrong_state:not_editable"]),
    ],
    actions: &[
        act!(
            "resolve",
            Read,
            false,
            None,
            [],
            ResolveArgs,
            ResolveResult,
            "A.7, A.9"
        ),
        act!(
            "definition",
            Read,
            false,
            None,
            [],
            Empty,
            SessionTypeDefinition,
            "A.7, A.9"
        ),
    ],
    row: of::<SessionTypeRow>,
    clause: "section 6 (session_type)",
};

const SPAWN_TARGET: KindSpec = KindSpec {
    kind: "spawn_target",
    schema_version: "1.0",
    core_fields: &["id", "label", "path", "kind", "git"],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        vt(
            "create",
            Committed,
            Some(of::<SpawnTargetInput>),
            None,
            "A.7, A.9",
        )
        .fails(&[
            "invalid_arguments:path_not_absolute",
            "invalid_arguments:path_not_found",
            "invalid_arguments:not_a_directory",
            "conflict:path_exists",
        ]),
        vt("update", Committed, None, None, "A.7, A.9").fails(&[
            "invalid_arguments:path_not_absolute",
            "invalid_arguments:path_not_found",
            "invalid_arguments:not_a_directory",
            "conflict:path_exists",
        ]),
        v("delete", Committed, "A.7").fails(&["wrong_state:in_use"]),
    ],
    actions: &[
        act!(
            "validate",
            Read,
            false,
            None,
            [],
            Empty,
            ValidateResult,
            "A.7, A.9"
        ),
        act!(
            "browse",
            Read,
            false,
            Some("spawn_target.browse"),
            [
                "invalid_arguments:path_not_absolute",
                "not_found:path",
                "wrong_state:not_a_directory",
                "wrong_state:unreadable"
            ],
            BrowseArgs,
            BrowseResult,
            "A.7, A.9, A.9b"
        ),
        act!(
            "rescan_packages",
            Accepted,
            false,
            None,
            ["wrong_state:not_a_directory"],
            Empty,
            Receipt,
            "A2 part 1, A.9b"
        ),
    ],
    row: of::<SpawnTargetRow>,
    clause: "section 6 (spawn_target)",
};

const WORKTREE: KindSpec = KindSpec {
    kind: "worktree",
    schema_version: "1.0",
    core_fields: &[
        "id",
        "target_id",
        "path",
        "branch",
        "head",
        "state",
        "active_sessions",
    ],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        vt(
            "create",
            Accepted,
            Some(of::<WorktreeCreateArgs>),
            Some(of::<Receipt>),
            "A.7, A.9",
        )
        .fails(&["wrong_state:not_git", "conflict:branch_checked_out"]),
        vt(
            "delete",
            Accepted,
            Some(of::<WorktreeDeleteArgs>),
            Some(of::<Receipt>),
            "A.7, A.9, A.9b",
        )
        .fails(&["wrong_state:dirty", "wrong_state:occupied"]),
    ],
    actions: &[],
    row: of::<WorktreeRow>,
    clause: "section 6 (worktree)",
};

const PACKAGE: KindSpec = KindSpec {
    kind: "package",
    schema_version: "1.0",
    core_fields: &[
        "id",
        "name",
        "kind",
        "state",
        "status",
        "provides",
        "requires",
        "version",
        "source",
        "lock",
        "pinned_ref",
        "pending_grants",
        "configuration",
        "entrypoints",
        "notice_reactions",
        "target_id",
        "static_origin",
    ],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        VerbSpec {
            verb: "create",
            effect: Accepted,
            permission: Some("package.manage"),
            args: Some(of::<PackageInstallArgs>),
            result: Some(of::<Receipt>),
            reasons: &[
                "invalid_arguments:path_invalid",
                "conflict:already_installed",
            ],
            clause: "A.7, A.9",
        },
        VerbSpec {
            verb: "update",
            effect: Committed,
            permission: Some("package.manage"),
            args: Some(of::<PackagePatch>),
            result: None,
            reasons: &["invalid_arguments"],
            clause: "A.7, A.9",
        },
        VerbSpec {
            verb: "delete",
            effect: Accepted,
            permission: Some("package.manage"),
            args: Some(of::<PackageRemoveArgs>),
            result: Some(of::<Receipt>),
            reasons: &[],
            clause: "A.7, A.9",
        },
    ],
    actions: &[
        act!(
            "enable",
            Accepted,
            false,
            Some("package.manage"),
            [
                "wrong_state:staged",
                "wrong_state:quarantined",
                "wrong_state:errored",
                "wrong_state:fs_root_unbound",
                "invalid_arguments:fs_root_unknown",
                "invalid_arguments:path_not_absolute",
                "invalid_arguments:path_not_found"
            ],
            EnableArgs,
            Receipt,
            "A.7, A2 part 1, A.9b"
        ),
        act!(
            "disable",
            Accepted,
            false,
            Some("package.manage"),
            [],
            Empty,
            Receipt,
            "A.7, A.9b"
        ),
        act!(
            "reload",
            Accepted,
            false,
            Some("package.manage"),
            ["wrong_state:disabled"],
            Empty,
            Receipt,
            "A.7, A.9b"
        ),
        act!(
            "start_entrypoint",
            Accepted,
            false,
            Some("package.manage"),
            ["wrong_state:disabled"],
            EntrypointArgs,
            Receipt,
            "A.7, A.9b"
        ),
        act!(
            "stop_entrypoint",
            Accepted,
            false,
            Some("package.manage"),
            ["wrong_state:disabled"],
            EntrypointArgs,
            Receipt,
            "A.7, A.9b"
        ),
        act!(
            "restart_entrypoint",
            Accepted,
            false,
            Some("package.manage"),
            ["wrong_state:disabled"],
            EntrypointArgs,
            Receipt,
            "A.7, A.9b"
        ),
        act!(
            "preview_upgrade",
            Read,
            false,
            Some("package.manage"),
            ["wrong_state:local_source", "wrong_state:fetch_failed"],
            Empty,
            UpgradePreview,
            "A.7, A.9, A.9b"
        ),
        act!(
            "upgrade",
            Accepted,
            true,
            Some("package.manage"),
            [
                "conflict:breaking_not_accepted",
                "conflict:ambiguous_provider",
                "conflict:requirement_missing",
                "conflict:requirement_conflict",
                "wrong_state:pinned",
                "wrong_state:local_source",
                "wrong_state:up_to_date",
                "wrong_state:package_operation_in_progress"
            ],
            PackageUpgradeArgs,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "pin",
            Committed,
            false,
            Some("package.manage"),
            ["wrong_state:local_source"],
            PinArgs,
            PackageRow,
            "A.7, A.9, A.9b"
        ),
        act!(
            "unpin",
            Committed,
            false,
            Some("package.manage"),
            [],
            Empty,
            PackageRow,
            "A.7, A.9, A.9b"
        ),
        act!(
            "rollback",
            Accepted,
            true,
            Some("package.manage"),
            ["wrong_state:no_history", "wrong_state:local_source"],
            Empty,
            Receipt,
            "A.7, A.9, A.9b"
        ),
        act!(
            "approve_grants",
            Committed,
            false,
            Some("package.manage"),
            ["conflict:grants_mismatch", "wrong_state:nothing_pending"],
            ApproveGrantsArgs,
            PackageRow,
            "A.7, A.9, A.9b"
        ),
        act!(
            "open_asset",
            Immediate,
            false,
            None,
            [
                "not_found",
                "wrong_state:assets_origin_unconfigured",
                "invalid_arguments"
            ],
            OpenAssetArgs,
            OpenAssetResult,
            "A.7, A.9, A.9b"
        ),
    ],
    row: of::<PackageRow>,
    clause: "section 6 (package)",
};

const CATALOG: KindSpec = KindSpec {
    kind: "catalog",
    schema_version: "1.0",
    core_fields: &["id", "url", "refreshed_at", "entries"],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        VerbSpec {
            verb: "create",
            effect: Committed,
            permission: Some("catalog.manage"),
            args: Some(of::<CatalogCreateArgs>),
            result: None,
            reasons: &["invalid_arguments:url_not_https", "conflict"],
            clause: "A.7, A.9, A.9b",
        },
        VerbSpec {
            verb: "delete",
            effect: Committed,
            permission: Some("catalog.manage"),
            args: None,
            result: None,
            reasons: &[],
            clause: "A.7",
        },
    ],
    actions: &[act!(
        "refresh",
        Accepted,
        false,
        Some("catalog.manage"),
        [],
        Empty,
        Receipt,
        "A.7, A.9"
    )],
    row: of::<CatalogRow>,
    clause: "section 6 (catalog)",
};

const QUARANTINE: KindSpec = KindSpec {
    kind: "quarantine",
    schema_version: "1.0",
    core_fields: &["id", "package", "reason", "since"],
    verbs: &READS,
    actions: &[act!(
        "resolve",
        Committed,
        false,
        None,
        [],
        ResolveDecisionArgs,
        Empty,
        "A.7, A.9, A.9b"
    )],
    row: of::<QuarantineRow>,
    clause: "section 6 (quarantine)",
};

const NAVIGATION: KindSpec = KindSpec {
    kind: "navigation",
    schema_version: "1.0",
    core_fields: &["id", "package", "label", "icon", "order", "target", "badge"],
    verbs: &[v("list", Read, "VB-1"), v("watch", Read, "WT-1")],
    actions: &[act!(
        "resolve_route",
        Read,
        false,
        None,
        ["not_found"],
        ResolveRouteArgs,
        ResolveRouteResult,
        "A.7, A.9, A.9b"
    )],
    row: of::<NavigationRow>,
    clause: "section 6 (navigation)",
};

const VIEW: KindSpec = KindSpec {
    kind: "view",
    schema_version: "1.0",
    core_fields: &["id", "component", "binds", "actions", "placement"],
    verbs: &READS,
    actions: &[],
    row: of::<ViewRow>,
    clause: "section 6 (view)",
};

const PLUGIN_LOG: KindSpec = KindSpec {
    kind: "plugin_log",
    schema_version: "1.0",
    core_fields: &[
        "id", "package", "load_id", "at", "level", "message", "fields",
    ],
    verbs: &[v("list", Read, "VB-1"), v("watch", Read, "WT-1")],
    actions: &[],
    row: of::<PluginLogRow>,
    clause: "section 6 (plugin_log)",
};

const OPERATION: KindSpec = KindSpec {
    kind: "operation",
    schema_version: "1.0",
    core_fields: &[
        "id", "kind", "state", "terminal", "progress", "result", "error",
    ],
    verbs: &[v("get", Read, "VB-2"), v("watch", Read, "WT-1")],
    actions: &[],
    row: of::<OperationRow>,
    clause: "section 6 (operation)",
};

const CLIENT_CREDENTIAL: KindSpec = KindSpec {
    kind: "client_credential",
    schema_version: "1.0",
    core_fields: &["id", "created_at", "last_seen_at"],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        v("delete", Committed, "A.7, PR-8"),
    ],
    actions: &[],
    row: of::<ClientCredentialRow>,
    clause: "section 6 (client_credential)",
};

const PUSH_SUBSCRIPTION: KindSpec = KindSpec {
    kind: "push_subscription",
    schema_version: "1.0",
    core_fields: &["id", "credential_id", "created_at"],
    verbs: &[
        v("list", Read, "VB-1"),
        vt(
            "create",
            Committed,
            Some(of::<PushSubscriptionCreateArgs>),
            None,
            "A2 H-15",
        )
        .fails(&["invalid_arguments:endpoint_not_https", "conflict"]),
        v("delete", Committed, "A2 H-15").fails(&["not_found"]),
    ],
    actions: &[],
    row: of::<PushSubscriptionRow>,
    clause: "section 6 (push_subscription)",
};

const ROUTE: KindSpec = KindSpec {
    kind: "route",
    schema_version: "1.0",
    core_fields: &["id", "session", "binding", "since"],
    verbs: &[
        v("list", Read, "VB-1"),
        v("get", Read, "VB-2"),
        v("watch", Read, "WT-1"),
        v("delete", Immediate, "A.7, A.3").fails(&["not_found"]),
    ],
    actions: &[],
    row: of::<RouteRow>,
    clause: "section 6 (route)",
};

const KIND: KindSpec = KindSpec {
    kind: "kind",
    schema_version: "1.0",
    core_fields: &[
        "kind",
        "owner",
        "descriptor_id",
        "schema_version",
        "schema",
        "core_fields",
        "verbs",
        "actions",
        "watchable",
        "durable",
        "projection_allowlist",
    ],
    verbs: &[v("list", Read, "VB-1"), v("watch", Read, "WT-1")],
    actions: &[],
    row: of::<KindRow>,
    clause: "section 6 (kind), DS-3",
};

/// Reads a `result` value as its A.7b type.
pub type ResultCheck = fn(&serde_json::Value) -> Result<(), serde_json::Error>;

/// The state and result shape of one operation kind (A.7b).
///
/// Clause: HC A.7b.
pub struct OperationKindSpec {
    /// The value of `operation.kind`.
    pub kind: &'static str,
    /// The states that this version knows. `state` stays non-exhaustive (EN-6), and `terminal` says whether to keep watching.
    pub states: &'static [&'static str],
    /// The `failed.reason` values that A.7b lists (non-exhaustive). A kind whose reasons another contract owns lists none.
    pub failed_reasons: &'static [&'static str],
    /// The type of `result`, when A.7b names one.
    pub result: Option<SchemaFn>,
    /// Whether a `result` value reads as that type.
    pub check_result: Option<ResultCheck>,
    pub clause: &'static str,
}

fn check<T: serde::de::DeserializeOwned>(v: &serde_json::Value) -> Result<(), serde_json::Error> {
    serde_json::from_value::<T>(v.clone()).map(|_| ())
}

const GENERIC: &[&str] = &["queued", "running", "succeeded", "failed", "cancelled"];
const SPAWN: &[&str] = &["starting", "installed", "failed", "cancelled"];
const SAFE_WRITE: &[&str] = &[
    "queued",
    "typing",
    "delivered",
    "abandoned",
    "replaced",
    "cancelled",
    "failed",
    "unknown",
];

const fn opk(
    kind: &'static str,
    states: &'static [&'static str],
    failed_reasons: &'static [&'static str],
) -> OperationKindSpec {
    OperationKindSpec {
        kind,
        states,
        failed_reasons,
        result: None,
        check_result: None,
        clause: "A.7b",
    }
}

macro_rules! opk_result {
    ($kind:literal, $reasons:expr, $t:ty) => {
        OperationKindSpec {
            kind: $kind,
            states: GENERIC,
            failed_reasons: $reasons,
            result: Some(of::<$t>),
            check_result: Some(check::<$t>),
            clause: "A.7b",
        }
    };
}

/// The operation kinds of A.7b, each with its states, its `failed.reason` set and its `result` type.
///
/// Clause: HC A.7b.
pub const OPERATION_KINDS: &[OperationKindSpec] = &[
    OperationKindSpec {
        kind: "spawn",
        states: SPAWN,
        failed_reasons: &[
            "exec_failed",
            "cwd_missing",
            "worktree_failed",
            "worker_failed",
            "session_type_unavailable",
        ],
        result: Some(of::<crate::refs::SessionRef>),
        check_result: Some(check::<crate::refs::SessionRef>),
        clause: "A.7b",
    },
    // The states, reasons and result of a safe write are HP SW-2's.
    opk("safe_write", SAFE_WRITE, &[]),
    opk_result!(
        "hub.update",
        &["download_failed", "verify_failed", "install_failed"],
        UpdateResult
    ),
    // Ended by `close { hub_shutdown, lock_released }` (HS-9): a client never waits for a terminal state.
    opk("hub.shutdown", GENERIC, &[]),
    opk("hub.restart", GENERIC, &[]),
    opk("hub.swap", GENERIC, &[]),
    opk_result!("hub.upgrade_packages", &[], BulkPackagesResult),
    opk_result!("hub.rollback_packages", &[], BulkPackagesResult),
    opk("session.stop", GENERIC, &[]),
    opk("worktree.create", GENERIC, &["git_failed"]),
    opk("worktree.delete", GENERIC, &[]),
    // The install reasons of HP PM-4, PD-6 and PK-3 come with HP; these are the ones that HC adds (PKC-1).
    OperationKindSpec {
        kind: "package.install",
        states: GENERIC,
        failed_reasons: &[
            "fetch_failed",
            "url_refused",
            "too_large",
            "hash_mismatch",
            "path_not_found",
            "path_not_contained",
        ],
        result: Some(of::<PackageInstallResult>),
        check_result: Some(check::<PackageInstallResult>),
        clause: "A.7b, A6-1",
    },
    opk("package.remove", GENERIC, &[]),
    opk("package.enable", GENERIC, &[]),
    opk("package.disable", GENERIC, &[]),
    opk("package.reload", GENERIC, &[]),
    opk("package.entrypoint", GENERIC, &[]),
    opk_result!(
        "package.upgrade",
        &["fetch_failed", "hash_mismatch", "activation_failed"],
        PackageUpgradeResult
    ),
    opk_result!(
        "package.rollback",
        &["hash_mismatch"],
        PackageRollbackResult
    ),
    opk(
        "catalog.refresh",
        GENERIC,
        &["fetch_failed", "url_refused", "malformed", "too_large"],
    ),
    // pending steward ruling H3: `found` and `removed` are the members of `row.result` of the rescan operation.
    opk_result!("spawn_target.rescan_packages", &[], RescanResult),
];

/// The spec of an operation kind by name. An unknown kind has none: it is opaque (EN-6).
///
/// Clause: HC A.7b, EN-6.
pub fn operation_kind(name: &str) -> Option<&'static OperationKindSpec> {
    OPERATION_KINDS.iter().find(|k| k.kind == name)
}

/// The range filters that a Hub kind declares in `describe` (`KindDescriptor::range_filters`), beside the equality `filter` of
/// `list`, as `(kind, filter)` pairs.
///
/// Clause: HC VB-1, section 6 (`plugin_log`).
pub const RANGE_FILTERS: &[(&str, &str)] = &[("plugin_log", "after")];

/// The range filters of one Hub kind (VB-1). A kind with none has an empty list.
///
/// Clause: HC VB-1.
pub fn range_filters(kind: &str) -> Vec<&'static str> {
    RANGE_FILTERS
        .iter()
        .filter(|(k, _)| *k == kind)
        .map(|(_, f)| *f)
        .collect()
}

/// Every Hub-owned kind of HC section 6 (base and A1 to A4; A2 adds actions and fields, no kind).
///
/// Clause: HC section 6, A2.
pub const KINDS: &[KindSpec] = &[
    HUB,
    SESSION,
    SESSION_TYPE,
    SPAWN_TARGET,
    WORKTREE,
    PACKAGE,
    CATALOG,
    QUARANTINE,
    NAVIGATION,
    VIEW,
    PLUGIN_LOG,
    OPERATION,
    CLIENT_CREDENTIAL,
    PUSH_SUBSCRIPTION,
    ROUTE,
    KIND,
];

/// The spec of a Hub kind by name.
///
/// Clause: HC section 6.
pub fn kind(name: &str) -> Option<&'static KindSpec> {
    KINDS.iter().find(|k| k.kind == name)
}
