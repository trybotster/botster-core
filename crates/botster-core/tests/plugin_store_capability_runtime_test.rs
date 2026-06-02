//! Plugin-store capability runtime acceptance tests.

use std::collections::BTreeSet;
use std::sync::{Arc, Barrier};
use std::thread;

use botster_core::{
    BoundaryJson, Capability, CapabilityOperation, CapabilityOperationCompleted,
    CapabilityOperationFailure, CapabilityOperationId, CapabilityRuntimeErrorKind,
    CapabilityRuntimeEvent, CapabilityRuntimeRequest, CapabilitySet, CapabilitySurface,
    ExtensionEntrypoint, ExtensionKind, ExtensionRuntime, PackageManifest, PluginCapabilityRuntime,
    PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginHandlerRegistration, PluginKey, PluginLoadSpec, PluginOwnedDescriptor,
    PluginResourceKind, PluginResourceRef, PluginRuntime, PluginStoreBackend,
    PluginStoreCapabilityRequest, PluginStoreKey, PluginStoreLimits, PluginStoreOperation,
    PluginStoreResult, PluginUnloadSpec, PluginWorkerEngine, PluginWorkerRegistration, RequestId,
};
use botster_core_test_support::fake::{
    FakePluginBehavior, FakePluginRuntime, FakePluginStoreBackend, FakePluginStoreCapabilityRuntime,
};

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn operation_id(value: &str) -> CapabilityOperationId {
    CapabilityOperationId(value.to_string())
}

fn store_capability(namespace: &str) -> Capability {
    Capability {
        surface: CapabilitySurface::PluginDb,
        scope: Some(namespace.to_string()),
    }
}

fn capability_set(capabilities: Vec<Capability>) -> CapabilitySet {
    capabilities.into_iter().collect::<BTreeSet<_>>()
}

fn store_request(
    plugin_key: &PluginKey,
    id: &str,
    operation: PluginStoreOperation,
) -> CapabilityRuntimeRequest {
    CapabilityRuntimeRequest {
        plugin_key: plugin_key.clone(),
        operation_id: operation_id(id),
        operation: CapabilityOperation::PluginStore(PluginStoreCapabilityRequest {
            namespace: plugin_key.0.clone(),
            operation,
        }),
        timeout_ms: 250,
        callback: None,
    }
}

fn completed_plugin_store(event: CapabilityRuntimeEvent) -> PluginStoreResult {
    match event {
        CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            plugin_store: Some(result),
            ..
        }) => result,
        other => panic!("expected plugin-store completion, got {other:?}"),
    }
}

fn failed_plugin_store(event: CapabilityRuntimeEvent) -> CapabilityRuntimeErrorKind {
    match event {
        CapabilityRuntimeEvent::Failed(CapabilityOperationFailure { error_kind, .. }) => error_kind,
        other => panic!("expected plugin-store failure, got {other:?}"),
    }
}

#[test]
fn plugin_store_capability_requests_require_plugin_db_scope_before_acceptance() {
    let plugin = plugin_key("project-pipelines");
    let operation = PluginStoreOperation::Set {
        key: PluginStoreKey("runs/active".to_string()),
        schema_version: 1,
        payload: serde_json::json!({ "state": "running" }),
        expected_revision: None,
    };
    let mut denied = FakePluginStoreCapabilityRuntime::new(capability_set(Vec::new()));
    let error = denied
        .submit(store_request(&plugin, "set-denied", operation.clone()))
        .expect_err("missing PluginDb capability should deny before acceptance");

    assert_eq!(error.kind, CapabilityRuntimeErrorKind::CapabilityDenied);
    assert!(denied
        .drain_events(&plugin)
        .expect("drain denied")
        .is_empty());

    let mut accepted =
        FakePluginStoreCapabilityRuntime::new(capability_set(vec![store_capability(&plugin.0)]));
    let handle = accepted
        .submit(store_request(&plugin, "set-accepted", operation))
        .expect("PluginDb scope accepts request");
    assert_eq!(handle.required_capability, store_capability(&plugin.0));
}

