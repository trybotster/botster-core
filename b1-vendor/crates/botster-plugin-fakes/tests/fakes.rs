//! Tests of the fakes themselves: the behaviors that no transcript names, the script-dependent clause branches (design 7.2, rule 6), the reachability of every choice point
//! (design 7.2, rule 3) and the property tests of the receive order (steward ruling R-6) and the lanes (R-4).

use botster_plugin_contract::prelude::*;
use botster_plugin_fakes::*;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn wire(lane: u8, ty: u8, payload: &Value) -> Vec<u8> {
    encode_raw(lane, ty, &serde_json::to_vec(payload).unwrap(), u32::MAX).unwrap()
}

fn frames() -> Value {
    serde_json::from_str(include_str!("../../../conformance/hp/fixtures/frames.json")).unwrap()
}

fn items(bytes: &mut Vec<Vec<u8>>) -> Vec<(u8, u8, Value)> {
    let mut all: Vec<u8> = Vec::new();
    for b in bytes.drain(..) {
        all.extend(b);
    }
    let mut out = vec![];
    while all.len() >= 6 {
        let len = u32::from_le_bytes([all[0], all[1], all[2], all[3]]) as usize;
        out.push((
            all[4],
            all[5],
            serde_json::from_slice(&all[6..4 + len]).unwrap_or(Value::Null),
        ));
        all.drain(..4 + len);
    }
    out
}

fn drain(e: &mut dyn FakeEndpoint, conn: &str) -> Vec<(u8, u8, Value)> {
    let mut chunks = vec![];
    while let Some(b) = e.read(conn) {
        chunks.push(b);
    }
    items(&mut chunks)
}

/// A loaded FakeHub with the plugin registered: returns the Hub and the epoch.
fn loaded_hub(seed: u64, setup: Value) -> (FakeHub, u64) {
    let mut h = FakeHub::new(seed);
    let mut setup = setup;
    setup["start"] = json!(true);
    // The script answers every call that a test makes (R-1.3).
    if setup.get("calls").is_none() {
        setup["calls"] = json!({"log.write": [{"result": {"ok": true, "value": {"seq": 1}}}]});
    }
    h.configure(&setup).unwrap();
    let hello = drain(&mut h, "main");
    let epoch = hello[0].2["epoch"].as_u64().unwrap();
    let mut ready = frames()["ready"].clone();
    ready["epoch"] = json!(epoch);
    ready["handlers"] = json!([{"id": "h1", "kind": "mcp_tool", "descriptor": {}}]);
    h.write("main", &wire(0, 2, &ready)).unwrap();
    assert_eq!(drain(&mut h, "main")[0].1, 3, "Activated follows Ready");
    (h, epoch)
}

fn host_call(epoch: u64, id: u64, call: &str, ctx: Value) -> Value {
    let mut c = frames()["host_call"].clone();
    c["epoch"] = json!(epoch);
    c["call_id"] = json!(id);
    c["call"] = json!(call);
    c["context"] = ctx;
    c["args"] = json!({"level": "info", "message": "m"});
    c
}

/// The frames.json templates carry the test limits that the fake sends (design 5.1: one source of truth).
#[test]
fn frame_templates_match_the_test_limits() {
    let want = serde_json::to_value(test_limits(&Value::Null).unwrap()).unwrap();
    assert_eq!(frames()["hello"]["limits"], want);
}

