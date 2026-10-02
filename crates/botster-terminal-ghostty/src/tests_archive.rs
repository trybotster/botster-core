//! A test of the native archive. A static library must not define a libc allocator symbol: a program that links it
//! would free memory of the host's `malloc` with the archive's `free` (the Linux abort of review finding P32).

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
