//! Plugin capability runtime contract acceptance tests.

use botster_core::{
    BackpressureSummary, Capability, CapabilityOperation, CapabilityOperationCompleted,
    CapabilityOperationFailure, CapabilityOperationId, CapabilityOperationResult,
    CapabilityResourceEvent, CapabilityResourceId, CapabilityRuntimeEvent,
    CapabilityRuntimeRequest, CapabilitySurface, CapabilityTimerEvent, CapabilityWatchEvent,
    CapabilityWebSocketEvent, FilesystemCapabilityGrant, FilesystemCapabilityLimits,
    FilesystemCapabilityPermissions, FilesystemCapabilityRequest, FilesystemCapabilityResult,
    FilesystemEntry, FilesystemEntryKind, FilesystemMetadata, FilesystemOperation,
    HttpCapabilityRequest, HttpCapabilityResponse, HttpHeader, PluginCleanupResult,
    PluginHandlerKind, PluginHandlerRef, PluginKey, PluginResourceKind, PluginResourceRef,
    PluginStoreCapabilityRequest, PluginStoreKey, PluginStoreOperation, QueueSource, RequestId,
    ScopedRelativePath, TimerCapabilityRequest, WatchCapabilityRequest, WatchChangeKind,
    WebSocketCapabilityRequest, WebSocketMessage,
};

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn operation_id(id: &str) -> CapabilityOperationId {
    CapabilityOperationId(id.to_string())
}

fn resource_id(id: &str) -> CapabilityResourceId {
    CapabilityResourceId(id.to_string())
}

fn callback(plugin: &PluginKey, id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin.clone(),
        kind: PluginHandlerKind::Http,
        handler_id: id.to_string(),
    }
}

fn request(
    plugin: &PluginKey,
    id: &str,
    operation: CapabilityOperation,
) -> CapabilityRuntimeRequest {
    CapabilityRuntimeRequest {
        plugin_key: plugin.clone(),
        operation_id: operation_id(id),
        operation,
        timeout_ms: 250,
        callback: Some(callback(plugin, "capability-result")),
    }
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize capability runtime value");
    serde_json::from_str(&json).expect("deserialize capability runtime value")
}

