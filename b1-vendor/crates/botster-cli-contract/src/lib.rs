//! The names of the `botster` CLI surface: commands, flags, context keys, `config` keys, `status` fields, development checkout keys,
//! message texts and paths.
//!
//! Source: `cli-spec-v1-candidate34.md` (frozen). Each const cites the section or the clause that it mirrors, and
//! `tests/frozen.rs` checks each one against the frozen text. The crate holds data only. A live name and a dropped or gone name are
//! in separate consts, so a dropped name never reads as a live one.

#![forbid(unsafe_code)]

/// A flag of a command. `command` is the full command path, or `botster` (the binary, CLI-G1) for the global flags. `source` is `"2"` for a flag of the
/// section 2 table, or the clause that names the flag when the table does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    pub command: &'static str,
    pub flag: &'static str,
    pub source: &'static str,
}

/// A text of the spec: the code span number `ordinal` (1-based) of the row of `clause`, with its placeholders kept as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecText {
    pub clause: &'static str,
    pub ordinal: usize,
    pub text: &'static str,
}

/// The name of the binary (CLI-G1). It is also the command of the global flags.
pub const BINARY: &str = "botster";

/// The v1 commands, by full path (section 2 table, with the groups `debug`, `plugin` and `plugin catalog`, and the alias
/// `pairing-url`).
pub const COMMANDS: &[&str] = &[
    "start",
    "attach",
    "status",
    "context",
    "context file",
    "mcp-serve",
    "update",
    "get-connection-url",
    "pairing-url",
    "list-worktrees",
    "delete-worktree",
    "json-get",
    "json-set",
    "json-delete",
    "debug",
    "debug memory",
    "debug hub",
    "debug vt-replay",
    "config",
    "dev-rebuild",
    "plugin",
    "plugin install",
    "plugin update",
    "plugin status",
    "plugin pin",
    "plugin rollback",
    "plugin approve",
    "plugin remove",
    "plugin enable",
    "plugin disable",
    "plugin catalog",
    "plugin catalog add",
    "plugin catalog remove",
    "plugin catalog list",
    "plugin catalog refresh",
];

/// A command that the spec declares gone, with the rows that say so (a section 10 parity row, a section 11 deviation row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoneCommand {
    pub command: &'static str,
    pub clauses: &'static [&'static str],
}

/// The gone commands: `session` (the section 2 GONE row, CL-131) and `reset` (ruling UD-2: CL-118..120, D-15).
pub const GONE_COMMANDS: &[GoneCommand] = &[
    GoneCommand {
        command: "session",
        clauses: &["CL-131"],
    },
    GoneCommand {
        command: "reset",
        clauses: &["CL-118..120", "D-15"],
    },
];

/// The flags of each command (section 2 table; `--yes` from CLI-PL8).
pub const FLAGS: &[Flag] = &[
    Flag {
        command: "botster",
        flag: "--version",
        source: "2",
    },
    Flag {
        command: "botster",
        flag: "-V",
        source: "2",
    },
    Flag {
        command: "botster",
        flag: "--help",
        source: "2",
    },
    Flag {
        command: "botster",
        flag: "-h",
        source: "2",
    },
    Flag {
        command: "start",
        flag: "--headless",
        source: "2",
    },
    Flag {
        command: "start",
        flag: "--offline",
        source: "2",
    },
    Flag {
        command: "start",
        flag: "--swap",
        source: "2",
    },
    Flag {
        command: "start",
        flag: "--drop-unadoptable",
        source: "2",
    },
    Flag {
        command: "attach",
        flag: "--swap",
        source: "2",
    },
    Flag {
        command: "attach",
        flag: "--drop-unadoptable",
        source: "2",
    },
    Flag {
        command: "update",
        flag: "--check",
        source: "2",
    },
    Flag {
        command: "update",
        flag: "--no-wait",
        source: "2",
    },
    Flag {
        command: "get-connection-url",
        flag: "--hub",
        source: "2",
    },
    Flag {
        command: "get-connection-url",
        flag: "--qr",
        source: "2",
    },
    Flag {
        command: "debug memory",
        flag: "--hub",
        source: "2",
    },
    Flag {
        command: "plugin install",
        flag: "--ref",
        source: "2",
    },
    Flag {
        command: "plugin install",
        flag: "--enable",
        source: "2",
    },
    Flag {
        command: "plugin install",
        flag: "--bind",
        source: "2",
    },
    Flag {
        command: "plugin install",
        flag: "--no-wait",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--all",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--preview",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--check",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--bind",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--accept-breaking",
        source: "2",
    },
    Flag {
        command: "plugin update",
        flag: "--no-wait",
        source: "2",
    },
    Flag {
        command: "plugin status",
        flag: "--json",
        source: "2",
    },
    Flag {
        command: "plugin pin",
        flag: "--unpin",
        source: "2",
    },
    Flag {
        command: "plugin rollback",
        flag: "--all",
        source: "2",
    },
    Flag {
        command: "plugin rollback",
        flag: "--no-wait",
        source: "2",
    },
    Flag {
        command: "plugin approve",
        flag: "-y",
        source: "2",
    },
    Flag {
        command: "plugin remove",
        flag: "--purge",
        source: "2",
    },
    Flag {
        command: "plugin remove",
        flag: "-y",
        source: "2",
    },
    Flag {
        command: "plugin remove",
        flag: "--no-wait",
        source: "2",
    },
    Flag {
        command: "plugin remove",
        flag: "--yes",
        source: "CLI-PL8",
    },
];