#[test]
fn plugin_store_capability_requests_are_accepted_before_deferred_completion() {
    let plugin = plugin_key("project-pipelines");
    let backend = FakePluginStoreBackend::new();
    let mut runtime = FakePluginStoreCapabilityRuntime::with_backend_and_limits(
        backend.clone(),
        capability_set(vec![store_capability(&plugin.0)]),
        PluginStoreLimits::default(),
    );

    let handle = runtime
        .submit(store_request(
            &plugin,
            "set-1",
            PluginStoreOperation::Set {
                key: PluginStoreKey("runs/active".to_string()),
                schema_version: 1,
                payload: serde_json::json!({ "state": "queued" }),
                expected_revision: None,
            },
        ))
        .expect("submit returns handle");

    assert_eq!(handle.operation_id, operation_id("set-1"));
    assert!(
        backend.records_for(&plugin).is_empty(),
        "fake backend work is deferred until events are drained"
    );

    let events = runtime.drain_events(&plugin).expect("drain events");
    assert_eq!(events.len(), 1);
    assert_eq!(backend.records_for(&plugin).len(), 1);
}

#[test]
fn plugin_store_isolates_namespaces_by_plugin_key_and_lists_deterministically() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let mut runtime = FakePluginStoreCapabilityRuntime::new(capability_set(vec![
        store_capability(&plugin_a.0),
        store_capability(&plugin_b.0),
    ]));

    for plugin in [&plugin_a, &plugin_b] {
        runtime
            .submit(store_request(
                plugin,
                &format!("set-{}", plugin.0),
                PluginStoreOperation::Set {
                    key: PluginStoreKey("shared/key".to_string()),
                    schema_version: 1,
                    payload: serde_json::json!({ "owner": plugin.0 }),
                    expected_revision: None,
                },
            ))
            .expect("submit namespace record");
        runtime.drain_events(plugin).expect("complete set");
    }

    runtime
        .submit(store_request(
            &plugin_a,
            "list-a",
            PluginStoreOperation::List { prefix: None },
        ))
        .expect("submit list");
    let result = completed_plugin_store(
        runtime
            .drain_events(&plugin_a)
            .expect("list events")
            .pop()
            .expect("list event"),
    );

    assert!(matches!(
        result,
        PluginStoreResult::List { entries }
            if entries.len() == 1 && entries[0].key == PluginStoreKey("shared/key".to_string())
    ));

    let cross_namespace = CapabilityRuntimeRequest {
        operation: CapabilityOperation::PluginStore(PluginStoreCapabilityRequest {
            namespace: plugin_b.0.clone(),
            operation: PluginStoreOperation::Get {
                key: PluginStoreKey("shared/key".to_string()),
            },
        }),
        ..store_request(
            &plugin_a,
            "cross-get",
            PluginStoreOperation::List { prefix: None },
        )
    };
    let error = runtime
        .submit(cross_namespace)
        .expect_err("explicit namespace mismatch denied before acceptance");
    assert_eq!(error.kind, CapabilityRuntimeErrorKind::InvalidRequest);
}

