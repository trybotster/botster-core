//! Shared builders for the plugin-contract tests.
#![allow(dead_code)]

use botster_plugin_contract::prelude::*;
use serde_json::{json, Map, Value};

pub const MAX_KEY_REPEAT: u64 = 8;

/// A valid set of limits (HP section 15): every numeric value is inside its range.
pub fn sample_limits() -> PluginLimits {
    let mut m = Map::new();
    for s in LIMIT_SPECS {
        let v = match s.range {
            Range::Between(lo, _) => lo,
            _ => 1_000_000,
        };
        m.insert(s.name.to_string(), json!(v));
    }
    let lane = json!({"frames": 8, "bytes": 65536});
    for (k, v) in [
        ("frame_bytes", json!(1_048_576)),
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
    ] {
        m.insert(k.to_string(), v);
    }
    serde_json::from_value(Value::Object(m)).expect("sample limits parse")
}

pub fn hello() -> Hello {
    Hello {
        epoch: Epoch(7),
        protocol_version: 1,
        min_protocol_version: 1,
        features: vec![FEATURE_MCP_PROXY.into()],
        grants: vec![Grant {
            surface: "events".into(),
            scope: Some("events_hub".into()),
        }],
        limits: sample_limits(),
        bindings: vec![],
        priorities: v1_priorities(),
        instance: "inst-1".into(),
        plugin_key: "demo".into(),
        resumed: false,
        hub_id: "hub-1".into(),
    }
}

pub fn ready() -> Ready {
    Ready {
        epoch: Epoch(7),
        protocol_version: 1,
        handlers: vec![HandlerDecl {
            id: "t".into(),
            kind: HandlerKind::Timer,
            descriptor: json!({}),
        }],
        tools: vec![],
        prompts: vec![],
        resources: vec![],
        events: EventsDecl::default(),
        entity_providers: vec![],
        timers: vec![TimerDecl {
            timer_id: "tick".into(),
            delay_ms: 1000,
            handler_id: Some("t".into()),
        }],
        watches: vec![],
        fs_watches: vec![],
        session_actions: vec![],
        operations: OperationsDecl::default(),
        grants: vec![],
        host: HostInfo {
            name: "fake".into(),
            version: "1".into(),
            globals: vec![],
        },
        features: Features::default(),
    }
}

pub fn session() -> SessionRef {
    SessionRef {
        hub_id: "hub-1".into(),
        session_id: "s1".into(),
    }
}

pub fn invoke() -> Invoke {
    Invoke {
        epoch: Epoch(7),
        request_id: WireId::Num(1),
        handler: "t".into(),
        kind: HandlerKind::Timer,
        payload: json!({"timer_id": "tick"}),
        caller: Caller::System,
        origin: Origin("timer".into()),
        source: None,
        descriptor_id: "d1".into(),
        deadline_ms: 5000,
    }
}

pub fn host_call(call: &str) -> HostCall {
    HostCall {
        epoch: Epoch(7),
        call_id: WireId::Str("c1".into()),
        context: CallContext::Request(WireId::Num(1)),
        call: call.into(),
        args: json!({}),
        max_result_bytes: 4096,
    }
}

/// One sample of every frame type.
pub fn every_frame() -> Vec<Frame> {
    vec![
        Frame::Hello(hello()),
        Frame::Ready(ready()),
        Frame::Activated(Activated { epoch: Epoch(7) }),
        Frame::Shutdown(Shutdown {
            epoch: Epoch(7),
            deadline_ms: 100,
            reason: "unload".into(),
        }),
        Frame::Fault(Fault {
            epoch: Epoch(7),
            code: "unsupported_frame".into(),
            detail: Some(json!({"type": 66})),
        }),
        Frame::Cancel(Cancel {
            epoch: Epoch(7),
            request_id: WireId::Num(1),
            reason: "deadline".into(),
        }),
        Frame::Invoke(invoke()),
        Frame::Result(ResultFrame {
            epoch: Epoch(7),
            request_id: WireId::Num(1),
            result: PluginResult::Ok(json!({})),
        }),
        Frame::HostCall(host_call("store.get")),
        Frame::HostResult(HostResult {
            epoch: Epoch(7),
            call_id: WireId::Str("c1".into()),
            result: PluginResult::Err(ErrorBody::new(ErrorKind::NotFound, "no such document")),
            effect: Effect::None,
        }),
    ]
}

/// Wire bytes for a raw payload.
pub fn wire(lane: u8, ty: u8, payload: &[u8]) -> Vec<u8> {
    encode_raw(lane, ty, payload, u32::MAX).expect("encodes")
}