/// The legacy flags that v1 drops, each with the clause that drops it.
pub const DROPPED_FLAGS: &[Flag] = &[Flag {
    command: "delete-worktree",
    flag: "--force",
    source: "D-18",
}];

/// The answers that a confirmation prompt accepts as yes (CLI-PL8); any other answer aborts.
pub const CONFIRM_ANSWERS: &[&str] = &["y", "yes"];

/// The MCP tools that `mcp-serve` offers outside a session (CLI-MS4).
pub const MCP_TOOLS: &[&str] = &["botster_status"];

/// The labels of the CLI log files, the `<label>` of the log path (CLI-G3).
pub const LOG_LABELS: &[&str] = &["start", "attach", "cli"];

/// The keys of `<config root>/dev-checkouts.toml`, one per component; each maps to the absolute path of a checkout (CLI-DV4). They are
/// keys of a CLI file, not the Hub's installed identities.
pub const DEV_CHECKOUT_KEYS: &[&str] = &["hub", "worker", "plugin_hosts", "mcp_bridge"];

/// The keys of `botster context <key>` (section 6, the alias table of CLI-CX3).
pub const CONTEXT_KEYS: &[&str] = &[
    "prompt",
    "worktree_path",
    "branch_name",
    "repo",
    "session_uuid",
    "hub_id",
    "agent_name",
    "session_dir",
    "workspace_id",
];

/// The keys of the CLI-CX3 alias table whose source is "(dropped)".
pub const DROPPED_CONTEXT_KEYS: &[&str] = &["hub_manifest_path"];

/// The keys that `botster config` prints (CLI-CF1).
pub const CONFIG_KEYS: &[&str] = &[
    "app_name",
    "config_dir",
    "data_dir",
    "hub_socket",
    "log_dir",
];

/// The legacy `config` keys that CLI-CF1 does not print.
pub const DROPPED_CONFIG_KEYS: &[&str] = &[
    "server_url",
    "poll_interval",
    "agent_timeout",
    "max_sessions",
    "worktree_base",
];

/// The fields of the `Hub status` block (CLI-SS1).
pub const STATUS_FIELDS: &[&str] = &[
    "socket",
    "running",
    "protocol",
    "hub_id",
    "pid",
    "uptime",
    "data_root",
    "sessions",
    "plugins",
];

/// The legacy `status` fields that CLI-SS1 drops (ruling UD-5, D-17).
pub const DROPPED_STATUS_FIELDS: &[&str] = &[
    "pid_file",
    "manifest",
    "manifest_pid",
    "server_id",
    "updated_at",
    "workspaces",
    "lock_file",
];

