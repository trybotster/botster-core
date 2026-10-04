//! Tests of the native archive. The binding links it statically, and a static library must not define a libc allocator
//! symbol: a program that links it would free memory of the host's `malloc` with the archive's `free` (the Linux abort
//! of review finding P32).

use std::ffi::{c_char, c_int, c_void, CStr};
use std::process::Command;

/// The allocator functions of libc that a host program provides.
const ALLOCATOR_SYMBOLS: &[&str] = &[
    "malloc",
    "calloc",
    "realloc",
    "reallocarray",
    "free",
    "aligned_alloc",
    "posix_memalign",
    "memalign",
    "valloc",
    "pvalloc",
    "malloc_usable_size",
];

#[test]
fn the_native_archive_defines_no_libc_allocator_symbol() {
    let archive = env!("BOTSTER_GHOSTTY_VT_ARCHIVE");
    // `nm` comes with the build host's binutils or LLVM tools. Without it the test cannot run, and it fails.
    let output = Command::new("nm")
        .args(["-g", "--defined-only", archive])
        .output()
        .expect("the archive test needs nm on the build host");
    assert!(
        output.status.success(),
        "nm failed on {archive}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listing = String::from_utf8_lossy(&output.stdout);

    // A line is `address type name`. A name on macOS has one leading underscore.
    let defined: Vec<&str> = listing
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .map(|name| name.strip_prefix('_').unwrap_or(name))
        .filter(|name| ALLOCATOR_SYMBOLS.contains(name))
        .collect();
    assert!(defined.is_empty(), "the archive defines {defined:?}");
    // The listing is not empty, so the check looked at something.
    assert!(listing.contains("ghostty_terminal_new"));
}

/// `Dl_info` of `dladdr` (the same layout on macOS and glibc).
#[repr(C)]
struct DlInfo {
    fname: *const c_char,
    fbase: *mut c_void,
    sname: *const c_char,
    saddr: *mut c_void,
}

extern "C" {
    fn dladdr(address: *const c_void, info: *mut DlInfo) -> c_int;
}

#[test]
fn the_library_is_linked_into_the_program_and_not_loaded_from_a_shared_library() {
    // The loader names the image that holds a library function. A static link puts it in the test program; a dynamic
    // link (the Apple linker takes `libghostty-vt.dylib` when it is next to the archive) puts it in the shared library,
    // and the program then needs that file at run time.
    let mut info = DlInfo {
        fname: std::ptr::null(),
        fbase: std::ptr::null_mut(),
        sname: std::ptr::null(),
        saddr: std::ptr::null_mut(),
    };
    // SAFETY: the address is a function of the linked library, and `info` is a valid `Dl_info`.
    let found = unsafe { dladdr(crate::sys::ghostty_terminal_new as *const c_void, &mut info) };
    assert_ne!(found, 0, "dladdr found no image for ghostty_terminal_new");
    assert!(!info.fname.is_null());
    // SAFETY: `dladdr` succeeded, so `fname` is a C string that the loader owns.
    let image = unsafe { CStr::from_ptr(info.fname) }.to_string_lossy();
    // glibc names the main program as it was started (possibly a relative path), so the file names are compared.
    let program = std::env::current_exe().unwrap();
    assert_eq!(
        std::path::Path::new(image.as_ref()).file_name(),
        program.file_name(),
        "ghostty_terminal_new is loaded from {image}, not from the test program"
    );
}
