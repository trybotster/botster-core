//! Inject allocation failure through the existing C allocator interface.

use super::*;
use std::alloc::{alloc, dealloc, Layout};

#[repr(C)]
struct Allocator {
    ctx: *mut c_void,
    vtable: *const Vtable,
}

#[repr(C)]
struct Vtable {
    alloc: unsafe extern "C" fn(*mut c_void, usize, u8, usize) -> *mut c_void,
    resize: unsafe extern "C" fn(*mut c_void, *mut c_void, usize, u8, usize, usize) -> bool,
    remap: unsafe extern "C" fn(*mut c_void, *mut c_void, usize, u8, usize, usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void, *mut c_void, usize, u8, usize),
}

unsafe extern "C" fn allocate(
    ctx: *mut c_void,
    len: usize,
    alignment: u8,
    _: usize,
) -> *mut c_void {
    // SAFETY: the allocator context outlives every native allocation and free.
    if unsafe { &*ctx.cast::<Cell<bool>>() }.get() {
        return std::ptr::null_mut();
    }
    // The pinned src/lib/allocator.zig passes log2 alignment, as Zig's allocator does.
    let layout = Layout::from_size_align(len, 1usize << alignment).unwrap();
    // SAFETY: the library requests a nonzero allocation with a valid alignment.
    unsafe { alloc(layout).cast() }
}

unsafe extern "C" fn resize(
    _: *mut c_void,
    _: *mut c_void,
    _: usize,
    _: u8,
    _: usize,
    _: usize,
) -> bool {
    false
}

unsafe extern "C" fn remap(
    _: *mut c_void,
    _: *mut c_void,
    _: usize,
    _: u8,
    _: usize,
    _: usize,
) -> *mut c_void {
    std::ptr::null_mut()
}

unsafe extern "C" fn free(
    _: *mut c_void,
    memory: *mut c_void,
    len: usize,
    alignment: u8,
    _: usize,
) {
    let layout = Layout::from_size_align(len, 1usize << alignment).unwrap();
    // SAFETY: the library supplies the allocation's original pointer, length and alignment.
    unsafe { dealloc(memory.cast(), layout) };
}

#[test]
fn a_native_allocation_failure_sets_the_public_processing_error() {
    let fail = Cell::new(false);
    let vtable = Vtable {
        alloc: allocate,
        resize,
        remap,
        free,
    };
    let allocator = Allocator {
        ctx: (&fail as *const Cell<bool>).cast_mut().cast(),
        vtable: &vtable,
    };
    let mut terminal = Terminal::new(
        &Size {
            rows: 10,
            cols: 40,
            cell_px: None,
        },
        History::On,
    )
    .unwrap();
    let mut handle = std::ptr::null_mut();
    // SAFETY: allocator data outlives the terminal. The old terminal has no live external references.
    unsafe {
        check(sys::ghostty_terminal_new(
            (&allocator as *const Allocator).cast(),
            &mut handle,
            40,
            10,
        ))
        .unwrap();
        sys::ghostty_terminal_free(terminal.handle.as_ptr());
        terminal.handle = NonNull::new(handle).unwrap();
    }
    terminal.register_callbacks().unwrap();
    assert!(!terminal.vt_processing_error().unwrap());
    fail.set(true);
    terminal.vt_write(b"\x1b]2;unavailable\x1b\\");
    let mut native = false;
    // SAFETY: this data key writes a bool into the supplied live pointer.
    unsafe {
        check(sys::ghostty_terminal_get(
            terminal.handle.as_ptr(),
            sys::data::VT_PROCESSING_ERROR,
            (&mut native as *mut bool).cast(),
        ))
        .unwrap();
    }
    assert!(native);
    assert_eq!(terminal.vt_processing_error().unwrap(), native);
}
