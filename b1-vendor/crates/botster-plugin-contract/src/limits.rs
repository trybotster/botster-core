//! `PluginLimits`, the parameter ranges and the `hub.toml` key paths.
//!
//! Clause: HP section 15, LM-1, A2-35, A2-52, and the parameter lists of sections 9.8 to 9.14 and 11.1.
//!
//! Durations without a unit suffix in their key are milliseconds (WF-5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Whether a `hub.toml` key is re-read at `reload_config` or only at Hub start (HP A2-52).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyMode {
    Live,
    Restart,
}

/// The accepted range of a limit (HP section 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// No range is stated; the Hub bounds it.
    HubBound,
    /// An inclusive numeric range.
    Between(u64, u64),
    /// Another limit is the lower bound (inclusive).
    AtLeast(&'static str),
    /// Another limit is the upper bound (inclusive).
    AtMost(&'static str),
}

/// One parameter of `PluginLimits`: its name, range, `hub.toml` key and clauses.
pub struct LimitSpec {
    pub name: &'static str,
    pub range: Range,
    /// The `hub.toml` table and mode (HP A2-52). `None` when the table names no key for it.
    pub toml: Option<(&'static str, KeyMode)>,
    pub clauses: &'static str,
}

const KIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The lane bounds of `lanes[lane] = {frames, bytes}` (HP section 15, SV-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LaneBound {
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub frames: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub bytes: u64,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The four lanes, by name (HP section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LaneLimits {
    pub control: LaneBound,
    pub invoke: LaneBound,
    pub call: LaneBound,
    pub bulk: LaneBound,
}

macro_rules! limits {
    (
        counts { $( $cn:ident : $cr:expr, $ct:expr, $ccl:literal );* $(;)? }
        extras { $( $en:ident : $et:ty, $er:expr, $ect:expr, $ecl:literal );* $(;)? }
        optional { $( $on:ident : $ot:ty, $or:expr, $oct:expr, $ocl:literal );* $(;)? }
    ) => {
        /// The limits that the Hub chooses per plugin and sends in `Hello` (HP section 15, LM-1).
        ///
        /// Numeric fields are u64. `lanes`, `probe_text`, `session_port_range` and the address and path parameters have their own types.
        /// The five Core service protections and `instruction_budget` are optional: a protection that Core cannot apply is reported, not set (HP section 15).
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
        pub struct PluginLimits {
            $( pub $cn: u64, )*
            $( pub $en: $et, )*
            $( #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::common::opt_no_null")] pub $on: Option<$ot>, )*
        }

        /// Every parameter of `PluginLimits`, in declaration order.
        pub const LIMIT_SPECS: &[LimitSpec] = &[
            $( LimitSpec { name: stringify!($cn), range: $cr, toml: $ct, clauses: $ccl }, )*
            $( LimitSpec { name: stringify!($en), range: $er, toml: $ect, clauses: $ecl }, )*
            $( LimitSpec { name: stringify!($on), range: $or, toml: $oct, clauses: $ocl }, )*
        ];
    };
}

use KeyMode::{Live, Restart};
use Range::{AtLeast, AtMost, Between, HubBound};

limits! {
    counts {
        frame_bytes: HubBound, Some(("plugin_limits", Restart)), "MD-2, WF-1";
        startup: HubBound, Some(("plugin_limits", Restart)), "HS-5";
        shutdown: HubBound, Some(("plugin_limits", Restart)), "LD-4";
        orphan_grace: HubBound, Some(("plugin_limits", Restart)), "CR-6";
        restart_window: HubBound, Some(("plugin_limits", Restart)), "CR-3";
        invoke_queue: HubBound, Some(("plugin_limits", Live)), "IV-4";
        dispatch_deadline: HubBound, Some(("plugin_limits", Live)), "IV-3";
        long_deadline: HubBound, Some(("plugin_limits", Live)), "IV-3";
        page_items: HubBound, Some(("plugin_limits", Live)), "ST-4, SS-1";
        max_result_bytes: HubBound, Some(("plugin_limits", Live)), "CA-2, ST-4";
        max_watches: HubBound, Some(("plugin_limits", Live)), "ST-6";
        max_timers: HubBound, Some(("plugin_limits", Live)), "CA-12";
        operations_get_rate: HubBound, Some(("plugin_limits", Live)), "A2-35, CA-16";
        operation_receipt_window: Between(1_000, 86_400_000), Some(("plugin_limits", Live)), "CA-16";
        operation_receipts: Between(16, 65_536), Some(("plugin_limits", Live)), "CA-16";
        log_record_bytes: HubBound, Some(("logging", Live)), "CA-9";
        log_ring_records: HubBound, Some(("logging", Live)), "CA-9";
        log_ring_bytes: HubBound, Some(("logging", Live)), "CA-9";
        log_rate: HubBound, Some(("logging", Live)), "CA-9";
        log_burst: HubBound, Some(("logging", Live)), "CA-9";
        event_key_bytes: Between(1, 65_536), Some(("events", Live)), "EV-1, EV-13";
        event_queue: HubBound, Some(("events", Live)), "EV-5";
        event_queue_high: Between(1, 65_536), Some(("events", Live)), "EV-11";
        event_queue_normal: Between(1, 65_536), Some(("events", Live)), "EV-11";
        event_queue_low: Between(1, 65_536), Some(("events", Live)), "EV-11";
        event_starvation_bound: Between(1, 65_536), Some(("events", Live)), "EV-11";
        ingress_queue: HubBound, Some(("events", Live)), "EV-3";
        max_event_bytes: HubBound, Some(("events", Live)), "EV-1";
        loss_ledger_entries: Between(1, 65_536), Some(("events", Live)), "EV-12";
        terminal_event_bytes: Between(256, MIB), Some(("events", Live)), "EV-14";
        output_tap_chunk_bytes: Between(256, MIB), Some(("events", Live)), "EV-14";
        session_family_inflight: Between(1, 16), Some(("events", Live)), "EV-6";
        max_write_bytes: Between(KIB, 16 * MIB), Some(("sessions", Live)), "SW-7";
        max_raw_write_bytes: Between(KIB, 16 * MIB), Some(("sessions", Live)), "RW-1";
        write_pending_bytes: Between(KIB, 16 * MIB), Some(("sessions", Live)), "SW-5";
        echo_deadline_ms: Between(50, 10_000), Some(("sessions", Live)), "SW-4";
        cleanup_window_ms: Between(50, 60_000), Some(("sessions", Live)), "SW-4";
        write_client_quiet_ms: Between(0, 600_000), Some(("sessions", Live)), "SW-3";
        write_max_wait_ms: AtLeast("write_client_quiet_ms"), Some(("sessions", Live)), "SW-4";
        probe_width: AtMost("max_key_repeat"), None, "SW-3, SW-4";
        max_notification_patterns: HubBound, Some(("sessions", Live)), "CA-22";
        max_argv_bytes: Between(KIB, MIB), Some(("sessions", Live)), "CA-19";
        max_env_bytes: Between(KIB, MIB), Some(("sessions", Live)), "CA-19";
        max_document_bytes: HubBound, Some(("store", Live)), "ST-8";
        max_plugin_bytes: HubBound, Some(("store", Live)), "ST-8";
        max_plugin_documents: HubBound, Some(("store", Live)), "ST-8";
        batch_ops: HubBound, Some(("store", Live)), "ST-3, ST-8";
        net_timeout_max_ms: HubBound, Some(("net", Live)), "NT-3";
        webhook_timeout_ms: HubBound, Some(("net", Live)), "NT-7";
        net_body_bytes: HubBound, Some(("net", Live)), "NT-3, NT-7, WP-5";
        ws_send_queue_bytes: HubBound, Some(("net", Live)), "NT-4";
        net_concurrent_requests: HubBound, Some(("net", Live)), "NT-6";
        net_open_sockets: HubBound, Some(("net", Live)), "NT-6";
        net_bytes_in_flight: HubBound, Some(("net", Live)), "NT-6";
        net_rate: HubBound, Some(("net", Live)), "NT-6";
        max_relay_envelope_bytes: AtMost("frame_bytes"), Some(("web", Live)), "WP-2";
        relay_queue: HubBound, Some(("web", Live)), "WP-4";
        relay_attempts_max: HubBound, Some(("web", Live)), "WP-2";
        relay_attempt_ttl_ms: HubBound, Some(("web", Live)), "WP-2";
        ice_servers_max: HubBound, Some(("web", Live)), "WP-6";
        asset_bytes_max: HubBound, Some(("assets", Live)), "AS-1";
        asset_store_bytes: HubBound, Some(("assets", Live)), "AS-1";
        asset_count_max: HubBound, Some(("assets", Live)), "AS-1";
        asset_token_ttl_ms: HubBound, Some(("assets", Live)), "AS-2";
        notice_ttl_max_ms: HubBound, Some(("notices", Live)), "VW-9";
        notice_queue: HubBound, Some(("notices", Live)), "VW-9";
        process_timeout_max_ms: HubBound, Some(("process", Live)), "PX-3";
        process_tail_bytes: HubBound, Some(("process", Live)), "PX-2";
        process_concurrent: HubBound, Some(("process", Live)), "PX-3";
        fs_read_bytes: HubBound, Some(("fs", Live)), "FS-2";
        max_fs_watches: HubBound, Some(("fs", Live)), "FS-3";
        plugin_call_depth: HubBound, Some(("plugin_calls", Live)), "PC-3";
        plugin_call_idempotency_window: HubBound, Some(("plugin_calls", Live)), "PC-4";
        plugin_call_timeout_max_ms: HubBound, Some(("plugin_calls", Live)), "PC-1";
        package_bytes_max: HubBound, Some(("packages", Live)), "PM-4";
        lock_history_max: HubBound, Some(("packages", Live)), "PM-3";
        catalog_bytes_max: HubBound, Some(("packages", Live)), "PM-2";
        mcp_proxy_call_deadline_ms: HubBound, Some(("mcp_proxy", Live)), "MP-6";
        mcp_proxy_concurrent: HubBound, Some(("mcp_proxy", Live)), "MP-6";
        mcp_servers_max: HubBound, Some(("mcp_proxy", Live)), "MP-1";
        mcp_proxy_init_deadline_ms: HubBound, Some(("mcp_proxy", Live)), "MP-12";
        mcp_proxy_list_pages: HubBound, Some(("mcp_proxy", Live)), "MP-12";
        mcp_proxy_tools_max: HubBound, Some(("mcp_proxy", Live)), "MP-12";
        mcp_proxy_message_bytes: HubBound, Some(("mcp_proxy", Live)), "MP-12";
        host_memory_bytes: Between(MIB, 4 * 1024 * MIB), None, "LM-6, LM-7";
    }
    extras {
        lanes: LaneLimits, HubBound, Some(("plugin_limits", Restart)), "SV-3, section 2";
        probe_text: String, HubBound, Some(("sessions", Restart)), "SW-4";
        session_port_range: [u16; 2], HubBound, Some(("sessions", Restart)), "CA-20";
        target_plugin_dir: String, HubBound, Some(("plugins", Restart)), "CP-8, A2-39";
        process_path: String, HubBound, Some(("plugins", Restart)), "PX-1";
        net_listen: String, HubBound, Some(("net", Restart)), "NT-7, WP-5, A2-41";
        assets_listen: String, HubBound, Some(("net", Restart)), "AS-2";
    }
    optional {
        cpu_seconds: u64, HubBound, Some(("plugin_limits", Restart)), "section 15";
        open_files: u64, HubBound, Some(("plugin_limits", Restart)), "section 15";
        processes: u64, HubBound, Some(("plugin_limits", Restart)), "section 15";
        address_space_bytes: u64, HubBound, Some(("plugin_limits", Restart)), "section 15";
        stderr_bytes: u64, HubBound, Some(("plugin_limits", Restart)), "section 15";
        instruction_budget: u64, HubBound, Some(("plugin_limits", Restart)), "LM-4";
    }
}

/// `trusted_hosts` is a `hub.toml` key of the `[network]` table, not a plugin limit (HP TR-1, A2-51 (c)).
pub const TRUSTED_HOSTS_KEY: (&str, &str, KeyMode) = ("network", "trusted_hosts", Restart);

/// One failure of `PluginLimits::validate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitViolation {
    OutOfRange {
        name: &'static str,
        value: u64,
    },
    CrossLimit {
        name: &'static str,
        other: &'static str,
    },
    ProbeText,
    PortRange,
}

impl PluginLimits {
    /// The numeric value of a named u64 limit, for range checks.
    pub fn number(&self, name: &str) -> Option<u64> {
        serde_json::to_value(self).ok()?.get(name)?.as_u64()
    }

    /// Checks every stated range and cross-limit rule of HP section 15 (A2-52: the Hub refuses a value outside them).
    /// `max_key_repeat` is Core's bound for `probe_width` (HP section 15; Core IN-9).
    pub fn validate(&self, max_key_repeat: u64) -> Vec<LimitViolation> {
        let mut out = Vec::new();
        let value = serde_json::to_value(self).expect("limits serialize");
        for spec in LIMIT_SPECS {
            let Some(v) = value.get(spec.name).and_then(|v| v.as_u64()) else {
                continue;
            };
            match spec.range {
                Range::HubBound => {}
                Range::Between(lo, hi) => {
                    if v < lo || v > hi {
                        out.push(LimitViolation::OutOfRange {
                            name: spec.name,
                            value: v,
                        });
                    }
                }
                Range::AtLeast(other) => {
                    if let Some(o) = value.get(other).and_then(|v| v.as_u64()) {
                        if v < o {
                            out.push(LimitViolation::CrossLimit {
                                name: spec.name,
                                other: static_name(other),
                            });
                        }
                    }
                }
                Range::AtMost(other) => {
                    let bound = if other == "max_key_repeat" {
                        Some(max_key_repeat)
                    } else {
                        value.get(other).and_then(|v| v.as_u64())
                    };
                    if bound.is_some_and(|o| v > o) {
                        out.push(LimitViolation::CrossLimit {
                            name: spec.name,
                            other: static_name(other),
                        });
                    }
                }
            }
        }
        if self.cleanup_window_ms < self.echo_deadline_ms {
            out.push(LimitViolation::CrossLimit {
                name: "cleanup_window_ms",
                other: "echo_deadline_ms",
            });
        }
        let t = &self.probe_text;
        if t.is_empty() || t.len() > 8 || !t.bytes().all(|b| (0x20..0x7f).contains(&b)) {
            out.push(LimitViolation::ProbeText);
        }
        let [first, last] = self.session_port_range;
        if first < 1024 || last < first || (last - first) < 15 {
            out.push(LimitViolation::PortRange);
        }
        out
    }
}

fn static_name(n: &str) -> &'static str {
    LIMIT_SPECS
        .iter()
        .find(|s| s.name == n)
        .map(|s| s.name)
        .unwrap_or(if n == "max_key_repeat" {
            "max_key_repeat"
        } else {
            "?"
        })
}
