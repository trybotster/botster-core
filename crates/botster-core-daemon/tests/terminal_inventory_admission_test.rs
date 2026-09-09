//! Production daemon forwarding must not allocate around inventory admission.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::mem::size_of;

use botster_core::{TerminalSubscriptionInventory, TerminalSubscriptionInventoryError};
use botster_core_daemon::{CoreDaemon, CoreDaemonConfig};

struct CountingAllocator;

thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|count| count.set(count.get().map(|value| value + 1)));
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|count| count.set(count.get().map(|value| value + 1)));
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|count| count.set(count.get().map(|value| value + 1)));
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn terminal_inventory_daemon_forwards_budget_without_output_allocation() {
    // Construction does not start a session or touch this registry path.
    for config in [
        CoreDaemonConfig::new("unused-inventory-registry"),
        CoreDaemonConfig::new("unused-inventory-registry").with_worker_path("unused-worker"),
    ] {
        let daemon = CoreDaemon::new(config);
        let required = size_of::<TerminalSubscriptionInventory>();
        for budget in [0, required - 1, required] {
            COUNT.with(|count| count.set(Some(0)));
            let result = daemon.list_terminal_subscriptions(budget);
            let allocations = COUNT.with(|count| count.replace(None).expect("measurement enabled"));
            assert_eq!(allocations, 0);
            if budget < required {
                assert_eq!(
                    result,
                    Err(TerminalSubscriptionInventoryError::BudgetTooSmall {
                        required_bytes: required,
                        max_bytes: budget,
                    })
                );
            } else {
                let inventory = result.expect("exact empty fit");
                assert_eq!(inventory.logical_bytes, required);
                assert!(inventory.records.is_empty());
            }
        }
    }
}
