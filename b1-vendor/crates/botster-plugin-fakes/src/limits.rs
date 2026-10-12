//! Test values for `PluginLimits` (HP LM-5: every limit has a test value small enough for a suite to reach it).

use botster_plugin_contract::prelude::*;
use serde_json::{json, Map, Value};

/// A valid `PluginLimits` of small test values, with `overrides` (a JSON object) applied over it. `Err` names a value that does not parse.
pub fn test_limits(overrides: &Value) -> Result<PluginLimits, String> {
    let mut m = Map::new();
    for s in LIMIT_SPECS {
        let v = match s.range {
            Range::Between(lo, _) => lo,
            _ => 1_000_000,
        };
        m.insert(s.name.to_string(), json!(v));
    }
    let lane = json!({"frames": 8, "bytes": 262144});
    for (k, v) in [
        ("frame_bytes", json!(65536)),
        ("startup", json!(10000)),
        ("shutdown", json!(5000)),
        ("restart_window", json!(60000)),
        ("invoke_queue", json!(4)),
        ("dispatch_deadline", json!(5000)),
        ("long_deadline", json!(60000)),
        ("max_result_bytes", json!(8192)),
        ("event_queue_high", json!(4)),
        ("event_queue_normal", json!(4)),
        ("event_queue_low", json!(4)),
        ("event_starvation_bound", json!(2)),
        ("loss_ledger_entries", json!(4)),
        ("echo_deadline_ms", json!(100)),
        ("cleanup_window_ms", json!(200)),
        ("write_client_quiet_ms", json!(0)),
        ("write_max_wait_ms", json!(10)),
        ("probe_width", json!(2)),
        ("max_relay_envelope_bytes", json!(1000)),
        ("operation_receipt_window", json!(60_000)),
        (
            "lanes",
            json!({"control": lane, "invoke": lane, "call": lane, "bulk": lane}),
        ),
        ("probe_text", json!("zx")),
        ("session_port_range", json!([20000, 20100])),
        ("target_plugin_dir", json!(".botster/plugins")),
        ("process_path", json!("/usr/bin")),
        ("net_listen", json!("127.0.0.1:0")),
        ("assets_listen", json!("127.0.0.1:0")),
        ("instruction_budget", json!(100_000)),
    ] {
        m.insert(k.to_string(), v);
    }
    if let Value::Object(o) = overrides {
        for (k, v) in o {
            m.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(Value::Object(m)).map_err(|e| e.to_string())
}

/// The receive bound of each lane, from `limits.lanes` (HP section 15, MD-3).
pub fn lane_caps(l: &PluginLimits) -> [(u64, u64); 4] {
    let b = |x: LaneBound| (x.frames, x.bytes);
    [
        b(l.lanes.control),
        b(l.lanes.invoke),
        b(l.lanes.call),
        b(l.lanes.bulk),
    ]
}