#[test]
fn plugin_store_crud_round_trips_json_envelope() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime =
        FakePluginStoreCapabilityRuntime::new(capability_set(vec![store_capability(&plugin.0)]));
    let key = PluginStoreKey("runs/active".to_string());

    runtime
        .submit(store_request(
            &plugin,
            "set-1",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 7,
                payload: serde_json::json!({ "state": "running" }),
                expected_revision: None,
            },
        ))
        .expect("submit set");
    let result =
        completed_plugin_store(runtime.drain_events(&plugin).expect("set event").remove(0));
    assert!(matches!(
        result,
        PluginStoreResult::Written { record }
            if record.plugin_key == plugin
                && record.key == key
                && record.schema_version == 7
                && record.revision == 1
    ));

    runtime
        .submit(store_request(
            &plugin,
            "get-1",
            PluginStoreOperation::Get { key: key.clone() },
        ))
        .expect("submit get");
    assert!(matches!(
        completed_plugin_store(runtime.drain_events(&plugin).expect("get event").remove(0)),
        PluginStoreResult::Record { record }
            if record.payload == serde_json::json!({ "state": "running" })
    ));

    runtime
        .submit(store_request(
            &plugin,
            "delete-1",
            PluginStoreOperation::Delete { key: key.clone() },
        ))
        .expect("submit delete");
    assert!(matches!(
        completed_plugin_store(
            runtime
                .drain_events(&plugin)
                .expect("delete event")
                .remove(0)
        ),
        PluginStoreResult::Deleted { revision: 1, .. }
    ));

    runtime
        .submit(store_request(
            &plugin,
            "get-missing",
            PluginStoreOperation::Get { key },
        ))
        .expect("submit missing get");
    assert_eq!(
        failed_plugin_store(
            runtime
                .drain_events(&plugin)
                .expect("missing event")
                .remove(0)
        ),
        CapabilityRuntimeErrorKind::StoreNotFound
    );
}

#[test]
fn plugin_store_merge_patch_uses_rfc_7396_style_semantics() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime =
        FakePluginStoreCapabilityRuntime::new(capability_set(vec![store_capability(&plugin.0)]));
    let key = PluginStoreKey("config".to_string());

    runtime
        .submit(store_request(
            &plugin,
            "set-config",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 1,
                payload: serde_json::json!({
                    "nested": { "keep": true, "remove": true },
                    "array": [1, 2]
                }),
                expected_revision: None,
            },
        ))
        .expect("submit set");
    runtime.drain_events(&plugin).expect("complete set");

    runtime
        .submit(store_request(
            &plugin,
            "patch-config",
            PluginStoreOperation::Patch {
                key: key.clone(),
                patch: serde_json::json!({
                    "nested": { "remove": null, "add": "yes" },
                    "array": [3],
                    "new": "field"
                }),
                expected_revision: Some(1),
            },
        ))
        .expect("submit patch");

    assert!(matches!(
        completed_plugin_store(runtime.drain_events(&plugin).expect("patch event").remove(0)),
        PluginStoreResult::Written { record }
            if record.revision == 2
                && record.payload == serde_json::json!({
                    "nested": { "keep": true, "add": "yes" },
                    "array": [3],
                    "new": "field"
                })
    ));

    let error = runtime
        .submit(store_request(
            &plugin,
            "bad-patch",
            PluginStoreOperation::Patch {
                key,
                patch: serde_json::json!(["not", "an", "object"]),
                expected_revision: None,
            },
        ))
        .expect_err("non-object merge patch denied before acceptance");
    assert_eq!(error.kind, CapabilityRuntimeErrorKind::PatchFailed);
}

#[test]
fn plugin_store_compare_and_swap_rejects_stale_revision_without_mutation() {
    let plugin = plugin_key("project-pipelines");
    let backend = FakePluginStoreBackend::new();
    let mut runtime = FakePluginStoreCapabilityRuntime::with_backend_and_limits(
        backend.clone(),
        capability_set(vec![store_capability(&plugin.0)]),
        PluginStoreLimits::default(),
    );
    let key = PluginStoreKey("runs/active".to_string());

    runtime
        .submit(store_request(
            &plugin,
            "set-1",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 1,
                payload: serde_json::json!({ "state": "one" }),
                expected_revision: None,
            },
        ))
        .expect("submit set");
    runtime.drain_events(&plugin).expect("complete set");

    runtime
        .submit(store_request(
            &plugin,
            "set-2",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 1,
                payload: serde_json::json!({ "state": "two" }),
                expected_revision: Some(1),
            },
        ))
        .expect("submit cas set");
    assert!(matches!(
        completed_plugin_store(runtime.drain_events(&plugin).expect("cas event").remove(0)),
        PluginStoreResult::Written { record } if record.revision == 2
    ));

    runtime
        .submit(store_request(
            &plugin,
            "stale-set",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 1,
                payload: serde_json::json!({ "state": "stale" }),
                expected_revision: Some(1),
            },
        ))
        .expect("submit stale set");
    assert_eq!(
        failed_plugin_store(
            runtime
                .drain_events(&plugin)
                .expect("stale event")
                .remove(0)
        ),
        CapabilityRuntimeErrorKind::RevisionConflict
    );
    assert_eq!(
        backend
            .get(&plugin, &key)
            .expect("get backend record")
            .expect("record exists")
            .payload,
        serde_json::json!({ "state": "two" })
    );
}

