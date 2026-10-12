//! `CoreLimits` (9B): every field with its default, and the pure `validate()` that implements the refusal rules of 9B.
//!
//! Durations travel as integer milliseconds. A field with no default of its own (`max_route_frame_bytes`, `tap_inflight_bytes`)
//! is optional, and its absence means the default that 9B derives from another field.

use crate::error::{CoreError, ErrorCode};
use crate::util::duration_ms;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;
const HOUR: u64 = 3600;

/// The limits of one `Core`. One value passed to `open`.
///
/// Clause: Core 9B, Core LM-1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CoreLimits {
    pub max_session_id_bytes: u32,
    pub max_sessions: u32,
    pub pending_ops: u32,
    pub input_ops_per_session: u32,
    pub input_retained_bytes: u64,
    /// Range 1 to 4,096.
    pub max_key_repeat: u32,
    /// Range 1 KiB to 64 MiB.
    pub max_paste_bytes: u64,
    /// Range 16 to 2^20.
    pub event_queue: u32,
    /// At least `max_sessions`.
    pub mandatory_events: u32,
    pub routes_per_session: u32,
    /// Range 256 to `max_route_frame_bytes`.
    pub output_deflate_min_bytes: u64,
    /// Range 1 KiB to 1 GiB.
    pub max_file_bytes: u64,
    pub route_input_queue_bytes: u64,
    pub tap_queue_bytes: u64,
    pub max_route_tag_bytes: u32,
    /// Range 4 KiB to `max_route_frame_bytes`.
    pub max_history_page_bytes: u64,
    /// Range 64 to `max_query_reply_bytes`.
    pub max_query_text_bytes: u32,
    /// Range 1 to 4,096 (Core A15-1).
    pub max_key_text_bytes: u32,
    /// Range 4 KiB to 1 MiB.
    pub tap_chunk_bytes: u64,
    /// Absent means `tap_chunk_bytes`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_inflight_bytes: Option<u64>,
    /// Range 1 to 64.
    pub admission_fairness_frames: u32,
    /// At least 2.
    pub input_events_kept: u32,
    /// At least `2 * (128 + 2 * max_route_tag_bytes)`.
    pub input_events_bytes: u64,
    pub input_event_payload_bytes: u32,
    pub input_tail_bytes: u32,
    /// Absent means `max_paste_bytes + 4096`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_route_frame_bytes: Option<u64>,
    /// At least `max_snapshot_bytes`.
    pub route_queue_bytes: u64,
    /// Range 100 ms to 600 s.
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub reader_progress_deadline: Duration,
    /// At least `reader_progress_deadline`, at most 24 h.
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub stall_close_after: Duration,
    pub open_captures_per_client: u32,
    /// Range 1 s to 24 h.
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub capture_ttl: Duration,
    pub max_snapshot_bytes: u64,
    pub snapshot_retained_bytes: u64,
    /// Range `pending_queries` to 4,096.
    pub query_history: u32,
    /// Range 1 to 256.
    pub pending_queries: u32,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub max_query_deadline: Duration,
    /// Range 64 to `max_route_frame_bytes`.
    pub max_query_bytes: u64,
    /// Range 64 to `max_route_frame_bytes`.
    pub max_query_reply_bytes: u64,
    pub clipboard_bytes: u64,
    pub service_log_bytes: u64,
    pub max_services: u32,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub startup: Duration,
    #[serde(with = "duration_ms")]
    #[schemars(with = "u64")]
    pub stop_grace: Duration,
    pub pump_bytes: u64,
    pub pump_events: u32,
}

