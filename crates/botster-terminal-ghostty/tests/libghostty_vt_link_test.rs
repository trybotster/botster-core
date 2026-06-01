//! Feature-gated smoke test that proves Rust links and calls libghostty-vt.

#![cfg(feature = "libghostty-vt")]

use std::ptr;

use botster_terminal_ghostty::sys::{
    ghostty_terminal_free, ghostty_terminal_new, ghostty_terminal_vt_write, GhosttyTerminal,
    GhosttyTerminalOptions, GHOSTTY_SUCCESS,
};

#[test]
fn feature_enabled_build_links_and_exercises_libghostty_vt() {
    let mut terminal: GhosttyTerminal = ptr::null_mut();

    let result = unsafe {
        ghostty_terminal_new(
            ptr::null(),
            &mut terminal,
            GhosttyTerminalOptions {
                cols: 80,
                rows: 24,
                max_scrollback: 0,
            },
        )
    };

    assert_eq!(result, GHOSTTY_SUCCESS);
    assert!(!terminal.is_null());

    unsafe {
        ghostty_terminal_vt_write(terminal, b"hello from botster".as_ptr(), 18);
        ghostty_terminal_free(terminal);
    }
}