#[test]
fn every_operation_family_round_trips_and_declares_required_capability() {
    let plugin = plugin_key("project-pipelines");
    let requests = [
        request(
            &plugin,
            "http-1",
            CapabilityOperation::Http(HttpCapabilityRequest {
                method: "GET".to_string(),
                endpoint: "status-api".to_string(),
                headers: vec![HttpHeader {
                    name: "Accept".to_string(),
                    value: "application/json".to_string(),
                }],
                body: Vec::new(),
            }),
        ),
        request(
            &plugin,
            "ws-1",
            CapabilityOperation::WebSocket(WebSocketCapabilityRequest::Connect {
                endpoint: "events-feed".to_string(),
                protocols: vec!["botster.events.v1".to_string()],
            }),
        ),
        request(
            &plugin,
            "watch-1",
            CapabilityOperation::Watch(WatchCapabilityRequest::Register {
                scope_id: "workspace".to_string(),
                path: ScopedRelativePath("src".to_string()),
                recursive: true,
            }),
        ),
        request(
            &plugin,
            "fs-1",
            CapabilityOperation::Filesystem(FilesystemCapabilityRequest {
                scope_id: "workspace".to_string(),
                operation: FilesystemOperation::Read {
                    path: ScopedRelativePath("README.md".to_string()),
                },
                limits: Some(FilesystemCapabilityLimits {
                    max_read_bytes: Some(65_536),
                    max_write_bytes: None,
                    max_list_entries: None,
                }),
            }),
        ),
        request(
            &plugin,
            "store-1",
            CapabilityOperation::PluginStore(PluginStoreCapabilityRequest {
                namespace: "project-pipelines".to_string(),
                operation: PluginStoreOperation::Set {
                    key: PluginStoreKey("runs/active".to_string()),
                    value: serde_json::json!({ "state": "running" }),
                },
            }),
        ),
        request(
            &plugin,
            "timer-1",
            CapabilityOperation::Timer(TimerCapabilityRequest::Interval { interval_ms: 1_000 }),
        ),
    ];

    let expected = [
        Capability {
            surface: CapabilitySurface::Network,
            scope: Some("http".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Network,
            scope: Some("websocket".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Filesystem,
            scope: Some("workspace".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Filesystem,
            scope: Some("workspace".to_string()),
        },
        Capability {
            surface: CapabilitySurface::PluginDb,
            scope: Some("project-pipelines".to_string()),
        },
        Capability {
            surface: CapabilitySurface::Timers,
            scope: Some("callbacks".to_string()),
        },
    ];

    let resource_kinds = [
        PluginResourceKind::HttpRequest,
        PluginResourceKind::NetworkConnection,
        PluginResourceKind::Watch,
        PluginResourceKind::FilesystemOperation,
        PluginResourceKind::PluginStoreOperation,
        PluginResourceKind::Timer,
    ];

    for ((request, capability), resource_kind) in requests.iter().zip(expected).zip(resource_kinds)
    {
        assert_eq!(round_trip(request), *request);
        assert_eq!(request.required_capability(), capability);
        assert_eq!(request.resource_kind(), resource_kind);
        assert_eq!(
            request.resource_ref(resource_id("resource-1")).plugin_key,
            plugin
        );
    }
}

#[test]
fn scoped_filesystem_paths_are_relative_contracts_not_host_policy() {
    let scoped = ScopedRelativePath("logs/session.log".to_string());
    let absolute = ScopedRelativePath("/tmp/secret.txt".to_string());
    let traversal = ScopedRelativePath("../outside".to_string());
    let nested_traversal = ScopedRelativePath("logs/../outside".to_string());
    let drive_absolute = ScopedRelativePath("C:\\secret.txt".to_string());
    let drive_relative = ScopedRelativePath("C:secret.txt".to_string());
    let unc = ScopedRelativePath("\\\\server\\share\\secret.txt".to_string());

    assert!(scoped.is_scoped_relative());
    assert!(!absolute.is_scoped_relative());
    assert!(!traversal.is_scoped_relative());
    assert!(!nested_traversal.is_scoped_relative());
    assert!(!drive_absolute.is_scoped_relative());
    assert!(!drive_relative.is_scoped_relative());
    assert!(!unc.is_scoped_relative());

    let request = FilesystemCapabilityRequest {
        scope_id: "workspace".to_string(),
        operation: FilesystemOperation::Write {
            path: scoped.clone(),
            bytes: b"ok".to_vec(),
        },
        limits: Some(FilesystemCapabilityLimits {
            max_read_bytes: None,
            max_write_bytes: Some(1024),
            max_list_entries: None,
        }),
    };

    assert_eq!(request.operation.path(), &scoped);
    assert_eq!(round_trip(&request).operation, request.operation);
}

#[test]
fn scoped_filesystem_grants_limits_and_results_are_typed_contracts() {
    let grant = FilesystemCapabilityGrant {
        scope_id: "workspace".to_string(),
        permissions: FilesystemCapabilityPermissions {
            read: true,
            write: true,
            list: true,
            stat: true,
            remove: false,
        },
        limits: Some(FilesystemCapabilityLimits {
            max_read_bytes: Some(65_536),
            max_write_bytes: Some(16_384),
            max_list_entries: Some(256),
        }),
    };

    assert_eq!(round_trip(&grant), grant);

    let path = ScopedRelativePath("src/lib.rs".to_string());
    let result = CapabilityOperationResult::Filesystem(FilesystemCapabilityResult::List {
        path: ScopedRelativePath("src".to_string()),
        entries: vec![FilesystemEntry {
            path: path.clone(),
            kind: FilesystemEntryKind::File,
            size_bytes: Some(1234),
        }],
    });

    let stat = CapabilityOperationResult::Filesystem(FilesystemCapabilityResult::Stat {
        path,
        metadata: FilesystemMetadata {
            kind: FilesystemEntryKind::File,
            size_bytes: Some(1234),
            readonly: false,
        },
    });

    assert_eq!(round_trip(&result), result);
    assert_eq!(round_trip(&stat), stat);
}

#[test]
fn scoped_filesystem_permissions_gate_each_operation_kind() {
    let read_only = FilesystemCapabilityPermissions {
        read: true,
        write: false,
        list: false,
        stat: false,
        remove: false,
    };
    let path = ScopedRelativePath("README.md".to_string());

    assert!(read_only.allows(&FilesystemOperation::Read { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Write {
        path: path.clone(),
        bytes: b"denied".to_vec(),
    }));
    assert!(!read_only.allows(&FilesystemOperation::List { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Stat { path: path.clone() }));
    assert!(!read_only.allows(&FilesystemOperation::Remove { path }));

    let list_and_stat = FilesystemCapabilityPermissions {
        read: false,
        write: false,
        list: true,
        stat: true,
        remove: false,
    };
    let path = ScopedRelativePath("src".to_string());

    assert!(!list_and_stat.allows(&FilesystemOperation::Read { path: path.clone() }));
    assert!(list_and_stat.allows(&FilesystemOperation::List { path: path.clone() }));
    assert!(list_and_stat.allows(&FilesystemOperation::Stat { path }));
}

#[test]
fn events_round_trip_with_plugin_identity_operation_ids_and_pressure_route() {
    let plugin = plugin_key("project-pipelines");
    let websocket = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::NetworkConnection,
        resource_id: "ws-1".to_string(),
    };
    let watch = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::Watch,
        resource_id: "watch-1".to_string(),
    };
    let timer = PluginResourceRef {
        plugin_key: plugin.clone(),
        kind: PluginResourceKind::Timer,
        resource_id: "timer-1".to_string(),
    };
    let request = request(
        &plugin,
        "http-1",
        CapabilityOperation::Http(HttpCapabilityRequest {
            method: "GET".to_string(),
            endpoint: "status-api".to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }),
    );
    let pressure: BackpressureSummary = request.backpressure(256, 250);

    let events = vec![
        CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            plugin_key: plugin.clone(),
            operation_id: operation_id("http-1"),
            result: Some(CapabilityOperationResult::Http(HttpCapabilityResponse {
                status: 200,
                headers: Vec::new(),
                body: b"{}".to_vec(),
            })),
        }),
        CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
            plugin_key: plugin.clone(),
            operation_id: operation_id("fs-1"),
            result: Some(CapabilityOperationResult::Filesystem(
                FilesystemCapabilityResult::Read {
                    path: ScopedRelativePath("README.md".to_string()),
                    bytes: b"hello".to_vec(),
                },
            )),
        }),
        CapabilityRuntimeEvent::ResourceOpened(CapabilityResourceEvent {
            plugin_key: plugin.clone(),
            operation_id: operation_id("ws-1"),
            resource: websocket.clone(),
        }),
        CapabilityRuntimeEvent::WebSocketMessage(CapabilityWebSocketEvent {
            resource: websocket,
            message: WebSocketMessage::Text("updated".to_string()),
        }),
        CapabilityRuntimeEvent::Watch(CapabilityWatchEvent {
            resource: watch,
            path: ScopedRelativePath("src/main.rs".to_string()),
            change: WatchChangeKind::Modified,
        }),
        CapabilityRuntimeEvent::TimerFired(CapabilityTimerEvent {
            resource: timer,
            sequence: 3,
        }),
        CapabilityRuntimeEvent::TimedOut(CapabilityOperationFailure {
            plugin_key: plugin.clone(),
            operation_id: operation_id("slow-1"),
            error_kind: botster_core::CapabilityRuntimeErrorKind::TimedOut,
            reason: "operation exceeded timeout".to_string(),
        }),
        CapabilityRuntimeEvent::Backpressure(pressure.clone()),
    ];

    for event in events {
        assert_eq!(round_trip(&event), event);
    }
    assert_eq!(pressure.source, QueueSource::PluginWorker);
    assert_eq!(pressure.route.plugin_key, Some(plugin));
}

#[test]
fn cleanup_events_can_target_one_plugins_runtime_resources() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let cleanup = PluginCleanupResult {
        request_id: RequestId("cleanup-1".to_string()),
        plugin_key: plugin_a.clone(),
        removed_descriptors: Vec::new(),
        removed_resources: vec![
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::NetworkConnection,
                resource_id: "ws-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::Watch,
                resource_id: "watch-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::Timer,
                resource_id: "timer-1".to_string(),
            },
            PluginResourceRef {
                plugin_key: plugin_a.clone(),
                kind: PluginResourceKind::PluginStoreOperation,
                resource_id: "store-1".to_string(),
            },
        ],
    };

    assert!(cleanup
        .removed_resources
        .iter()
        .all(|resource| resource.plugin_key == plugin_a));
    assert!(!cleanup
        .removed_resources
        .iter()
        .any(|resource| resource.plugin_key == plugin_b));
    assert_eq!(
        round_trip(&CapabilityRuntimeEvent::CleanupCompleted(cleanup.clone())),
        CapabilityRuntimeEvent::CleanupCompleted(cleanup)
    );
}