/// HP CA-2: a scripted result over `max_result_bytes` is `response_too_large`; a result of exactly that size is delivered. The size is script data, so no transcript can assert it.
#[test]
fn ca_2_result_bound_is_inclusive() {
    let big = json!({"ok": true, "value": "x".repeat(40)});
    let size = serde_json::to_vec(&big).unwrap().len() as u64;
    for (max, expect_ok) in [(size, true), (size - 1, false)] {
        let (mut h, e) = loaded_hub(1, json!({"calls": {"hub.identity": [{"result": big}]}}));
        let mut c = host_call(e, 1, "hub.identity", json!("bootstrap"));
        c["max_result_bytes"] = json!(max);
        // `bootstrap` after Activated is refused, so call from a live request instead.
        h.control(&json!({"op": "invoke", "handler": "h1"}))
            .unwrap();
        let inv = drain(&mut h, "main")
            .into_iter()
            .find(|f| f.1 == 0x10)
            .unwrap();
        c["context"] = json!({"request": inv.2["request_id"]});
        h.write("main", &wire(2, 32, &c)).unwrap();
        let out = drain(&mut h, "main");
        assert_eq!(
            out[0].2["result"]["ok"] == json!(true),
            expect_ok,
            "max {max}: {:?}",
            out[0].2
        );
        if !expect_ok {
            assert_eq!(out[0].2["result"]["error"]["kind"], "response_too_large");
        }
    }
}

/// HP CA-2, CA-5: the size bound keeps the scripted effect of the call; the fake does not turn it into `committed` or `none`.
#[test]
fn ca_2_response_too_large_keeps_the_scripted_effect() {
    let script = json!({"store.put": [{"result": {"ok": true, "value": {"revision": "x".repeat(200)}}, "effect": "unknown"}]});
    let (mut h, e) = loaded_hub(1, json!({"calls": script}));
    h.control(&json!({"op": "invoke", "handler": "h1"}))
        .unwrap();
    let inv = drain(&mut h, "main")
        .into_iter()
        .find(|f| f.1 == 0x10)
        .unwrap();
    let mut c = host_call(e, 1, "store.put", json!({"request": inv.2["request_id"]}));
    c["max_result_bytes"] = json!(100);
    h.write("main", &wire(2, 32, &c)).unwrap();
    let out = drain(&mut h, "main");
    assert_eq!(out[0].2["result"]["error"]["kind"], "response_too_large");
    assert_eq!(out[0].2["effect"], "unknown");
}

/// HP EV-13: an unkeyed normal copy drops the oldest when the queue is full, a keyed copy replaces the older copy of its key at the end, and a marker counts the loss.
/// HP EV-12: a full high queue takes no further copy and the loss is counted.
#[test]
fn event_queues_shed_by_class() {
    let reg = |h: &mut FakeHub, epoch: u64| {
        let mut r = frames()["ready"].clone();
        r["epoch"] = json!(epoch);
        r["handlers"] = json!([{"id": "he", "kind": "event", "descriptor": {}}, {"id": "h1", "kind": "mcp_tool", "descriptor": {}}]);
        r["events"] = json!({"subscribed": [{"handler_id": "he", "pattern": {"owner": "hub", "name": "x"}}], "gap_handler": "he"});
        h.write("main", &wire(0, 2, &r)).unwrap();
    };
    let mut h = FakeHub::new(3);
    h.configure(&json!({"limits": {"event_queue_normal": 2, "event_queue_high": 1}}))
        .unwrap();
    let epoch = drain(&mut h, "main")[0].2["epoch"].as_u64().unwrap();
    reg(&mut h, epoch);
    drain(&mut h, "main");
    // Five copies arrive before the first is sent; the normal queue holds two.
    h.control(
        &json!({"op": "emit_events", "emitters": [{"owner": "hub", "name": "x", "count": 5}]}),
    )
    .unwrap();
    let first = drain(&mut h, "main");
    assert_eq!(first.len(), 1);
    let mut seen = vec![];
    let mut at = first;
    loop {
        for f in at.drain(..) {
            seen.push(f.2.clone());
            let mut r = frames()["result"].clone();
            r["epoch"] = json!(epoch);
            r["request_id"] = f.2["request_id"].clone();
            h.write("main", &wire(1, 0x11, &r)).unwrap();
        }
        at = drain(&mut h, "main");
        if at.is_empty() {
            break;
        }
    }
    let gaps: Vec<&Value> = seen.iter().filter(|f| f["origin"] == "event_gap").collect();
    assert_eq!(gaps.len(), 1, "one marker for the name: {seen:?}");
    assert_eq!(
        gaps[0]["payload"]["count"], 3,
        "five copies arrive together, capacity two: three lost"
    );
}