#[test]
fn plugin_store_concurrent_writers_surface_one_revision_conflict() {
    let plugin = plugin_key("project-pipelines");
    let backend = FakePluginStoreBackend::new();
    let key = PluginStoreKey("race".to_string());
    backend
        .set(
            &plugin,
            key.clone(),
            1,
            serde_json::json!({ "writer": "initial" }),
            None,
            PluginStoreLimits::default(),
        )
        .expect("seed backend");
    let barrier = Arc::new(Barrier::new(2));

    let handles = ["a", "b"].map(|writer| {
        let backend = backend.clone();
        let plugin = plugin.clone();
        let key = key.clone();
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            backend.set(
                &plugin,
                key,
                1,
                serde_json::json!({ "writer": writer }),
                Some(1),
                PluginStoreLimits::default(),
            )
        })
    });

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("writer thread"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(error) if error.kind == CapabilityRuntimeErrorKind::RevisionConflict
                )
            })
            .count(),
        1
    );
}

#[test]
fn plugin_store_quotas_reject_oversized_record_and_patch() {
    let plugin = plugin_key("project-pipelines");
    let backend = FakePluginStoreBackend::new();
    let mut runtime = FakePluginStoreCapabilityRuntime::with_backend_and_limits(
        backend.clone(),
        capability_set(vec![store_capability(&plugin.0)]),
        PluginStoreLimits {
            max_record_bytes: 24,
            max_plugin_keys: 2,
            max_plugin_bytes: 64,
        },
    );
    let key = PluginStoreKey("small".to_string());

    runtime
        .submit(store_request(
            &plugin,
            "too-large",
            PluginStoreOperation::Set {
                key: PluginStoreKey("large".to_string()),
                schema_version: 1,
                payload: serde_json::json!({ "value": "this string is too long" }),
                expected_revision: None,
            },
        ))
        .expect("submit too-large set");
    assert_eq!(
        failed_plugin_store(
            runtime
                .drain_events(&plugin)
                .expect("quota event")
                .remove(0)
        ),
        CapabilityRuntimeErrorKind::QuotaExceeded
    );

    runtime
        .submit(store_request(
            &plugin,
            "set-small",
            PluginStoreOperation::Set {
                key: key.clone(),
                schema_version: 1,
                payload: serde_json::json!({ "v": "ok" }),
                expected_revision: None,
            },
        ))
        .expect("submit small set");
    runtime.drain_events(&plugin).expect("complete set");
    let before = backend.get(&plugin, &key).expect("backend get");

    runtime
        .submit(store_request(
            &plugin,
            "patch-too-large",
            PluginStoreOperation::Patch {
                key: key.clone(),
                patch: serde_json::json!({ "oversized": "this patch makes the record too large" }),
                expected_revision: Some(1),
            },
        ))
        .expect("submit oversized patch");
    assert_eq!(
        failed_plugin_store(
            runtime
                .drain_events(&plugin)
                .expect("patch quota event")
                .remove(0)
        ),
        CapabilityRuntimeErrorKind::QuotaExceeded
    );
    assert_eq!(backend.get(&plugin, &key).expect("backend get"), before);
}

