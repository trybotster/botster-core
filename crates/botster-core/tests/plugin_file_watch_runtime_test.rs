//! File watch runtime behavior tests.

use botster_core::{
    BoundaryJson, Capability, CapabilityOperation, CapabilityOperationId, CapabilityRuntimeError,
    CapabilityRuntimeErrorKind, CapabilityRuntimeEvent, CapabilityRuntimeRequest,
    CapabilitySurface, FileWatchEventSource, FileWatchRegistration, FileWatchRuntime,
    FileWatchRuntimeConfig, FileWatchSourceError, FileWatchSourceEvent, PluginCancellationToken,
    PluginCapabilityRuntime, PluginHandlerKind, PluginHandlerRef, PluginInvocationContext,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginKey,
    PluginResourceKind, PluginResourceRef, PluginRuntime, PluginWorkerEngine,
    PluginWorkerEngineConfig, QueueSource, RequestId, ScopedRelativePath, WatchCapabilityRequest,
    WatchChangeKind,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeWatchSource {
    registrations: Vec<FileWatchRegistration>,
    unregistered: Vec<PluginResourceRef>,
    events: Vec<FileWatchSourceEvent>,
}

impl FakeWatchSource {
    fn emit(&mut self, event: FileWatchSourceEvent) {
        self.events.push(event);
    }
}

impl FileWatchEventSource for FakeWatchSource {
    fn register(
        &mut self,
        registration: FileWatchRegistration,
    ) -> Result<(), FileWatchSourceError> {
        self.registrations.push(registration);
        Ok(())
    }

    fn unregister(&mut self, resource: &PluginResourceRef) -> Result<(), FileWatchSourceError> {
        self.unregistered.push(resource.clone());
        Ok(())
    }

    fn drain_events(&mut self) -> Result<Vec<FileWatchSourceEvent>, FileWatchSourceError> {
        Ok(std::mem::take(&mut self.events))
    }
}

#[derive(Default)]
struct FastRuntime {
    invocations: Arc<Mutex<Vec<PluginInvocationRequest>>>,
}

impl PluginRuntime for FastRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.invocations
            .lock()
            .expect("fast runtime invocations lock")
            .push(request.clone());
        PluginInvocationResult::Completed(PluginInvocationSuccess {
            request_id: request.request_id,
            handler: request.handler,
            payload: Some(BoundaryJson(serde_json::json!({ "ok": true }))),
        })
    }

    fn stop(&self, _plugin_key: &PluginKey) {}
}

fn plugin_key(name: &str) -> PluginKey {
    PluginKey(name.to_string())
}

fn operation_id(id: &str) -> CapabilityOperationId {
    CapabilityOperationId(id.to_string())
}

fn filesystem_capability(scope: &str) -> Capability {
    Capability {
        surface: CapabilitySurface::Filesystem,
        scope: Some(scope.to_string()),
    }
}

fn callback(plugin: &PluginKey) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin.clone(),
        kind: PluginHandlerKind::Watch,
        handler_id: "watch-directory".to_string(),
    }
}

fn register_request(
    plugin: &PluginKey,
    operation: &str,
    scope: &str,
    path: &str,
) -> CapabilityRuntimeRequest {
    CapabilityRuntimeRequest {
        plugin_key: plugin.clone(),
        operation_id: operation_id(operation),
        operation: CapabilityOperation::Watch(WatchCapabilityRequest::Register {
            scope_id: scope.to_string(),
            path: ScopedRelativePath(path.to_string()),
            recursive: true,
        }),
        timeout_ms: 250,
        callback: Some(callback(plugin)),
    }
}

fn invocation(handler: PluginHandlerRef) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId("invoke-fast".to_string()),
        handler,
        timeout_ms: 250,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: Some("file-watch-test".to_string()),
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({ "event": "unrelated" })),
    }
}

#[test]
fn allowed_registration_returns_handle_and_registers_source() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = FileWatchRuntime::new(FakeWatchSource::default());
    runtime.grant_capability(plugin.clone(), filesystem_capability("workspace"));

    let handle = runtime
        .submit(register_request(&plugin, "watch-op", "workspace", "src"))
        .expect("watch registration accepted");

    assert_eq!(handle.plugin_key, plugin);
    assert_eq!(
        handle.required_capability,
        filesystem_capability("workspace")
    );
    let resource = handle.resource.expect("watch resource");
    assert_eq!(resource.kind, PluginResourceKind::Watch);
    assert_eq!(runtime.source().registrations.len(), 1);
    assert_eq!(runtime.source().registrations[0].resource, resource);
}

