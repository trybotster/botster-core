//! Caller-budgeted inventory production and allocation boundary proofs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::mem::size_of;

use botster_core::{
    ClientId, ClientWorker, SessionId, SubscriptionId, TerminalCapabilitySet,
    TerminalSubscriptionInventory, TerminalSubscriptionInventoryError, TerminalSubscriptionRecord,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_protocol::{FEATURE_RESIZE, FEATURE_TERMINAL_STREAMING};

struct CountingAllocator;

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

fn count_allocation() {
    COUNT.with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, size) }
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
fn terminal_inventory_empty_still_requires_wrapper_allowance() {
    let worker = ClientWorker::new();
    let required = size_of::<TerminalSubscriptionInventory>();
    let (result, allocations) = measured(|| worker.list_terminal_subscriptions(required - 1));
    assert_eq!(allocations, 0);
    assert_eq!(
        result,
        Err(TerminalSubscriptionInventoryError::BudgetTooSmall {
            required_bytes: required,
            max_bytes: required - 1,
        })
    );
    let (result, allocations) = measured(|| worker.list_terminal_subscriptions(required));
    let inventory = result.expect("exact empty fit");
    assert_eq!(allocations, 0);
    assert!(inventory.records.is_empty());
    assert_eq!(inventory.logical_bytes, required);
}

#[test]
fn terminal_inventory_exact_fit_counts_long_ids_and_capability_storage() {
    let mut worker = ClientWorker::new();
    let client = ClientId("c".repeat(128 * 1024));
    let session = SessionId("s".repeat(128 * 1024));
    let subscription = SubscriptionId("r".repeat(botster_terminal_protocol::MAX_ROUTE_ID_BYTES));
    let capabilities =
        TerminalCapabilitySet::from_tokens([FEATURE_RESIZE, FEATURE_TERMINAL_STREAMING])
            .expect("advertised tokens");
    let capability_bytes: usize = capabilities
        .iter()
        .map(|token| size_of::<String>() + token.len())
        .sum();
    let (generation, _) = worker
        .record_attach(client.clone(), session.clone(), subscription.clone())
        .expect("attach unrestricted IDs");
    worker
        .bind_waking_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            generation,
            capabilities.clone(),
            Box::new(SharedFakeTerminalAdapter::auto_complete()),
        )
        .expect("bind");
    let required = size_of::<TerminalSubscriptionInventory>()
        + size_of::<TerminalSubscriptionRecord>()
        + client.0.len()
        + session.0.len()
        + subscription.0.len()
        + capability_bytes;
    let (result, allocations) = measured(|| worker.list_terminal_subscriptions(required - 1));
    assert_eq!(
        allocations, 0,
        "refusal must precede Vec, ID and capability clones"
    );
    assert_eq!(
        result,
        Err(TerminalSubscriptionInventoryError::BudgetTooSmall {
            required_bytes: required,
            max_bytes: required - 1,
        })
    );
    let inventory = worker
        .list_terminal_subscriptions(required)
        .expect("exact fit");
    assert_eq!(inventory.logical_bytes, required);
    assert_eq!(
        inventory.records,
        vec![TerminalSubscriptionRecord {
            client_id: client,
            session_id: session,
            subscription_id: subscription,
            generation,
            adapter_bound: true,
            capabilities: Some(capabilities),
        }]
    );
}

#[test]
fn terminal_inventory_order_is_deterministic_and_sort_has_no_heap_scratch() {
    let mut worker = ClientWorker::new();
    let mut expected = Vec::new();
    let mut required = size_of::<TerminalSubscriptionInventory>();
    for index in (0..96).rev() {
        let client = ClientId(format!("client-{index:03}"));
        let session = SessionId(format!("session-{:03}", index % 7));
        let subscription = SubscriptionId(format!("route-{index:03}"));
        required += size_of::<TerminalSubscriptionRecord>()
            + client.0.len()
            + session.0.len()
            + subscription.0.len();
        let (generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("attach");
        expected.push(TerminalSubscriptionRecord {
            client_id: client,
            session_id: session,
            subscription_id: subscription,
            generation,
            adapter_bound: false,
            capabilities: None,
        });
    }
    expected.sort_by(|left, right| {
        (&left.session_id.0, &left.subscription_id.0, left.generation).cmp(&(
            &right.session_id.0,
            &right.subscription_id.0,
            right.generation,
        ))
    });
    for _ in 0..3 {
        let (result, allocations) = measured(|| worker.list_terminal_subscriptions(required));
        let inventory = result.expect("fit");
        assert_eq!(
            allocations,
            1 + 3 * expected.len(),
            "one row Vec and three ID clones per row, no sorting scratch"
        );
        assert_eq!(inventory.logical_bytes, required);
        assert_eq!(inventory.records, expected);
    }
    let row = &expected[0];
    let (matches, allocations) = measured(|| {
        worker.terminal_subscription_matches(&row.session_id, &row.client_id, &row.subscription_id)
    });
    assert!(matches);
    assert_eq!(allocations, 0);
    assert!(!worker.terminal_subscription_matches(
        &row.session_id,
        &ClientId("wrong".into()),
        &row.subscription_id
    ));
}

#[cfg(feature = "local-runtime")]
#[test]
fn terminal_inventory_engine_forwarders_preserve_refusal_without_allocation() {
    let local = botster_core::DefaultBotsterEngine::new();
    let worker = botster_core::DefaultBotsterEngine::worker_backed("unused-worker-path");
    let required = size_of::<TerminalSubscriptionInventory>();
    let (local_result, local_allocations) = measured(|| local.list_terminal_subscriptions(0));
    let (worker_result, worker_allocations) = measured(|| worker.list_terminal_subscriptions(0));
    let expected = Err(TerminalSubscriptionInventoryError::BudgetTooSmall {
        required_bytes: required,
        max_bytes: 0,
    });
    assert_eq!(local_result, expected);
    assert_eq!(worker_result, expected);
    assert_eq!(local_allocations, 0);
    assert_eq!(worker_allocations, 0);
    assert_eq!(
        local
            .list_terminal_subscriptions(required)
            .expect("empty fit")
            .logical_bytes,
        required
    );
    assert_eq!(
        worker
            .list_terminal_subscriptions(required)
            .expect("empty fit")
            .logical_bytes,
        required
    );
}
