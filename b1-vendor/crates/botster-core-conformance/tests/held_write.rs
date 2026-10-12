//! The held-write transcripts keep the form of steward ruling R-74: `pty_blocked` with `kernel_bytes: 1`, a write of F bytes
//! ([`HELD_WRITE_FILL_BYTES`]), the `pty_queued` fence in `[1, F]`, and `"fake": "structural"` (FakeCore is frozen and has no
//! `pty_queued`).

use botster_core_conformance::{CORE_TRANSCRIPTS, HELD_WRITE_FILL_BYTES};
use botster_core_contract::prelude::CoreLimits;
use serde_json::{json, Value};

/// Every object in a JSON value, depth first.
fn objects<'a>(v: &'a Value, out: &mut Vec<&'a serde_json::Map<String, Value>>) {
    match v {
        Value::Object(map) => {
            out.push(map);
            map.values().for_each(|v| objects(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| objects(v, out)),
        _ => {}
    }
}

#[test]
fn f_is_twice_the_largest_measured_capacity_and_fits_one_paste() {
    // The measured capacities of a non-canonical PTY with no reader (R-74): Linux 6.12 with 1-byte writes, and macOS.
    let largest: u64 = [20480, 1022].into_iter().max().unwrap();
    assert_eq!(HELD_WRITE_FILL_BYTES, 2 * largest);
    assert!(HELD_WRITE_FILL_BYTES as usize <= CoreLimits::default().max_paste_bytes as usize);
}

#[test]
fn every_held_write_transcript_has_the_r74_form() {
    let mut held = vec![];
    for file in CORE_TRANSCRIPTS.files() {
        let t: Value = serde_json::from_slice(file.contents()).unwrap();
        let id = t["id"].as_str().unwrap().to_string();
        let mut all = vec![];
        objects(&t["steps"], &mut all);
        let kernel_bytes: Vec<&Value> = all
            .iter()
            .filter(|o| o.get("op") == Some(&json!("pty_blocked")))
            .filter_map(|o| o.get("kernel_bytes"))
            .collect();
        let queued = all
            .iter()
            .any(|o| o.get("op") == Some(&json!("pty_queued")));
        if kernel_bytes.is_empty() && !queued {
            continue;
        }
        assert!(
            kernel_bytes.iter().all(|n| **n == json!(1)),
            "{id}: kernel_bytes is 1"
        );
        assert!(queued, "{id}: a held write fences with pty_queued");
        assert!(
            !kernel_bytes.is_empty(),
            "{id}: pty_queued needs kernel_bytes"
        );
        assert_eq!(
            t["fake"],
            json!("structural"),
            "{id}: structural on FakeCore"
        );
        let controls = t["requires"]["controls"].as_array().unwrap();
        assert!(
            controls.contains(&json!("pty_queued")),
            "{id}: requires pty_queued"
        );
        let fills: Vec<&Value> = all.iter().filter_map(|o| o.get("repeat")).collect();
        assert!(!fills.is_empty(), "{id}: a fill of F bytes");
        for fill in fills {
            assert_eq!(
                fill[1],
                json!(HELD_WRITE_FILL_BYTES),
                "{id}: the fill is F bytes"
            );
            assert_eq!(
                fill[0].as_str().map(str::len),
                Some(2),
                "{id}: the fill repeats one byte"
            );
        }
        for between in all.iter().filter_map(|o| o.get("$between")) {
            assert_eq!(
                *between,
                json!([1, HELD_WRITE_FILL_BYTES]),
                "{id}: the fence is [1, F]"
            );
        }
        held.push(id);
    }
    held.sort();
    assert_eq!(
        held,
        [
            "conf::a2_2_cancel_race_reports_the_real_outcome",
            "conf::am_2_next_write_waits_for_completion_or_partial_end",
            "conf::in_2_link_loss_is_unknown_not_zero",
            "conf::in_2_partial_is_certain",
            "conf::in_6_cancel_reports_progress",
            "conf::in_6_refused_full_wakes_on_room",
            "conf::in_7_lost_worker_ends_pending",
            "conf::in_9_modes_read_at_write_start",
            "conf::tm_6_begin_and_attach_signal_wake",
        ]
    );
}