#[test]
fn missing_or_wrong_scope_grant_is_rejected_before_source_registration() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = FileWatchRuntime::new(FakeWatchSource::default());
    runtime.grant_capability(plugin.clone(), filesystem_capability("other"));

    let error = runtime
        .submit(register_request(&plugin, "watch-op", "workspace", "src"))
        .expect_err("missing scope grant rejected");

    assert_eq!(error.kind, CapabilityRuntimeErrorKind::CapabilityDenied);
    assert!(runtime.source().registrations.is_empty());
}

#[test]
fn invalid_paths_and_cross_plugin_callbacks_are_rejected_before_source_registration() {
    let plugin = plugin_key("project-pipelines");
    let other = plugin_key("preview");
    let mut runtime = FileWatchRuntime::new(FakeWatchSource::default());
    runtime.grant_capability(plugin.clone(), filesystem_capability("workspace"));

    for path in [
        "",
        "/tmp/secret",
        "\\tmp\\secret",
        "../outside",
        "src/../outside",
    ] {
        let error = runtime
            .submit(register_request(&plugin, "watch-op", "workspace", path))
            .expect_err("invalid scoped path rejected");
        assert_eq!(error.kind, CapabilityRuntimeErrorKind::InvalidRequest);
    }

    let mut request = register_request(&plugin, "watch-op", "workspace", "src");
    request.callback = Some(callback(&other));
    let error = runtime
        .submit(request)
        .expect_err("cross-plugin callback rejected");
    assert_eq!(error.kind, CapabilityRuntimeErrorKind::InvalidRequest);
    assert!(runtime.source().registrations.is_empty());
}

#[test]
fn noisy_events_coalesce_after_debounce_and_preserve_overflow() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 4,
            event_capacity: 8,
            debounce_ms: 25,
        },
    )
    .expect("runtime config");
    runtime.grant_capability(plugin.clone(), filesystem_capability("workspace"));
    let handle = runtime
        .submit(register_request(&plugin, "watch-op", "workspace", "src"))
        .expect("watch registration accepted");
    let resource = handle.resource.expect("watch resource");
    assert!(matches!(
        runtime
            .drain_events(&plugin)
            .expect("drain opened")
            .as_slice(),
        [CapabilityRuntimeEvent::ResourceOpened(_)]
    ));

    runtime.source_mut().emit(FileWatchSourceEvent::path(
        resource.clone(),
        ScopedRelativePath("src/lib.rs".to_string()),
        WatchChangeKind::Created,
        10,
    ));
    runtime.source_mut().emit(FileWatchSourceEvent::path(
        resource.clone(),
        ScopedRelativePath("src/lib.rs".to_string()),
        WatchChangeKind::Modified,
        15,
    ));
    runtime.source_mut().emit(FileWatchSourceEvent::path(
        resource.clone(),
        ScopedRelativePath("src/lib.rs".to_string()),
        WatchChangeKind::Removed,
        20,
    ));
    runtime
        .source_mut()
        .emit(FileWatchSourceEvent::overflow(resource, 21));

    assert!(runtime
        .drain_events(&plugin)
        .expect("before debounce")
        .is_empty());

    runtime.advance_to(46);
    let events = runtime.drain_events(&plugin).expect("after debounce");
    let changes = events
        .iter()
        .filter_map(|event| match event {
            CapabilityRuntimeEvent::Watch(event) => Some(event.change),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(changes.len(), 2);
    assert!(changes.contains(&WatchChangeKind::Removed));
    assert!(changes.contains(&WatchChangeKind::Overflow));
}

#[test]
fn event_queue_pressure_is_bounded_and_reports_plugin_worker_route() {
    let plugin = plugin_key("project-pipelines");
    let mut runtime = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 4,
            event_capacity: 1,
            debounce_ms: 0,
        },
    )
    .expect("runtime config");
    runtime.grant_capability(plugin.clone(), filesystem_capability("workspace"));
    let handle = runtime
        .submit(register_request(&plugin, "watch-op", "workspace", "src"))
        .expect("watch registration accepted");
    let resource = handle.resource.expect("watch resource");
    assert_eq!(runtime.drain_events(&plugin).expect("opened").len(), 1);

    runtime.source_mut().emit(FileWatchSourceEvent::path(
        resource.clone(),
        ScopedRelativePath("src/a.rs".to_string()),
        WatchChangeKind::Modified,
        1,
    ));
    runtime.source_mut().emit(FileWatchSourceEvent::path(
        resource,
        ScopedRelativePath("src/b.rs".to_string()),
        WatchChangeKind::Modified,
        1,
    ));

    let events = runtime.drain_events(&plugin).expect("bounded drain");
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], CapabilityRuntimeEvent::Watch(_)));
    match &events[1] {
        CapabilityRuntimeEvent::Backpressure(pressure) => {
            assert_eq!(pressure.source, QueueSource::PluginWorker);
            assert_eq!(pressure.capacity, 1);
            assert_eq!(pressure.route.plugin_key, Some(plugin));
        }
        other => panic!("expected backpressure event, got {other:?}"),
    }
}