/// Design 7.2, rule 3: every choice point is reachable. Across seeds each takes at least two outcomes.
#[test]
fn every_choice_point_is_reachable() {
    let mut seen: std::collections::BTreeMap<Choice, BTreeSet<usize>> = Default::default();
    let mut note = |c: &std::collections::BTreeMap<Choice, Vec<usize>>| {
        for (k, v) in c {
            seen.entry(*k).or_default().extend(v.iter().copied());
        }
    };
    for seed in 0..32 {
        let (mut h, e) = loaded_hub(seed, json!({}));
        // Lane order and batching: frames on all lanes arrive together.
        for i in 0..4 {
            h.write(
                "main",
                &wire(
                    3,
                    32,
                    &host_call(e, 100 + i, "log.write", json!("autonomous")),
                ),
            )
            .unwrap();
            h.write(
                "main",
                &wire(
                    2,
                    32,
                    &host_call(e, 200 + i, "hub.identity", json!("autonomous")),
                ),
            )
            .unwrap();
        }
        // Concurrent admission order.
        h.control(&json!({"op": "invoke", "batch": [{"handler": "h1"}, {"handler": "h1"}, {"handler": "h1"}]})).unwrap();
        let inv = drain(&mut h, "main")
            .into_iter()
            .find(|f| f.1 == 0x10)
            .unwrap();
        // A Result that crosses a cancel.
        let mut r = frames()["result"].clone();
        r["epoch"] = json!(e);
        r["request_id"] = inv.2["request_id"].clone();
        h.write("main", &wire(1, 0x11, &r)).unwrap();
        h.control(&json!({"op": "cancel", "ticket": 1})).unwrap();
        note(h.choices());
        // Event order across emitters.
        let mut h2 = FakeHub::new(seed);
        h2.configure(&json!({})).unwrap();
        let e2 = drain(&mut h2, "main")[0].2["epoch"].as_u64().unwrap();
        let mut rd = frames()["ready"].clone();
        rd["epoch"] = json!(e2);
        rd["handlers"] = json!([{"id": "he", "kind": "event", "descriptor": {}}]);
        rd["events"] = json!({"subscribed": [{"handler_id": "he", "pattern": {"owner": "hub", "name": "x"}}], "gap_handler": "he"});
        h2.write("main", &wire(0, 2, &rd)).unwrap();
        drain(&mut h2, "main");
        h2.control(&json!({"op": "emit_events", "emitters": [{"owner": "hub", "name": "a", "count": 3}, {"owner": "plugin", "name": "b", "count": 3}]})).unwrap();
        note(h2.choices());
        // A host that abandons a cancelled handler may or may not send a late Result.
        let mut p = FakePlugin::new(seed);
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../conformance/hp/fixtures/host_hold.json"
        ))
        .unwrap();
        p.configure(&json!({"fixture": fixture})).unwrap();
        let mut hello = frames()["hello"].clone();
        hello["epoch"] = json!(7);
        p.write("main", &wire(0, 1, &hello)).unwrap();
        drain(&mut p, "main");
        p.write("main", &wire(0, 3, &json!({"epoch": 7}))).unwrap();
        let mut inv = frames()["invoke"].clone();
        inv["epoch"] = json!(7);
        p.write("main", &wire(1, 0x10, &inv)).unwrap();
        drain(&mut p, "main");
        p.write(
            "main",
            &wire(0, 6, &json!({"epoch": 7, "request_id": 1, "reason": "x"})),
        )
        .unwrap();
        drain(&mut p, "main");
        note(p.choices());
    }
    for c in [
        Choice::LaneOrder,
        Choice::Batching,
        Choice::ConcurrentInvoke,
        Choice::EmitterOrder,
        Choice::CancelRace,
        Choice::LateResult,
    ] {
        assert!(
            seen.get(&c).is_some_and(|v| v.len() >= 2),
            "choice point {c:?} took {:?}",
            seen.get(&c)
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum Body {
    Current,
    Stale,
    NotObject,
    BadUtf8,
    Trailing,
}

fn body_bytes(b: Body, epoch: u64) -> Vec<u8> {
    match b {
        Body::Current => serde_json::to_vec(&json!({"epoch": epoch, "x": 1})).unwrap(),
        Body::Stale => serde_json::to_vec(&json!({"epoch": epoch + 1000, "x": 1})).unwrap(),
        Body::NotObject => b"[1,2]".to_vec(),
        Body::BadUtf8 => b"{\"epoch\":1,\"x\":\"\xff\"}".to_vec(),
        Body::Trailing => format!("{{\"epoch\":{epoch}}} 7").into_bytes(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// Steward ruling R-6: a receiver checks the payload form, then the epoch, then the type. A bad payload ends the epoch whatever the type; a stale frame is dropped and
    /// counted and never gets a `Fault`; only then does the type decide (HP WF-2, MD-4, VR-2).
    #[test]
    fn receive_order_is_length_payload_epoch_type(
        seed in 0u64..32,
        body in prop_oneof![Just(Body::Current), Just(Body::Stale), Just(Body::NotObject), Just(Body::BadUtf8), Just(Body::Trailing)],
        ty in (0u8..=255).prop_filter("unknown type", |t| ![1, 2, 3, 4, 5, 6, 0x10, 0x11, 0x20, 0x21].contains(t)),
        lane in 0u8..4,
    ) {
        let (mut h, e) = loaded_hub(seed, json!({}));
        let bytes = body_bytes(body, e);
        h.write("main", &encode_raw(lane, ty, &bytes, u32::MAX).unwrap()).unwrap();
        let out = drain(&mut h, "main");
        let counters = h.control(&json!({"op": "counters"})).unwrap();
        match body {
            Body::NotObject | Body::BadUtf8 | Body::Trailing => {
                prop_assert!(h.closed("main").is_some(), "the epoch ends");
                prop_assert!(out.is_empty());
            }
            Body::Stale => {
                prop_assert!(h.closed("main").is_none());
                prop_assert!(out.is_empty(), "no Fault for a stale frame");
                prop_assert_eq!(&counters["stale_dropped"], &json!(1));
            }
            Body::Current if ty < 0x80 => {
                prop_assert_eq!(out.len(), 1);
                prop_assert_eq!(out[0].1, 5);
                prop_assert_eq!(&out[0].2["code"], &json!("unsupported_frame"));
                prop_assert_eq!(&out[0].2["detail"]["type"], &json!(ty));
            }
            Body::Current => {
                prop_assert!(out.is_empty());
                prop_assert_eq!(&counters["ignored_advisory"], &json!(1));
            }
        }
    }

    /// HP WF-1: a `len` over `frame_bytes` ends the epoch from the header alone, whatever follows.
    #[test]
    fn a_len_over_the_limit_ends_the_epoch(seed in 0u64..32, extra in 1u32..100_000, tail in proptest::collection::vec(any::<u8>(), 0..8)) {
        let (mut h, _) = loaded_hub(seed, json!({}));
        let limit = test_limits(&Value::Null).unwrap().frame_bytes as u32;
        let mut bytes = (limit + extra).to_le_bytes().to_vec();
        bytes.extend(tail);
        h.write("main", &bytes).unwrap();
        drain(&mut h, "main");
        prop_assert!(h.closed("main").is_some());
    }

    /// Steward ruling R-4: a bulk-class call on lane 2, or any other call on lane 3, is `invalid_request` on the lane it came on; it is not a `Fault` and the epoch goes on.
    /// A call on its own lane is never refused `invalid_request`.
    #[test]
    fn a_call_on_the_wrong_lane_is_invalid_request(seed in 0u64..32, idx in 0usize..CALLS.len(), lane in 2u8..4) {
        let spec = &CALLS[idx];
        let (mut h, e) = loaded_hub(seed, json!({}));
        let c = host_call(e, 7, spec.name, json!("autonomous"));
        h.write("main", &wire(lane, 32, &c)).unwrap();
        let out = drain(&mut h, "main");
        prop_assert_eq!(out.len(), 1);
        prop_assert_eq!(out[0].1, 0x21, "a HostResult, never a Fault");
        prop_assert_eq!(out[0].0, lane, "on the lane of the call");
        let wrong = spec.bulk != (lane == 3);
        let kind = out[0].2["result"]["error"]["kind"].as_str().unwrap_or("");
        prop_assert_eq!(kind == "invalid_request", wrong);
        prop_assert!(h.closed("main").is_none());
    }
}

/// Design 7.5, R-1.3: FakePlugin against FakeHub in loopback, under every seed: handshake, an invocation with two host calls, a crash-free `Shutdown`.
#[test]
fn plugin_and_hub_loop_back() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/hp/fixtures/host_calls.json"
    ))
    .unwrap();
    for seed in 0..32 {
        let mut hub = FakeHub::new(seed);
        hub.configure(&json!({"calls": {
            "store.get": [{"result": {"ok": true, "value": {"doc": {}, "revision": "r1"}}}],
            "hub.identity": [{"result": {"ok": true, "value": {"hub_id": "hub-1", "display_name": "hub"}}}],
        }}))
        .unwrap();
        let mut plugin = FakePlugin::new(seed);
        plugin.configure(&json!({"fixture": fixture})).unwrap();
        let pump = |hub: &mut FakeHub, plugin: &mut FakePlugin| loop {
            let mut moved = false;
            while let Some(b) = hub.read("main") {
                plugin.write("main", &b).unwrap();
                moved = true;
            }
            while let Some(b) = plugin.read("main") {
                hub.write("main", &b).unwrap();
                moved = true;
            }
            // The channel is a pipe: the end of one side is the end of the other.
            if plugin.closed("main").is_some() && hub.closed("main").is_none() {
                hub.close_channel("main");
                moved = true;
            }
            if !moved {
                break;
            }
        };
        pump(&mut hub, &mut plugin);
        assert_eq!(
            hub.control(&json!({"op": "status"})).unwrap()["state"],
            "loaded",
            "seed {seed}"
        );
        hub.control(&json!({"op": "invoke", "handler": "h1"}))
            .unwrap();
        pump(&mut hub, &mut plugin);
        let done = hub.control(&json!({"op": "completions"})).unwrap();
        assert_eq!(
            done[0]["answer"],
            json!({"ok": true, "value": "done"}),
            "seed {seed}"
        );
        hub.control(&json!({"op": "unload"})).unwrap();
        pump(&mut hub, &mut plugin);
        assert_eq!(
            hub.control(&json!({"op": "status"})).unwrap()["state"],
            "unloaded",
            "seed {seed}"
        );
    }
}

/// HP IV-7: when the Hub processes a `Result` it first compares the deadline with its clock, so an expired deadline rejects the `Result` even though its timer wake has not
/// run yet. Whether a real Hub processed the `Result` first is its own order, so no transcript can assert this; the unit test pins the fake's branch.
#[test]
fn iv_7_expired_deadline_rejects_a_result_before_the_wake() {
    let (mut h, e) = loaded_hub(2, json!({"limits": {"dispatch_deadline": 100}}));
    h.control(&json!({"op": "invoke", "handler": "h1"}))
        .unwrap();
    let inv = drain(&mut h, "main")
        .into_iter()
        .find(|f| f.1 == 0x10)
        .unwrap();
    let mut r = frames()["result"].clone();
    r["epoch"] = json!(e);
    r["request_id"] = inv.2["request_id"].clone();
    // The Result is waiting; the clock passes the deadline before it is processed.
    h.write("main", &wire(1, 0x11, &r)).unwrap();
    h.advance_clock(100);
    let done = h.control(&json!({"op": "completions"})).unwrap();
    assert_eq!(done[0]["answer"]["error"]["kind"], "timed_out");
    assert_eq!(
        h.control(&json!({"op": "counters"})).unwrap()["late_results"],
        1
    );
}
