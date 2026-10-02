//! The libghostty-vt binding of Botster Core.
//!
//! libghostty owns every terminal semantic (BUILD.md, architecture facts): this crate parses no terminal bytes, tracks
//! no mode and writes no escape sequence. It owns bookkeeping only: buffers, counters and the copy of what a callback
//! reports.
//!
//! `Terminal` is `Send` and not `Sync`: the worker machine owns it on one thread.

mod events;
mod modes;
mod sys;

use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub use botster_route_codec::prelude::ModeFlags;
pub use events::{Drained, TerminalEvent, MAX_BUFFERED_BYTES, MAX_BUFFERED_EVENTS};

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
    handle: NonNull<c_void>,
    /// The event buffer that the callbacks fill. It is a leaked `Box`, freed in `Drop` after the terminal.
    shared: NonNull<events::Shared>,
    // Not Sync: the library keeps unsynchronized state.
    _not_sync: PhantomData<Cell<()>>,
}

// SAFETY: the library does not tie a terminal to the thread that made it, and `&mut self` serializes every call. The
// event buffer is only reached through the terminal. A terminal is `!Sync`, so it is never used from two threads at
// once.
unsafe impl Send for Terminal {}

impl Terminal {
    /// A terminal of the given size, with no cell pixel size.
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        let mut handle: sys::Terminal = std::ptr::null_mut();
        // SAFETY: `handle` is a valid out pointer, a null allocator selects the default, and a successful call stores
        // a live handle that `Drop` frees.
        check(unsafe { sys::ghostty_terminal_new(std::ptr::null(), &mut handle, cols, rows) })?;
        let handle = NonNull::new(handle).ok_or(Error::InvalidValue)?;

        let shared = NonNull::from(Box::leak(Box::<events::Shared>::default()));
        let terminal = Self { handle, shared, _not_sync: PhantomData };
        terminal.register_callbacks()?;
        Ok(terminal)
    }

    /// Register the callbacks that fill the event buffer. The title and the working directory are read in their own
    /// callbacks, so this is the one place that sets effects.
    fn register_callbacks(&self) -> Result<(), Error> {
        let handle = self.handle.as_ptr();
        // SAFETY: the handle is live. The userdata is the boxed buffer, valid until `Drop`. Each callback is passed as
        // the option's value, which is how the library takes a function pointer.
        unsafe {
            check(sys::ghostty_terminal_set(handle, sys::opt::USERDATA, self.shared.as_ptr().cast()))?;
            check(sys::ghostty_terminal_set(handle, sys::opt::BELL, events::on_bell as sys::BellFn as *const c_void))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::TITLE_CHANGED,
                events::on_title_changed as sys::TitleChangedFn as *const c_void,
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::PWD_CHANGED,
                events::on_pwd_changed as sys::PwdChangedFn as *const c_void,
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::DESKTOP_NOTIFICATION,
                events::on_notification as sys::DesktopNotificationFn as *const c_void,
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::SEMANTIC_PROMPT,
                events::on_semantic_prompt as sys::SemanticPromptFn as *const c_void,
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::CLIPBOARD_WRITE,
                events::on_clipboard_write as sys::ClipboardWriteFn as *const c_void,
            ))?;
        }
        Ok(())
    }

    /// Feed one chunk of program output to the model. The callbacks run inside this call and fill the event buffer.
    pub fn vt_write(&mut self, bytes: &[u8]) {
        // SAFETY: the handle is live, and the slice is valid for its length for the duration of the call.
        unsafe { sys::ghostty_terminal_vt_write(self.handle.as_ptr(), bytes.as_ptr(), bytes.len()) }
    }

    /// Take what the model reported since the last drain, in observation order.
    pub fn drain_events(&mut self) -> Drained {
        // SAFETY: the buffer is live until `Drop`, and `&mut self` means that no callback runs now, so this is the
        // only reference.
        unsafe { self.shared.as_mut() }.drain()
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

    /// The modes of the model (5.1A `ModeFlags`), read from the library now.
    pub fn modes(&self) -> ModeFlags {
        modes::mode_flags(self.handle.as_ptr())
    }

    /// The title, as the library holds it (empty when none was set).
    pub fn title(&self) -> String {
        // SAFETY: the handle is live.
        unsafe { events::read_string(self.handle.as_ptr(), sys::data::TITLE) }
    }

    /// The working directory that OSC 7 set, as the library holds it (empty when none was set).
    pub fn cwd(&self) -> String {
        // SAFETY: the handle is live.
        unsafe { events::read_string(self.handle.as_ptr(), sys::data::PWD) }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: the handle is live and is freed exactly once. Freeing the terminal first means that no callback runs
        // after the buffer is freed. The buffer came from `Box::leak` and is freed exactly once.
        unsafe {
            sys::ghostty_terminal_free(self.handle.as_ptr());
            drop(Box::from_raw(self.shared.as_ptr()));
        }
    }
}

#[cfg(test)]
mod tests;
