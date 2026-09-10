//! Borrowed live-owner lookup: identity semantics and allocation-free forwarding.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use botster_core::{
    ClientId, ClientWorker, DetachTerminalSubscriptionResult, SessionId, SubscriptionId,
};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig};

struct CountingAllocator;

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

fn allocated() {
    COUNT.with(|count| count.set(count.get().map(|value| value + 1)));
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measured<T>(action: impl FnOnce() -> T) -> (T, usize) {
    COUNT.with(|count| count.set(Some(0)));
    let result = action();
    let allocations = COUNT.with(|count| count.replace(None).expect("measurement enabled"));
    (result, allocations)
}

#[test]
fn owner_lookup_is_live_only_and_preserves_captured_generation() {
    let mut worker = ClientWorker::new();
    let first = ClientId("first".into());
    let replacement = ClientId("replacement".into());
    let session = SessionId("session".into());
    let route = SubscriptionId("route".into());
    assert_eq!(worker.terminal_subscription_owner(&session, &route), None);
    worker.expect_terminal_adapter(first.clone(), session.clone(), route.clone());
    assert_eq!(worker.terminal_subscription_owner(&session, &route), None);
    let (captured, _) = worker
        .record_attach(first.clone(), session.clone(), route.clone())
        .expect("attach first owner");
    let (repeated, _) = worker
        .record_attach(first.clone(), session.clone(), route.clone())
        .expect("repeat first owner");
    assert_eq!(captured, repeated);
    assert_eq!(
        worker.terminal_subscription_owner(&session, &route),
        Some((&first, captured))
    );
    let (current, _) = worker
        .record_attach(replacement.clone(), session.clone(), route.clone())
        .expect("replace route owner");
    assert_ne!(current, captured);
    let owner = worker.terminal_subscription_owner(&session, &route);
    let matching = owner
        .filter(|(client, _)| *client == &first)
        .map(|(_, generation)| generation);
    let foreign = owner.is_some_and(|(client, _)| client != &first);
    assert!(foreign);
    assert_eq!(matching, None);
    // A caller's captured generation takes precedence over a new live lookup.
    let selected = Some(captured).or(matching).expect("captured generation");
    assert!(matches!(
        worker.detach_generation(&session, &route, selected),
        DetachTerminalSubscriptionResult::GenerationMismatch { live, requested }
            if live == current && requested == captured
    ));
    assert_eq!(
        worker.terminal_subscription_owner(&session, &route),
        Some((&replacement, current))
    );
    worker
        .detach_live(&session, &route)
        .expect("detach current owner");
    // The route remains historically known, but is no longer live.
    assert_eq!(worker.terminal_subscription_owner(&session, &route), None);
}

#[test]
fn owner_lookup_distinguishes_sessions_and_sibling_routes() {
    let mut worker = ClientWorker::new();
    let first = ClientId("first".into());
    let second = ClientId("second".into());
    let session = SessionId("session".into());
    let other_session = SessionId("other-session".into());
    let route = SubscriptionId("route".into());
    let sibling = SubscriptionId("sibling".into());
    let (first_generation, _) = worker
        .record_attach(first.clone(), session.clone(), route.clone())
        .unwrap();
    let (second_generation, _) = worker
        .record_attach(second.clone(), session.clone(), sibling.clone())
        .unwrap();
    let (other_generation, _) = worker
        .record_attach(first.clone(), other_session.clone(), route.clone())
        .unwrap();
    assert_eq!(
        worker.terminal_subscription_owner(&session, &route),
        Some((&first, first_generation))
    );
    assert_eq!(
        worker.terminal_subscription_owner(&session, &sibling),
        Some((&second, second_generation))
    );
    assert_eq!(
        worker.terminal_subscription_owner(&other_session, &route),
        Some((&first, other_generation))
    );
    let replacement_route = SubscriptionId("replacement-route".into());
    let (replacement_generation, _) = worker
        .record_attach(first.clone(), session.clone(), replacement_route.clone())
        .unwrap();
    assert_eq!(worker.terminal_subscription_owner(&session, &route), None);
    assert_eq!(
        worker.terminal_subscription_owner(&session, &replacement_route),
        Some((&first, replacement_generation))
    );
    assert_eq!(
        worker.terminal_subscription_owner(&session, &sibling),
        Some((&second, second_generation))
    );
    assert_eq!(
        worker.terminal_subscription_owner(&other_session, &route),
        Some((&first, other_generation))
    );
}

#[test]
fn owner_lookup_first_and_repeated_populated_queries_allocate_nothing() {
    let mut worker = ClientWorker::new();
    let client = ClientId("c".repeat(128 * 1024));
    let session = SessionId("s".repeat(128 * 1024));
    let route = SubscriptionId("r".repeat(botster_terminal_protocol::MAX_ROUTE_ID_BYTES));
    let missing = SubscriptionId("missing".into());
    let (generation, _) = worker
        .record_attach(client.clone(), session.clone(), route.clone())
        .unwrap();
    for index in 0..64 {
        worker
            .record_attach(
                ClientId(format!("other-{index}")),
                SessionId(format!("session-{index}")),
                SubscriptionId(format!("route-{index}")),
            )
            .unwrap();
    }
    for _ in 0..3 {
        let (owner, allocations) =
            measured(|| worker.terminal_subscription_owner(&session, &route));
        assert_eq!(
            allocations, 0,
            "lookup must not clone even long identifiers"
        );
        assert_eq!(owner, Some((&client, generation)));
        let (absent, allocations) =
            measured(|| worker.terminal_subscription_owner(&session, &missing));
        assert_eq!(allocations, 0);
        assert_eq!(absent, None);
    }
}

#[test]
fn owner_lookup_daemon_forwards_both_engine_variants_without_allocation() {
    // Construction alone starts no process and does not read this registry path.
    let session = SessionId("s".repeat(128 * 1024));
    let route = SubscriptionId("r".repeat(botster_terminal_protocol::MAX_ROUTE_ID_BYTES));
    for config in [
        CoreDaemonConfig::new("unused-owner-lookup-registry"),
        CoreDaemonConfig::new("unused-owner-lookup-registry").with_worker_path("unused-worker"),
    ] {
        let daemon = CoreDaemon::new(config);
        for _ in 0..3 {
            let (owner, allocations) =
                measured(|| daemon.terminal_subscription_owner(&session, &route));
            assert_eq!(allocations, 0);
            assert_eq!(owner, None);
        }
    }
}
