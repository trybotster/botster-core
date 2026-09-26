//! Allocation regression for the host-facing completion drain.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

use botster_core::{
    BoundaryJson, ExtensionEntrypoint, ExtensionKind, ExtensionRuntime, PackageManifest,
    PluginAdmissionResult, PluginHandlerKind, PluginHandlerRef, PluginInvocationClass,
    PluginInvocationContext, PluginInvocationRequest, PluginKey, PluginLoadSpec,
    PluginWorkerEngine, PluginWorkerRegistration, RequestId,
};
use botster_core_test_support::fake::FakePluginRuntime;

struct CountingAllocator;
thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn note_allocation() {
    ALLOCATIONS.with(|count| count.set(count.get().map(|value| value + 1)));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        note_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let value = operation();
    let count = ALLOCATIONS.with(|count| count.replace(None).expect("allocation measurement"));
    (value, count)
}

fn register(engine: &PluginWorkerEngine, key: PluginKey) {
    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: key,
            package: "idle".into(),
            entrypoint: "plugin.lua".into(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: PackageManifest {
            name: "large-idle-manifest".repeat(256),
            version: "0.1.0".into(),
            kind: ExtensionKind::Plugin,
            botster: ">=0.1.0".into(),
            source: None,
            capabilities: Vec::new(),
            entrypoints: vec![ExtensionEntrypoint {
                runtime: ExtensionRuntime::Lua,
                path: "plugin.lua".into(),
                bootstrap: false,
            }],
            dependencies: Vec::new(),
            features: Vec::new(),
            host_profile: None,
            configuration: None,
            runnable_entrypoints: Vec::new(),
        },
        runtime: Arc::new(FakePluginRuntime::success("unused")),
        handlers: Vec::new(),
        resources: Vec::new(),
    });
}

#[test]
fn completion_drain_does_not_allocate_for_idle_workers_or_blocked_front() {
    // Darwin lazily allocates the std mutex on first lock. Compare fresh
    // engines before checking steady state so registry work cannot hide there.
    let empty_engine = PluginWorkerEngine::new();
    let (_, empty_first_allocations) = measured(|| empty_engine.drain_completions(8, 4096));
    let (_, empty_repeated_allocations) = measured(|| empty_engine.drain_completions(8, 4096));
    assert_eq!(empty_repeated_allocations, 0);
    let engine = PluginWorkerEngine::new();
    for index in 0..32 {
        register(&engine, PluginKey(format!("idle-{index}")));
    }
    let (empty, allocations) = measured(|| engine.drain_completions(8, 4096));
    assert_eq!(
        allocations, empty_first_allocations,
        "idle registry must not add first-use drain allocations"
    );
    assert_eq!(empty.item_count, 0);
    assert!(!empty.has_remaining);
    let (_, repeated_allocations) = measured(|| engine.drain_completions(8, 4096));
    assert_eq!(
        repeated_allocations, 0,
        "idle registry must never be cloned by drain"
    );

    let request = PluginInvocationRequest {
        request_id: RequestId("published".into()),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("idle-17".into()),
            kind: PluginHandlerKind::Command,
            handler_id: "missing".into(),
        },
        timeout_ms: 1000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: BoundaryJson(serde_json::json!({})),
    };
    let admitted = engine.admit(PluginInvocationClass::Background, request, 4096);
    assert!(
        matches!(admitted, PluginAdmissionResult::Queued { .. }),
        "expected immediate completion, got {admitted:?}"
    );
    let (blocked, allocations) = measured(|| engine.drain_completions(8, 1));
    assert_eq!(
        allocations, 0,
        "blocked front must not trigger a worker census"
    );
    assert_eq!(blocked.item_count, 0);
    assert!(blocked.has_remaining);
    let (fitting, allocations) = measured(|| engine.drain_completions(1, 4096));
    assert_eq!(allocations, 1, "only the selected output vector allocates");
    assert_eq!(fitting.item_count, 1);
    assert!(!fitting.has_remaining);
    let snapshot = engine.debug_snapshot();
    assert_eq!(snapshot.reserved_completion_count, 0);
    assert_eq!(snapshot.reserved_completion_bytes, 0);
}