/// The texts that the CLI prints (sections 3 to 9c).
pub const MESSAGES: &[SpecText] = &[
    SpecText { clause: "CLI-G1", ordinal: 4, text: "botster <cli version>" },
    SpecText { clause: "CLI-G2", ordinal: 1, text: "Error: <message>" },
    SpecText { clause: "CLI-G3", ordinal: 15, text: "--- log rotated (cap reached) ---" },
    SpecText { clause: "CLI-G7", ordinal: 1, text: "No running Botster hub found. Start one with: botster start --headless" },
    SpecText { clause: "CLI-G7", ordinal: 6, text: "Hub shut down during the command" },
    SpecText { clause: "CLI-G7", ordinal: 9, text: "Hub busy; try again" },
    SpecText { clause: "CLI-RT4b", ordinal: 1, text: "Refusing to connect: <path> is not owned by the current user" },
    SpecText { clause: "CLI-ST1", ordinal: 3, text: "Hub already running — attaching..." },
    SpecText { clause: "CLI-ST1", ordinal: 6, text: "Hub already running on this device (socket=<path>, binary: <running binary>). Use botster attach or stop the existing hub first." },
    SpecText { clause: "CLI-ST3", ordinal: 3, text: "'start' requires an interactive terminal. Use 'botster start --headless'." },
    SpecText { clause: "CLI-ST5", ordinal: 13, text: "Update available: v<cur> -> v<new>. Run 'botster update' to install." },
    SpecText { clause: "CLI-ST5", ordinal: 14, text: "Update available: v<cur> -> v<new>" },
    SpecText { clause: "CLI-ST5", ordinal: 15, text: "Update now? [Y/n]" },
    SpecText { clause: "CLI-ST6", ordinal: 5, text: "Hub runs <running binary>; this is <this binary>. Use --swap to replace it." },
    SpecText { clause: "CLI-ST6", ordinal: 17, text: "Swap refused: sessions that cannot be adopted" },
    SpecText { clause: "CLI-ST6", ordinal: 18, text: "  unadoptable: <session ref> ..." },
    SpecText { clause: "CLI-ST6", ordinal: 19, text: "  unknown protocol: <session ref> ..." },
    SpecText { clause: "CLI-ST6", ordinal: 20, text: "Use --swap --drop-unadoptable to end them." },
    SpecText { clause: "CLI-ST6", ordinal: 22, text: "unknown protocol" },
    SpecText { clause: "CLI-SS1", ordinal: 2, text: "Hub status" },
    SpecText { clause: "CLI-SS1", ordinal: 5, text: "yes" },
    SpecText { clause: "CLI-SS1", ordinal: 6, text: "no" },
    SpecText { clause: "CLI-SS1", ordinal: 9, text: "none" },
    SpecText { clause: "CLI-SS1", ordinal: 12, text: "running binary: dev <sha> | release <version>" },
    SpecText { clause: "CLI-SS1", ordinal: 27, text: "<component> running <channel> <commit> (<count>)" },
    SpecText { clause: "CLI-SS1", ordinal: 30, text: "<component> running unknown unknown (<count>)" },
    SpecText { clause: "CLI-SS1", ordinal: 28, text: "unknown" },
    SpecText { clause: "CLI-SS1", ordinal: 32, text: "binary differs: 'botster start --swap' replaces it" },
    SpecText { clause: "CLI-CX1", ordinal: 3, text: "BOTSTER_CONTEXT_FILE is required for botster context" },
    SpecText { clause: "CLI-CX5", ordinal: 5, text: "session_dir is not available for this session" },
    SpecText { clause: "CLI-CX5", ordinal: 8, text: "paired file path must be relative and stay inside session_dir" },
    SpecText { clause: "CLI-MS4", ordinal: 3, text: "Botster MCP is available only inside a Botster-managed session" },
    SpecText { clause: "CLI-UP1", ordinal: 9, text: "Run 'botster update' to install." },
    SpecText { clause: "CLI-UP1", ordinal: 11, text: "✓ Already running the latest version" },
    SpecText { clause: "CLI-UP1", ordinal: 13, text: "✓ Running v<cur>, newer than the latest release" },
    SpecText { clause: "CLI-UP1", ordinal: 15, text: "Update check failed: <reason text>" },
    SpecText { clause: "CLI-UP1", ordinal: 17, text: "the Hub is offline (hub.toml [hub] offline)" },
    SpecText { clause: "CLI-UP1", ordinal: 19, text: "this is a development build; it is not updated" },
    SpecText { clause: "CLI-UP1", ordinal: 21, text: "the release could not be checked" },
    SpecText { clause: "CLI-UP2", ordinal: 2, text: "✓ Already running the latest version (or newer)" },
    SpecText { clause: "CLI-UP2", ordinal: 4, text: "✓ Successfully updated to version <X>" },
    SpecText { clause: "CLI-UP2", ordinal: 11, text: "An update is already in progress" },
    SpecText { clause: "CLI-UP2", ordinal: 13, text: "A binary swap is in progress; try again afterwards" },
    SpecText { clause: "CLI-UP2", ordinal: 16, text: "operation <id> continues in the Hub" },
    SpecText { clause: "CLI-PU2", ordinal: 3, text: "No connection URL available for hub '<id>'. <reason>" },
    SpecText { clause: "CLI-WT1", ordinal: 16, text: "No worktrees found" },
    SpecText { clause: "CLI-WT1", ordinal: 14, text: "Path" },
    SpecText { clause: "CLI-WT1", ordinal: 15, text: "Branch" },
    SpecText { clause: "CLI-WT2", ordinal: 5, text: "Successfully deleted worktree for issue #<N>" },
    SpecText { clause: "CLI-WT2", ordinal: 6, text: "No worktree for issue #<N>" },
    SpecText { clause: "CLI-WT2", ordinal: 8, text: "Worktree for issue #<N> is in use by a current session" },
    SpecText { clause: "CLI-JS1", ordinal: 6, text: "Key '<k>' not found in path '<p>'" },
    SpecText { clause: "CLI-JS1", ordinal: 10, text: "Cannot delete root object" },
    SpecText { clause: "CLI-PL2", ordinal: 5, text: "Outcome unknown: the Hub restarted before the operation finished. Check with 'botster plugin status <name>'." },
    SpecText { clause: "CLI-PL3", ordinal: 15, text: "Staged <name>: approve its grants with 'botster plugin approve <name>'" },
    SpecText { clause: "CLI-PL3", ordinal: 20, text: "Choose one with --bind <interface>=<provider>" },
    SpecText { clause: "CLI-PL3", ordinal: 29, text: "Installed <name> <version> (<source>)" },
    SpecText { clause: "CLI-PL4", ordinal: 6, text: "<name> up to date" },
    SpecText { clause: "CLI-PL4", ordinal: 13, text: "<name> pinned at <ref>" },
    SpecText { clause: "CLI-PL4", ordinal: 20, text: "Accept with --accept-breaking <consumer>:<interface>" },
    SpecText { clause: "CLI-PL4", ordinal: 36, text: "Choose with --bind <consumer>:<interface>=<provider>" },
    SpecText { clause: "CLI-PL5", ordinal: 5, text: "NAME STATUS VERSION SOURCE PIN" },
    SpecText { clause: "CLI-PL5", ordinal: 15, text: "degraded (<interface>)" },
    SpecText { clause: "CLI-PL7", ordinal: 3, text: "Rolled back <name> to <ref>" },
    SpecText { clause: "CLI-PL8", ordinal: 2, text: "Remove plugin <name>? [y/N]" },
    SpecText { clause: "CLI-PL8", ordinal: 6, text: "Aborted." },
    SpecText { clause: "CLI-PL8", ordinal: 9, text: "Data kept. Use --purge to delete it." },
    SpecText { clause: "CLI-PL9", ordinal: 7, text: "Enabled <name>" },
    SpecText { clause: "CLI-PL9", ordinal: 8, text: "Disabled <name>" },
    SpecText { clause: "CLI-PL9", ordinal: 10, text: "Could not enable <name>: <reason>" },
    SpecText { clause: "CLI-PL9", ordinal: 13, text: "<name> is <reason>" },
    SpecText { clause: "CLI-PL9", ordinal: 15, text: "Approve its grants with 'botster plugin approve <name>'" },
    SpecText { clause: "CLI-PL11", ordinal: 4, text: "Approve these grants for <name>? [y/N]" },
    SpecText { clause: "CLI-PL13", ordinal: 2, text: "Pinned <name> at <ref>" },
    SpecText { clause: "CLI-PL13", ordinal: 4, text: "Unpinned <name>" },
    SpecText { clause: "CLI-PL13", ordinal: 6, text: "Updated <name> <old> -> <new>" },
    SpecText { clause: "CLI-PL13", ordinal: 9, text: "Removed <name>" },
    SpecText { clause: "CLI-PL13", ordinal: 11, text: "Approved <name>" },
    SpecText { clause: "CLI-PL13", ordinal: 13, text: "Added catalog <id>" },
    SpecText { clause: "CLI-PL13", ordinal: 15, text: "Removed catalog <id>" },
    SpecText { clause: "CLI-PL13", ordinal: 17, text: "Refreshed catalog <id>" },
    SpecText { clause: "CLI-PL13", ordinal: 19, text: "ID<TAB>URL<TAB>ENTRIES" },
    SpecText { clause: "CLI-PL13", ordinal: 30, text: "operation <id>" },
    SpecText { clause: "CLI-PL14", ordinal: 3, text: "<name> is already installed" },
    SpecText { clause: "CLI-PL14", ordinal: 5, text: "Not a plugin directory: <path>" },
    SpecText { clause: "CLI-PL14", ordinal: 7, text: "Install failed: <reason>" },
    SpecText { clause: "CLI-PL14", ordinal: 13, text: "<name> is a local plugin (live; nothing to update)" },
    SpecText { clause: "CLI-PL14", ordinal: 17, text: "Another package operation is running; try again afterwards" },
    SpecText { clause: "CLI-PL14", ordinal: 27, text: "Could not fetch <name>; try again" },
    SpecText { clause: "CLI-PL14", ordinal: 30, text: "Nothing to roll back" },
    SpecText { clause: "CLI-DV3", ordinal: 4, text: "Hub restarted with the new build" },];

