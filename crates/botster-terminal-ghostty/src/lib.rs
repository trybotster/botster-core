//! The libghostty-vt binding of Botster Core.
//!
//! libghostty owns every terminal semantic (BUILD.md, architecture facts): this crate parses no terminal bytes, tracks
//! no mode and writes no escape sequence. It owns bookkeeping only: buffers, counters and the copy of what a callback
//! reports.
//!
//! `Terminal` is `Send` and not `Sync`: the worker machine owns it on one thread.

mod sys;

use std::cell::Cell;
use std::marker::PhantomData;
use std::ptr::NonNull;

/// An error of the libghostty-vt library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The library could not allocate.
    OutOfMemory,
    /// An argument was out of range.
    InvalidValue,
    /// A buffer was too small, or an unexpected result code.
    Other(i32),
}

impl Error {
    fn from_code(code: sys::Result) -> Self {
        match code {
            sys::OUT_OF_MEMORY => Error::OutOfMemory,
            sys::INVALID_VALUE => Error::InvalidValue,
            other => Error::Other(other),
        }
    }
}

fn check(code: sys::Result) -> Result<(), Error> {
    if code == sys::SUCCESS {
        Ok(())
    } else {
        Err(Error::from_code(code))
    }
}

/// A terminal model.
pub struct Terminal {
    handle: NonNull<std::ffi::c_void>,
    // Not Sync: the library keeps unsynchronized state.
    _not_sync: PhantomData<Cell<()>>,
}

// SAFETY: the library does not tie a terminal to the thread that made it, and `&mut self` serializes every call. A
// terminal is `!Sync`, so it is never used from two threads at once.
unsafe impl Send for Terminal {}

impl Terminal {
    /// A terminal of the given size, with no cell pixel size.
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        let mut handle: sys::Terminal = std::ptr::null_mut();
        // SAFETY: `handle` is a valid out pointer, a null allocator selects the default, and a successful call stores
        // a live handle that `Drop` frees.
        check(unsafe { sys::ghostty_terminal_new(std::ptr::null(), &mut handle, cols, rows) })?;
        let handle = NonNull::new(handle).ok_or(Error::InvalidValue)?;
        Ok(Self { handle, _not_sync: PhantomData })
    }

    /// Feed one chunk of program output to the model.
    pub fn vt_write(&mut self, bytes: &[u8]) {
        // SAFETY: the handle is live, and the slice is valid for the length for the duration of the call.
        unsafe { sys::ghostty_terminal_vt_write(self.handle.as_ptr(), bytes.as_ptr(), bytes.len()) }
    }

    /// Apply a new size. A zero cell size means that it is unknown.
    pub fn resize(&mut self, cols: u16, rows: u16, cell_width_px: u32, cell_height_px: u32) -> Result<(), Error> {
        // SAFETY: the handle is live.
        check(unsafe {
            sys::ghostty_terminal_resize(self.handle.as_ptr(), cols, rows, cell_width_px, cell_height_px)
        })
    }

    /// The number of columns.
    pub fn cols(&self) -> u16 {
        self.get_u16(sys::data::COLS)
    }

    /// The number of rows.
    pub fn rows(&self) -> u16 {
        self.get_u16(sys::data::ROWS)
    }

    fn get_u16(&self, key: i32) -> u16 {
        let mut out: u16 = 0;
        // SAFETY: the handle is live, and `key` is one of the keys whose output type is a `u16` cell count.
        let code = unsafe { sys::ghostty_terminal_get(self.handle.as_ptr(), key, (&mut out as *mut u16).cast()) };
        debug_assert_eq!(code, sys::SUCCESS);
        out
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: the handle is live and is freed exactly once.
        unsafe { sys::ghostty_terminal_free(self.handle.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_terminal_has_its_size_and_takes_output() {
        let mut terminal = Terminal::new(80, 24).unwrap();
        assert_eq!((terminal.cols(), terminal.rows()), (80, 24));
        terminal.vt_write(b"hello");
        terminal.resize(100, 30, 0, 0).unwrap();
        assert_eq!((terminal.cols(), terminal.rows()), (100, 30));
    }

    #[test]
    fn terminal_is_send_and_not_sync() {
        fn is_send<T: Send>() {}
        is_send::<Terminal>();
        static_assertions::assert_not_impl_any!(Terminal: Sync);
    }

    #[test]
    fn the_zig_package_list_is_the_one_that_the_prefetch_script_reads() {
        // build_data.rs holds the only list. The script reads it with sed, so each entry sits on its own line.
        let data = include_str!("../build_data.rs");
        let listed = data.lines().filter(|line| line.trim_start().starts_with('"') && line.trim_end().ends_with("\",")).count();
        // 6 build arguments, 7 packages and the 2 of them that the zon files name.
        assert_eq!(listed, 15);
    }
}