impl Default for CoreLimits {
    /// The documented defaults of 9B.
    ///
    /// Clause: Core 9B.
    fn default() -> Self {
        CoreLimits {
            max_session_id_bytes: 128,
            max_sessions: 256,
            pending_ops: 256,
            input_ops_per_session: 32,
            input_retained_bytes: 2 * MIB,
            max_key_repeat: 64,
            max_paste_bytes: MIB,
            event_queue: 1024,
            mandatory_events: 4096,
            routes_per_session: 16,
            output_deflate_min_bytes: 4 * KIB,
            max_file_bytes: 32 * MIB,
            route_input_queue_bytes: MIB,
            tap_queue_bytes: 4 * MIB,
            max_route_tag_bytes: 256,
            max_history_page_bytes: 256 * KIB,
            max_query_text_bytes: 1024,
            max_key_text_bytes: 256,
            tap_chunk_bytes: 64 * KIB,
            tap_inflight_bytes: None,
            admission_fairness_frames: 1,
            input_events_kept: 64,
            input_events_bytes: 16 * KIB,
            input_event_payload_bytes: 256,
            input_tail_bytes: 1024,
            max_route_frame_bytes: None,
            route_queue_bytes: 32 * MIB,
            reader_progress_deadline: Duration::from_secs(10),
            stall_close_after: Duration::from_secs(60),
            open_captures_per_client: 4,
            capture_ttl: Duration::from_secs(60),
            max_snapshot_bytes: 16 * MIB,
            snapshot_retained_bytes: 64 * MIB,
            query_history: 64,
            pending_queries: 8,
            max_query_deadline: Duration::from_secs(60),
            max_query_bytes: 4096,
            max_query_reply_bytes: 64 * KIB,
            clipboard_bytes: MIB,
            service_log_bytes: 256 * KIB,
            max_services: 256,
            startup: Duration::from_secs(10),
            stop_grace: Duration::from_secs(5),
            pump_bytes: MIB,
            pump_events: 256,
        }
    }
}

/// The refusal of a `CoreLimits` value: the failing field, and the rule that it broke.
///
/// Clause: Core 9B, Core LM-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidConfig {
    pub field: &'static str,
    pub rule: String,
}

impl std::fmt::Display for InvalidConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.rule)
    }
}

impl std::error::Error for InvalidConfig {}

impl From<InvalidConfig> for CoreError {
    /// The `InvalidConfig` code of 9.3, with the failing field.
    ///
    /// Clause: Core 9B, Core 9.3.
    fn from(e: InvalidConfig) -> CoreError {
        CoreError::new(
            ErrorCode::InvalidConfig {
                field: e.field.to_string(),
            },
            e.rule,
        )
    }
}

fn refuse(field: &'static str, rule: impl Into<String>) -> InvalidConfig {
    InvalidConfig {
        field,
        rule: rule.into(),
    }
}

fn in_range(field: &'static str, value: u64, lo: u64, hi: u64) -> Result<(), InvalidConfig> {
    if value < lo || value > hi {
        return Err(refuse(
            field,
            format!("{value} is outside the range {lo} to {hi}"),
        ));
    }
    Ok(())
}

fn in_time(
    field: &'static str,
    value: Duration,
    lo: Duration,
    hi: Duration,
) -> Result<(), InvalidConfig> {
    if value < lo || value > hi {
        return Err(refuse(
            field,
            format!("{value:?} is outside the range {lo:?} to {hi:?}"),
        ));
    }
    Ok(())
}

impl CoreLimits {
    /// `max_route_frame_bytes`, or its 9B default.
    ///
    /// Clause: Core 9B.
    pub fn effective_max_route_frame_bytes(&self) -> u64 {
        self.max_route_frame_bytes
            .unwrap_or(self.max_paste_bytes.saturating_add(4096))
    }

    /// `tap_inflight_bytes`, or its 9B default.
    ///
    /// Clause: Core 9B.
    pub fn effective_tap_inflight_bytes(&self) -> u64 {
        self.tap_inflight_bytes.unwrap_or(self.tap_chunk_bytes)
    }

    /// The 9B refusal rules. `Err` names the first failing field.
    ///
    /// Clause: Core 9B, Core LM-1.
    pub fn validate(&self) -> Result<(), InvalidConfig> {
        self.check_at_least_one()?;
        self.check_ranges()?;
        self.check_cross_limits()
    }

