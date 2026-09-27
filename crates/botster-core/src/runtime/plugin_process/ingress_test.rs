//! Ingress accounting: exact credit returns, reply ids released once, and a
//! late call after generation retirement.

use std::sync::Arc;

use serde_json::json;

use super::{CallAdmission, Ingress, PluginIngress};
use crate::actor::{
    PluginCleanupScope, PluginInvocationRequest, PluginInvocationResult, PluginKey, PluginLoadSpec,
    PluginUnloadSpec,
};
use crate::engine::{
    CallId, DeliveryPool, PluginDeliveryQuota, PluginWorkerEngine, PluginWorkerRegistration,
};
use crate::runtime::plugin_process::protocol::{
    CreditGrants, HostCallFrame, HostCallKindFrame, LogFrame, PluginMessageBody,
};
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;

struct Idle;

impl PluginRuntime for Idle {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        PluginInvocationResult::Completed(crate::actor::PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: None,
        })
    }
}

fn plugin() -> PluginKey {
    PluginKey("ingress".to_string())
}

fn engine_and_pool(slots: usize) -> (PluginWorkerEngine, DeliveryPool) {
    let engine = PluginWorkerEngine::new();
    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin(),
            package: "ingress".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: serde_json::from_value(json!({
            "name": "ingress",
            "version": "0.1.0",
            "kind": "plugin",
            "botster": ">=0.1.0",
            "source": null,
            "capabilities": [],
            "entrypoints": [{ "runtime": "lua", "path": "plugin.lua", "bootstrap": false }],
            "dependencies": [],
            "features": [],
            "host_profile": null,
            "configuration": null,
            "runnable_entrypoints": []
        }))
        .expect("manifest"),
        runtime: Arc::new(Idle),
        handlers: Vec::new(),
        resources: Vec::new(),
    });
    let pool = engine
        .try_reserve_delivery(
            &plugin(),
            PluginDeliveryQuota {
                call_result_slots: slots,
                call_result_request_bytes: slots * 1024,
                call_result_completion_bytes: 4096,
                ordinary_completion_entries: 0,
                ordinary_completion_bytes: 0,
            },
        )
        .expect("pool");
    (engine, pool)
}

fn grants() -> CreditGrants {
    CreditGrants {
        ingress_bytes: 1000,
        reply_count: 1,
        reply_bytes: 500,
        log_count: 2,
        log_bytes: 300,
    }
}

fn body() -> PluginMessageBody {
    serde_json::from_value(json!(null)).expect("a message body")
}

fn call(call_id: u64) -> HostCallFrame {
    HostCallFrame {
        kind: HostCallKindFrame::Call {
            max_result_bytes: 10,
        },
        call_id,
        invocation_request_id: RequestId("r".to_string()),
        body: body(),
    }
}

fn reply(call_id: u64) -> HostCallFrame {
    HostCallFrame {
        kind: HostCallKindFrame::Reply,
        call_id,
        invocation_request_id: RequestId("r".to_string()),
        body: body(),
    }
}

fn log() -> LogFrame {
    LogFrame {
        dropped_since_last: 0,
        body: body(),
    }
}

#[test]
fn a_drain_returns_exactly_the_call_and_log_credit_it_frees() {
    let (_engine, pool) = engine_and_pool(2);
    let ingress = Ingress::new(grants());
    ingress.attach(pool).expect("attach");
    assert_eq!(ingress.host_call(call(1), 100), Ok(CallAdmission::Queued));
    assert_eq!(ingress.host_call(reply(2), 50), Ok(CallAdmission::Queued));
    ingress.log(log(), 30).expect("log");
    assert_eq!(ingress.held(), (100, 1, 30, 1));

    let (items, returned) = ingress.drain(16, 1000);
    assert_eq!(items.len(), 3);
    assert_eq!(
        (
            returned.ingress_bytes,
            returned.log_count,
            returned.log_bytes
        ),
        (100, 1, 30)
    );
    assert!(matches!(&items[1], PluginIngress::HostCall(reply) if reply.call_id == CallId(2)));
    assert_eq!(ingress.held(), (0, 0, 0, 1), "the reply keeps its credit");
    assert!(ingress.release_reply(CallId(2)));
    assert!(!ingress.release_reply(CallId(2)), "a reply releases once");
    assert_eq!(ingress.held(), (0, 0, 0, 0));
    // A released id holds no storage and may be used again.
    assert_eq!(ingress.host_call(reply(2), 50), Ok(CallAdmission::Queued));
}

#[test]
fn credit_is_charged_before_anything_is_queued() {
    let (_engine, pool) = engine_and_pool(1);
    let ingress = Ingress::new(grants());
    assert!(
        ingress.host_call(call(1), 10).is_err(),
        "no pool, no credit"
    );
    ingress.attach(pool.clone()).expect("attach");
    assert!(
        ingress.host_call(call(1), 1001).is_err(),
        "over the ingress bytes"
    );
    assert_eq!(pool.free(), (1, 1024), "a refused frame takes no unit");
    assert_eq!(ingress.host_call(call(1), 10), Ok(CallAdmission::Queued));
    assert!(ingress.host_call(call(2), 10).is_err(), "no free unit");
    assert!(
        ingress.host_call(reply(3), 501).is_err(),
        "over the reply allowance"
    );
    ingress.log(log(), 200).expect("first log");
    assert!(ingress.log(log(), 101).is_err(), "over the log bytes");
    assert_eq!(ingress.held(), (10, 1, 200, 0));
}

#[test]
fn a_call_after_the_generation_retired_is_dropped_not_a_violation() {
    let (engine, pool) = engine_and_pool(1);
    let ingress = Ingress::new(grants());
    ingress.attach(pool).expect("attach");
    engine.unload_plugin(PluginUnloadSpec {
        request_id: RequestId("unload".to_string()),
        plugin_key: plugin(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });
    assert_eq!(ingress.host_call(call(1), 10), Ok(CallAdmission::Dropped));
    assert_eq!(ingress.held(), (0, 0, 0, 0));
    assert!(ingress.drain(16, 1000).0.is_empty());
}