#[test]
fn unregister_release_and_cleanup_call_source_for_only_owned_watches() {
    let plugin_a = plugin_key("project-pipelines");
    let plugin_b = plugin_key("preview");
    let mut runtime = FileWatchRuntime::new(FakeWatchSource::default());
    runtime.grant_capability(plugin_a.clone(), filesystem_capability("workspace"));
    runtime.grant_capability(plugin_b.clone(), filesystem_capability("workspace"));
    let watch_a1 = runtime
        .submit(register_request(&plugin_a, "watch-a1", "workspace", "src"))
        .expect("watch a1")
        .resource
        .expect("resource a1");
    let watch_a2 = runtime
        .submit(register_request(&plugin_a, "watch-a2", "workspace", "docs"))
        .expect("watch a2")
        .resource
        .expect("resource a2");
    let watch_b = runtime
        .submit(register_request(&plugin_b, "watch-b", "workspace", "src"))
        .expect("watch b")
        .resource
        .expect("resource b");

    runtime
        .release_resource(watch_a1.clone())
        .expect("release one watch");
    assert_eq!(runtime.source().unregistered, vec![watch_a1.clone()]);

    let cleanup = runtime.cleanup_plugin(&plugin_a).expect("cleanup plugin a");
    assert_eq!(cleanup.plugin_key, plugin_a);
    assert_eq!(cleanup.removed_resources, vec![watch_a2.clone()]);
    assert_eq!(runtime.source().unregistered, vec![watch_a1, watch_a2]);
    assert!(runtime.release_resource(watch_b).is_ok());
}

#[test]
fn saturated_file_watch_runtime_does_not_block_plugin_worker_invocation() {
    let watcher_plugin = plugin_key("project-pipelines");
    let worker_plugin = plugin_key("preview");
    let worker_handler = PluginHandlerRef {
        plugin_key: worker_plugin.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: "render".to_string(),
    };
    let runtime = FastRuntime::default();
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_capacity: 1,
    });
    engine.load_plugin(botster_core::PluginWorkerRegistration {
        load: botster_core::PluginLoadSpec {
            plugin_key: worker_plugin.clone(),
            package: worker_plugin.0.clone(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: botster_core::PackageManifest {
            name: worker_plugin.0.clone(),
            version: "0.1.0".to_string(),
            kind: botster_core::ExtensionKind::Plugin,
            botster: ">=0.1.0".to_string(),
            source: None,
            capabilities: Vec::new(),
            entrypoints: vec![botster_core::ExtensionEntrypoint {
                runtime: botster_core::ExtensionRuntime::Lua,
                path: "plugin.lua".to_string(),
                bootstrap: false,
            }],
            host_profile: None,
            configuration: None,
        },
        runtime: Arc::new(runtime),
        handlers: vec![botster_core::PluginHandlerRegistration {
            handler: worker_handler.clone(),
            required_capability: None,
        }],
        resources: Vec::new(),
    });

    let file_watch = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 0,
            event_capacity: 1,
            debounce_ms: 0,
        },
    );
    assert!(matches!(
        file_watch,
        Err(CapabilityRuntimeError {
            kind: CapabilityRuntimeErrorKind::InvalidRequest,
            ..
        })
    ));

    let mut file_watch = FileWatchRuntime::with_config(
        FakeWatchSource::default(),
        FileWatchRuntimeConfig {
            registration_capacity: 1,
            event_capacity: 1,
            debounce_ms: 0,
        },
    )
    .expect("runtime config");
    file_watch.grant_capability(watcher_plugin.clone(), filesystem_capability("workspace"));
    file_watch
        .submit(register_request(
            &watcher_plugin,
            "watch-one",
            "workspace",
            "src",
        ))
        .expect("first watch accepted");
    let error = file_watch
        .submit(register_request(
            &watcher_plugin,
            "watch-two",
            "workspace",
            "docs",
        ))
        .expect_err("second watch rejected without blocking");
    assert_eq!(error.kind, CapabilityRuntimeErrorKind::Backpressured);

    assert!(matches!(
        engine.invoke(invocation(worker_handler)).result,
        PluginInvocationResult::Completed(_)
    ));
    let pressure = file_watch
        .drain_events(&watcher_plugin)
        .expect("drain pressure")
        .into_iter()
        .find_map(|event| match event {
            CapabilityRuntimeEvent::Backpressure(pressure) => Some(pressure),
            _ => None,
        })
        .expect("backpressure event");
    assert_eq!(pressure.source, QueueSource::PluginWorker);
}