/// The paths and path templates of the CLI (CLI-G3, CLI-RT2, CLI-RT3, CLI-ST4, CLI-CX5, CLI-CF1, CLI-DV4).
pub const PATHS: &[SpecText] = &[
    SpecText {
        clause: "CLI-G3",
        ordinal: 7,
        text: "<log dir>/botster-<label>-<YYYYMMDD-HHMMSS>.log",
    },
    SpecText {
        clause: "CLI-G3",
        ordinal: 8,
        text: "<log dir>",
    },
    SpecText {
        clause: "CLI-G3",
        ordinal: 9,
        text: "<config root>/logs/cli/",
    },
    SpecText {
        clause: "CLI-G3",
        ordinal: 10,
        text: "<data root>/logs/",
    },
    SpecText {
        clause: "CLI-RT2",
        ordinal: 2,
        text: "~/Library/Application Support/<app>",
    },
    SpecText {
        clause: "CLI-RT2",
        ordinal: 5,
        text: "~/.<app>",
    },
    SpecText {
        clause: "CLI-RT2",
        ordinal: 7,
        text: "<config root>/cli/",
    },
    SpecText {
        clause: "CLI-RT3",
        ordinal: 2,
        text: "$BOTSTER_CONFIG_DIR/hub.sock",
    },
    SpecText {
        clause: "CLI-RT3",
        ordinal: 4,
        text: "<runtime dir>/<app>/hub.sock",
    },
    SpecText {
        clause: "CLI-RT3",
        ordinal: 7,
        text: "/tmp/botster-<uid>",
    },
    SpecText {
        clause: "CLI-ST4",
        ordinal: 4,
        text: "<config root>/hub.toml",
    },
    SpecText {
        clause: "CLI-CX5",
        ordinal: 2,
        text: "<session_dir>/<rel>",
    },
    SpecText {
        clause: "CLI-CF1",
        ordinal: 9,
        text: "<config root>/logs/cli",
    },
    SpecText {
        clause: "CLI-DV4",
        ordinal: 1,
        text: "<config root>/dev-checkouts.toml",
    },
];
