//! The allocation controls of the testkit (`docs/core-testkit-controls.md`, steward ruling R-37): `alloc_window` and
//! `alloc_peak`, the measure of Core DP-3's "the frame limit is checked before the allocation"
//! (`conf::dp_3_frame_limit_checked_before_allocation`, a Core A20-1 testkit-proven id).
//!
//! [`PeakAllocator`] wraps the system allocator. It adds one compare per allocation and takes no lock. A test binary that
//! runs this proof installs it with `#[global_allocator]` (the conformance binary, `tests/conformance.rs`). The library does
//! not install it, so no other binary that links the testkit (xtask, the worker binary's tests) gets it. In a binary
//! without it, both controls are unsupported, never a pass.
//!
//! The peak is the peak of the calling thread. A trial of the testkit runs on one thread: the harness, Core and the
//! in-process worker with its route reader (Core A5-1; the testkit starts no thread). The conformance binary runs its
//! trials in parallel, so a process-wide peak would include the allocations of other trials.

use crate::controls::{parse, ControlRegistry};
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("alloc_window", alloc_window);
    registry.register("alloc_peak", alloc_peak);
}

/// True after the first allocation through [`PeakAllocator`]: the binary installed it.
static INSTALLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The largest single allocation size on this thread since its last window. A const initializer and a type with no
    /// drop: the access does not allocate, so the allocator can use it.
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

/// The system allocator, with the peak of the calling thread recorded for each `alloc`, `alloc_zeroed` and the new size
/// of each `realloc`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PeakAllocator;

fn record(size: usize) {
    if !INSTALLED.load(Ordering::Relaxed) {
        INSTALLED.store(true, Ordering::Relaxed);
    }
    // A thread in its teardown has no PEAK any more: its allocation is not measured.
    let _ = PEAK.try_with(|peak| peak.set(peak.get().max(size)));
}

// SAFETY: every call forwards to `System` with the same arguments; `record` only reads and writes plain values.
// The one `unsafe_code` exception of this crate (xtask unsafe_exception, lead ruling 2026-10-10).
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for PeakAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: the caller's contract for `alloc` is the contract of `System.alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        // SAFETY: as for `alloc`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        // SAFETY: the caller's contract for `realloc` is the contract of `System.realloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract for `dealloc` is the contract of `System.dealloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Whether this binary measures allocations: it installed [`PeakAllocator`].
pub(crate) fn installed() -> bool {
    INSTALLED.load(Ordering::Relaxed)
}

/// Whether the harness supports `op`: the allocation controls need [`PeakAllocator`]; every other control is not theirs.
pub(crate) fn supports(op: &str) -> bool {
    supported(op, installed())
}

fn supported(op: &str, installed: bool) -> bool {
    installed || !matches!(op, "alloc_window" | "alloc_peak")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

/// Starts a new window: the peak of this thread is 0.
fn alloc_window(
    _harness: &mut TestkitHarness,
    _handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoArgs {} = parse(args)?;
    window(installed())
}

fn window(installed: bool) -> Result<Value, ControlError> {
    if !installed {
        return Err(ControlError::Unsupported);
    }
    PEAK.with(|peak| peak.set(0));
    Ok(Value::Null)
}

/// `{bytes}`: the largest single allocation size on this thread since `alloc_window`.
fn alloc_peak(
    _harness: &mut TestkitHarness,
    _handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let NoArgs {} = parse(args)?;
    peak(installed())
}

fn peak(installed: bool) -> Result<Value, ControlError> {
    if !installed {
        return Err(ControlError::Unsupported);
    }
    Ok(json!({ "bytes": PEAK.with(Cell::get) }))
}

#[cfg(test)]
mod tests;
