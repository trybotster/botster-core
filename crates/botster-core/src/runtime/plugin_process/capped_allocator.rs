//! Child side: the counting allocator that enforces the memory cap (plan
//! section 7.4).
//!
//! The Hub's worker binary declares it as its global allocator:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: CappedAllocator = CappedAllocator::new();
//! ```
//!
//! It counts every Rust allocation, and the Lua heap too, because mlua
//! allocates through `std::alloc`. The cap arrives in `Bootstrap` and is set
//! before `Ready`, so it holds before any plugin byte loads. Core declares no
//! global allocator itself: that would change every binary that links it.
//!
//! An allocation over the cap never returns null, because a null would let
//! Lua raise a catchable "not enough memory" error. Instead the allocator
//! writes the memory-cap cause byte to the fatal pipe (one non-blocking,
//! allocation-free system call that never touches the IPC socket) and
//! aborts. The parent classifies the exit from that byte.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::protocol::CAUSE_MEMORY_CAP;
use super::worker::write_fatal;

/// Bytes currently allocated through the allocator.
static USED: AtomicUsize = AtomicUsize::new(0);
/// The cap; `usize::MAX` until the parent sets one.
static CAP: AtomicUsize = AtomicUsize::new(usize::MAX);
/// Set by the first allocation, which proves that the binary installed the
/// allocator.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// A counting global allocator over [`System`] that aborts, with the
/// memory-cap cause, when an allocation would exceed the cap.
#[derive(Debug, Default, Clone, Copy)]
pub struct CappedAllocator;

impl CappedAllocator {
    /// The allocator, for a `#[global_allocator]` static.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Count `bytes` more. Over the cap, publish the cause and abort.
fn charge(bytes: usize) {
    if !ACTIVE.load(Ordering::Relaxed) {
        ACTIVE.store(true, Ordering::Relaxed);
    }
    let used = USED
        .fetch_add(bytes, Ordering::Relaxed)
        .saturating_add(bytes);
    if used > CAP.load(Ordering::Relaxed) {
        exceeded();
    }
}

fn uncharge(bytes: usize) {
    USED.fetch_sub(bytes, Ordering::Relaxed);
}

/// The allocation-free failure path: one non-blocking write of the cause
/// byte to the fatal pipe, then abort.
fn exceeded() -> ! {
    write_fatal(&[CAUSE_MEMORY_CAP]);
    std::process::abort()
}

// SAFETY: every method forwards to `System` with the caller's layout and
// pointer unchanged; the counters only observe sizes.
unsafe impl GlobalAlloc for CappedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        // SAFETY: forwarded with the caller's layout.
        let ptr = unsafe { System.alloc(layout) };
        if ptr.is_null() {
            uncharge(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        charge(layout.size());
        // SAFETY: forwarded with the caller's layout.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if ptr.is_null() {
            uncharge(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded with the caller's pointer and layout.
        unsafe { System.dealloc(ptr, layout) };
        uncharge(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_size = layout.size();
        if new_size > old_size {
            charge(new_size - old_size);
        }
        // SAFETY: forwarded with the caller's pointer, layout, and size.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if new.is_null() {
            if new_size > old_size {
                uncharge(new_size - old_size);
            }
        } else if new_size < old_size {
            uncharge(old_size - new_size);
        }
        new
    }
}

/// Set the cap. Refused when the worker binary did not install
/// [`CappedAllocator`] (a requested cap is never silently ignored), or when
/// the worker already uses more than the cap.
pub(super) fn install(cap: u64) -> Result<(), String> {
    if !ACTIVE.load(Ordering::Relaxed) {
        return Err(
            "a memory cap requires CappedAllocator as the worker's global allocator".to_string(),
        );
    }
    let cap = usize::try_from(cap).unwrap_or(usize::MAX);
    let used = USED.load(Ordering::Relaxed);
    if used > cap {
        return Err(format!(
            "the worker already uses {used} bytes, more than the {cap}-byte memory cap"
        ));
    }
    CAP.store(cap, Ordering::Relaxed);
    Ok(())
}