#[test]
fn plugin_store_unload_reload_does_not_cross_corrupt_namespaces() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let backend = FakePluginStoreBackend::new();
    backend
        .set(
            &plugin_a,
            PluginStoreKey("state".to_string()),
            1,
            serde_json::json!({ "owner": "a" }),
            None,
            PluginStoreLimits::default(),
        )
        .expect("seed plugin a");
    backend
        .set(
            &plugin_b,
            PluginStoreKey("state".to_string()),
            1,
            serde_json::json!({ "owner": "b" }),
            None,
            PluginStoreLimits::default(),
        )
        .expect("seed plugin b");

    let engine = PluginWorkerEngine::new();
    let handler_a = handler(&plugin_a, "advance");
    let handler_b = handler(&plugin_b, "render");
    engine.load_plugin(registration(&plugin_a, handler_a.clone()));
    engine.load_plugin(registration(&plugin_b, handler_b));
    engine.record_resource(PluginResourceRef {
        plugin_key: plugin_a.clone(),
        kind: PluginResourceKind::PluginStoreOperation,
        resource_id: "store-a".to_string(),
    });
    engine.record_resource(PluginResourceRef {
        plugin_key: plugin_b.clone(),
        kind: PluginResourceKind::PluginStoreOperation,
        resource_id: "store-b".to_string(),
    });

    let cleanup = engine.unload_plugin(PluginUnloadSpec {
        request_id: request_id("unload-a"),
        plugin_key: plugin_a.clone(),
        cleanup: PluginCleanupScope::DescriptorsAndResources,
    });

    assert_eq!(cleanup.plugin_key, plugin_a);
    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin_a));
    assert_eq!(cleanup.removed_resources.len(), 1);
    assert_eq!(
        backend.records_for(&plugin_b)[0].payload,
        serde_json::json!({ "owner": "b" })
    );
    assert_eq!(
        backend.records_for(&plugin_a)[0].payload,
        serde_json::json!({ "owner": "a" })
    );
}

#[test]
fn fake_plugin_store_backend_supports_consumer_conformance_tests() {
    let plugin = plugin_key("consumer");
    let backend = FakePluginStoreBackend::new();
    let record = backend
        .set(
            &plugin,
            PluginStoreKey("settings".to_string()),
            2,
            serde_json::json!({ "enabled": true }),
            None,
            PluginStoreLimits::default(),
        )
        .expect("fake backend set");

    assert_eq!(record.revision, 1);
    assert_eq!(
        backend
            .list(&plugin, Some("set"))
            .expect("fake backend list")[0]
            .schema_version,
        2
    );
}

#[test]
fn plugin_store_core_has_no_hub_rails_or_sqlite_dependency() {
    let manifest = include_str!("../Cargo.toml");

    for forbidden in ["rails", "sqlite", "rusqlite", "sqlx", "botster-hub"] {
        assert!(
            !manifest.contains(forbidden),
            "botster-core must not link {forbidden} into plugin-store runtime"
        );
    }
}

fn handler(plugin_key: &PluginKey, id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: id.to_string(),
    }
}

fn registration(plugin_key: &PluginKey, handler: PluginHandlerRef) -> PluginWorkerRegistration {
    PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin_key.clone(),
            package: plugin_key.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: vec![PluginOwnedDescriptor {
                descriptor: PluginDescriptorRef {
                    plugin_key: plugin_key.clone(),
                    kind: PluginDescriptorKind::Command,
                    descriptor_id: handler.handler_id.clone(),
                },
                handler: Some(handler.clone()),
                body: BoundaryJson(serde_json::json!({ "id": handler.handler_id })),
            }],
            metadata: None,
        },
        manifest: PackageManifest {
            name: plugin_key.0.clone(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::Plugin,
            botster: ">=0.1.0".to_string(),
            source: None,
            capabilities: vec![store_capability(&plugin_key.0)],
            entrypoints: vec![ExtensionEntrypoint {
                runtime: ExtensionRuntime::Lua,
                path: "plugin.lua".to_string(),
                bootstrap: false,
            }],
        },
        runtime: Arc::new(FakePluginRuntime::new(FakePluginBehavior::Success(
            BoundaryJson(serde_json::json!({ "value": "ok" })),
        ))) as Arc<dyn PluginRuntime + Send + Sync>,
        handlers: vec![PluginHandlerRegistration {
            handler,
            required_capability: Some(store_capability(&plugin_key.0)),
        }],
        resources: Vec::new(),
    }
}