    /// 9B: every field is at least 1. No field of 9B allows zero.
    fn check_at_least_one(&self) -> Result<(), InvalidConfig> {
        macro_rules! nonzero {
            ($($f:ident),* $(,)?) => {$(
                if u64::from(self.$f) == 0 {
                    return Err(refuse(stringify!($f), "must be at least 1"));
                }
            )*};
        }
        macro_rules! nonzero_time {
            ($($f:ident),* $(,)?) => {$(
                if self.$f.is_zero() {
                    return Err(refuse(stringify!($f), "must be at least 1"));
                }
            )*};
        }
        nonzero!(
            max_session_id_bytes,
            max_sessions,
            pending_ops,
            input_ops_per_session,
            input_retained_bytes,
            max_key_repeat,
            max_paste_bytes,
            event_queue,
            mandatory_events,
            routes_per_session,
            output_deflate_min_bytes,
            max_file_bytes,
            route_input_queue_bytes,
            tap_queue_bytes
        );
        nonzero!(
            max_route_tag_bytes,
            max_history_page_bytes,
            max_query_text_bytes,
            max_key_text_bytes,
            tap_chunk_bytes
        );
        if self.tap_inflight_bytes == Some(0) {
            return Err(refuse("tap_inflight_bytes", "must be at least 1"));
        }
        nonzero!(
            admission_fairness_frames,
            input_events_kept,
            input_events_bytes,
            input_event_payload_bytes,
            input_tail_bytes
        );
        if self.max_route_frame_bytes == Some(0) {
            return Err(refuse("max_route_frame_bytes", "must be at least 1"));
        }
        nonzero!(route_queue_bytes);
        nonzero_time!(reader_progress_deadline, stall_close_after);
        nonzero!(open_captures_per_client);
        nonzero_time!(capture_ttl);
        nonzero!(
            max_snapshot_bytes,
            snapshot_retained_bytes,
            query_history,
            pending_queries
        );
        nonzero_time!(max_query_deadline);
        nonzero!(
            max_query_bytes,
            max_query_reply_bytes,
            clipboard_bytes,
            service_log_bytes,
            max_services
        );
        nonzero_time!(startup, stop_grace);
        nonzero!(pump_bytes, pump_events);
        Ok(())
    }

    /// 9B "Ranges (normative)", and the ranges that the field rows state.
    fn check_ranges(&self) -> Result<(), InvalidConfig> {
        let frame = self.effective_max_route_frame_bytes();
        let secs = Duration::from_secs;
        in_range("max_key_repeat", self.max_key_repeat.into(), 1, 4096)?;
        in_range("max_paste_bytes", self.max_paste_bytes, KIB, 64 * MIB)?;
        in_range("event_queue", self.event_queue.into(), 16, 1 << 20)?;
        in_range(
            "output_deflate_min_bytes",
            self.output_deflate_min_bytes,
            256,
            frame,
        )?;
        in_range("max_file_bytes", self.max_file_bytes, KIB, GIB)?;
        in_range(
            "max_history_page_bytes",
            self.max_history_page_bytes,
            4 * KIB,
            frame,
        )?;
        in_range("tap_chunk_bytes", self.tap_chunk_bytes, 4 * KIB, MIB)?;
        in_range(
            "admission_fairness_frames",
            self.admission_fairness_frames.into(),
            1,
            64,
        )?;
        in_time(
            "reader_progress_deadline",
            self.reader_progress_deadline,
            Duration::from_millis(100),
            secs(600),
        )?;
        in_time(
            "stall_close_after",
            self.stall_close_after,
            self.reader_progress_deadline,
            secs(24 * HOUR),
        )?;
        in_time("capture_ttl", self.capture_ttl, secs(1), secs(24 * HOUR))?;
        in_range("pending_queries", self.pending_queries.into(), 1, 256)?;
        in_range(
            "query_history",
            self.query_history.into(),
            self.pending_queries.into(),
            4096,
        )?;
        in_range("max_query_bytes", self.max_query_bytes, 64, frame)?;
        in_range(
            "max_query_reply_bytes",
            self.max_query_reply_bytes,
            64,
            frame,
        )?;
        in_range(
            "max_query_text_bytes",
            self.max_query_text_bytes.into(),
            64,
            self.max_query_reply_bytes,
        )?;
        in_range(
            "max_key_text_bytes",
            self.max_key_text_bytes.into(),
            1,
            4096,
        )?;
        Ok(())
    }

    /// 9B cross-limit rules: `mandatory_events`, `route_queue_bytes`, and the protected records of ST-7.
    fn check_cross_limits(&self) -> Result<(), InvalidConfig> {
        if self.mandatory_events < self.max_sessions {
            return Err(refuse("mandatory_events", "must be at least max_sessions"));
        }
        if self.route_queue_bytes < self.max_snapshot_bytes {
            return Err(refuse(
                "route_queue_bytes",
                "must be at least max_snapshot_bytes",
            ));
        }
        if self.input_events_kept < 2 {
            return Err(refuse(
                "input_events_kept",
                "must be at least 2 (the newest record and its status update)",
            ));
        }
        let protected = 2 * (128 + 2 * u64::from(self.max_route_tag_bytes));
        if self.input_events_bytes < protected {
            return Err(refuse(
                "input_events_bytes",
                format!("must be at least 2 * (128 + 2 * max_route_tag_bytes) = {protected}"),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// Every field of the 9B table, in table order.
    const FIELDS: [&str; 45] = [
        "max_session_id_bytes",
        "max_sessions",
        "pending_ops",
        "input_ops_per_session",
        "input_retained_bytes",
        "max_key_repeat",
        "max_paste_bytes",
        "event_queue",
        "mandatory_events",
        "routes_per_session",
        "output_deflate_min_bytes",
        "max_file_bytes",
        "route_input_queue_bytes",
        "tap_queue_bytes",
        "max_route_tag_bytes",
        "max_history_page_bytes",
        "max_query_text_bytes",
        "max_key_text_bytes",
        "tap_chunk_bytes",
        "tap_inflight_bytes",
        "admission_fairness_frames",
        "input_events_kept",
        "input_events_bytes",
        "input_event_payload_bytes",
        "input_tail_bytes",
        "max_route_frame_bytes",
        "route_queue_bytes",
        "reader_progress_deadline",
        "stall_close_after",
        "open_captures_per_client",
        "capture_ttl",
        "max_snapshot_bytes",
        "snapshot_retained_bytes",
        "query_history",
        "pending_queries",
        "max_query_deadline",
        "max_query_bytes",
        "max_query_reply_bytes",
        "clipboard_bytes",
        "service_log_bytes",
        "max_services",
        "startup",
        "stop_grace",
        "pump_bytes",
        "pump_events",
    ];

    /// The defaults with the given JSON fields replaced.
    fn limits(patch: Value) -> CoreLimits {
        serde_json::from_value(patch).expect("a CoreLimits patch parses")
    }

    fn refused(patch: Value) -> &'static str {
        limits(patch)
            .validate()
            .expect_err("the value is refused")
            .field
    }

    #[test]
    fn the_defaults_are_valid() {
        // Core 9B
        CoreLimits::default().validate().unwrap();
        assert_eq!(limits(json!({})), CoreLimits::default());
    }

    #[test]
    fn every_9b_field_is_a_field_of_the_type() {
        // Core 9B, Core A15-2: the table lists these 48 fields. Optional ones are absent from the default JSON.
        let value = serde_json::to_value(CoreLimits::default()).unwrap();
        let have: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for f in FIELDS {
            let optional = ["tap_inflight_bytes", "max_route_frame_bytes"].contains(&f);
            assert!(have.contains(&f) || optional, "{f} is not a field");
        }
        assert_eq!(have.len() + 2, FIELDS.len());
    }

    #[test]
    fn a_zero_in_any_field_is_invalid_config_naming_it() {
        // Core 9B (every field at least 1), Core LM-1: one refusal per field.
        for field in FIELDS {
            assert_eq!(refused(json!({ field: 0 })), field, "{field}");
        }
    }

    #[test]
    fn an_unknown_field_is_refused_at_parse() {
        // Core 9B: the table names every field.
        assert!(serde_json::from_value::<CoreLimits>(json!({ "unknown_limit": 1 })).is_err());
    }

    /// One row per stated range: the field, its lowest and highest legal value, and the patch that makes the neighbours consistent.
    #[test]
    fn every_stated_range_is_enforced_at_both_ends() {
        // Core 9B (ranges): reader_progress_deadline 100 ms to 600 s; stall_close_after reader..24 h; pending_queries 1..256;
        // query_history pending_queries..4096; max_query_bytes 64..frame; max_query_reply_bytes 64..frame; capture_ttl 1 s..24 h;
        // event_queue 16..2^20; max_file_bytes 1 KiB..1 GiB; max_paste_bytes 1 KiB..64 MiB.
        // Core 9B (field rows): max_key_repeat 1..4096; output_deflate_min_bytes 256..frame; max_history_page_bytes 4 KiB..frame;
        // max_query_text_bytes 64..max_query_reply_bytes; tap_chunk_bytes 4 KiB..1 MiB; admission_fairness_frames 1..64.
        // Core A15-1: max_key_text_bytes 1..4096.
        let cases: [(&str, u64, u64, Value); 14] = [
            ("max_key_repeat", 1, 4096, json!({})),
            (
                "max_paste_bytes",
                KIB,
                64 * MIB,
                json!({ "max_history_page_bytes": 4096, "output_deflate_min_bytes": 256, "max_query_reply_bytes": 4096 }),
            ),
            ("event_queue", 16, 1 << 20, json!({})),
            ("output_deflate_min_bytes", 256, MIB + 4096, json!({})),
            ("max_file_bytes", KIB, GIB, json!({})),
            ("max_history_page_bytes", 4 * KIB, MIB + 4096, json!({})),
            ("max_query_text_bytes", 64, 64 * KIB, json!({})),
            ("max_key_text_bytes", 1, 4096, json!({})),
            (
                "tap_chunk_bytes",
                4 * KIB,
                MIB,
                json!({ "tap_inflight_bytes": 1 }),
            ),
            ("admission_fairness_frames", 1, 64, json!({})),
            ("pending_queries", 1, 256, json!({ "query_history": 4096 })),
            ("query_history", 8, 4096, json!({})),
            ("max_query_bytes", 64, MIB + 4096, json!({})),
            (
                "max_query_reply_bytes",
                64,
                MIB + 4096,
                json!({ "max_query_text_bytes": 64 }),
            ),
        ];
        for (field, lo, hi, base) in &cases {
            let with = |v: u64| {
                let mut p = base.clone();
                p[field] = json!(v);
                p
            };
            limits(with(*lo))
                .validate()
                .unwrap_or_else(|e| panic!("{field} at its lowest legal value: {e}"));
            assert_eq!(refused(with(lo - 1)), *field, "{field} below its range");
            if *hi != u64::MAX {
                limits(with(*hi))
                    .validate()
                    .unwrap_or_else(|e| panic!("{field} at its highest legal value: {e}"));
                assert_eq!(refused(with(hi + 1)), *field, "{field} above its range");
            }
        }
    }

    #[test]
    fn the_key_text_bound_defaults_to_256() {
        // Core A15-1: default 256.
        assert_eq!(CoreLimits::default().max_key_text_bytes, 256);
    }

    #[test]
    fn the_time_ranges_are_enforced() {
        // Core 9B (ranges): reader_progress_deadline 100 ms to 600 s; capture_ttl 1 s to 24 h; stall_close_after up to 24 h.
        let ok = |p: Value| limits(p).validate().unwrap();
        ok(json!({ "reader_progress_deadline": 100 }));
        assert_eq!(
            refused(json!({ "reader_progress_deadline": 99 })),
            "reader_progress_deadline"
        );
        ok(json!({ "reader_progress_deadline": 600_000, "stall_close_after": 600_000 }));
        assert_eq!(
            refused(json!({ "reader_progress_deadline": 600_001, "stall_close_after": 700_000 })),
            "reader_progress_deadline"
        );
        ok(json!({ "capture_ttl": 1000 }));
        assert_eq!(refused(json!({ "capture_ttl": 999 })), "capture_ttl");
        ok(json!({ "capture_ttl": 24 * 3600 * 1000 }));
        assert_eq!(
            refused(json!({ "capture_ttl": 24 * 3600 * 1000 + 1 })),
            "capture_ttl"
        );
        ok(json!({ "stall_close_after": 24 * 3600 * 1000 }));
        assert_eq!(
            refused(json!({ "stall_close_after": 24 * 3600 * 1000 + 1 })),
            "stall_close_after"
        );
    }

    #[test]
    fn stall_close_after_must_reach_reader_progress_deadline() {
        // Core 9B (stall_close_after at least reader_progress_deadline)
        limits(json!({ "reader_progress_deadline": 20_000, "stall_close_after": 20_000 }))
            .validate()
            .unwrap();
        assert_eq!(
            refused(json!({ "reader_progress_deadline": 20_000, "stall_close_after": 19_999 })),
            "stall_close_after"
        );
    }

    #[test]
    fn query_history_must_reach_pending_queries() {
        // Core 9B (query_history from pending_queries to 4,096)
        limits(json!({ "pending_queries": 9, "query_history": 9 }))
            .validate()
            .unwrap();
        assert_eq!(
            refused(json!({ "pending_queries": 9, "query_history": 8 })),
            "query_history"
        );
    }

    #[test]
    fn mandatory_events_must_reach_max_sessions() {
        // Core 9B (mandatory_events)
        limits(json!({ "max_sessions": 4096 })).validate().unwrap();
        assert_eq!(refused(json!({ "max_sessions": 4097 })), "mandatory_events");
    }

    #[test]
    fn route_queue_bytes_must_hold_a_snapshot() {
        // Core 9B (route_queue_bytes), Core OU-9
        limits(json!({ "route_queue_bytes": 16 * MIB }))
            .validate()
            .unwrap();
        assert_eq!(
            refused(json!({ "route_queue_bytes": 16 * MIB - 1 })),
            "route_queue_bytes"
        );
    }

    #[test]
    fn the_protected_input_records_must_fit() {
        // Core 9B (input_events_kept, input_events_bytes), Core ST-7, Core st_7_limits_that_cannot_hold_the_protected_records
        assert_eq!(
            refused(json!({ "input_events_kept": 1 })),
            "input_events_kept"
        );
        limits(json!({ "input_events_kept": 2 }))
            .validate()
            .unwrap();
        let need = 2 * (128 + 2 * 256);
        limits(json!({ "input_events_bytes": need }))
            .validate()
            .unwrap();
        assert_eq!(
            refused(json!({ "input_events_bytes": need - 1 })),
            "input_events_bytes"
        );
        let need = 2 * (128 + 2 * 1000);
        limits(json!({ "max_route_tag_bytes": 1000, "input_events_bytes": need }))
            .validate()
            .unwrap();
        assert_eq!(
            refused(json!({ "max_route_tag_bytes": 1000, "input_events_bytes": need - 1 })),
            "input_events_bytes"
        );
    }

    #[test]
    fn the_derived_defaults_follow_their_source_field() {
        // Core 9B (max_route_frame_bytes, tap_inflight_bytes)
        let l = limits(json!({ "max_paste_bytes": 2 * MIB, "tap_chunk_bytes": 8 * KIB }));
        assert_eq!(l.effective_max_route_frame_bytes(), 2 * MIB + 4096);
        assert_eq!(l.effective_tap_inflight_bytes(), 8 * KIB);
        let l = limits(json!({ "max_route_frame_bytes": 5000, "tap_inflight_bytes": 7 }));
        assert_eq!(l.effective_max_route_frame_bytes(), 5000);
        assert_eq!(l.effective_tap_inflight_bytes(), 7);
    }

    #[test]
    fn a_refusal_becomes_the_invalid_config_code_with_the_field() {
        // Core 9B, Core 9.3 (InvalidConfig)
        let e: CoreError = limits(json!({ "event_queue": 0 }))
            .validate()
            .unwrap_err()
            .into();
        assert_eq!(
            e.code,
            ErrorCode::InvalidConfig {
                field: "event_queue".into()
            }
        );
        assert_eq!(e.category, crate::error::ErrorCategory::Caller);
    }
}
