//! The libghostty-vt binding of Botster Core.
//!
//! libghostty owns every terminal semantic (BUILD.md, architecture facts): this crate parses no terminal bytes, tracks
//! no mode and writes no escape sequence. It owns bookkeeping only: buffers, counters and the copy of what a callback
//! reports.
//!
//! `Terminal` is `Send` and not `Sync`: the worker machine owns it on one thread.

mod encode;
mod events;
mod inspection;
mod modes;
mod query;
mod reads;
mod reply;
mod snapshot;
mod sys;

use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub use botster_core_contract::prelude::{KeyInput, MouseInput, Size};
pub use botster_route_codec::prelude::ModeFlags;
pub use encode::EncodeError;
pub use events::{
    ClipboardEntry, ClipboardLocation, ClipboardWrite, Drained, TerminalEvent, MAX_BUFFERED_BYTES,
    MAX_BUFFERED_EVENTS,
};
pub use inspection::{CellAttributes, CellStyle, Colors, CursorAppearance, Rgb, StyleColor};
pub use query::{Query, QueryKind, QueryStep, Terminator, MAX_SHADOW_REPLY_BYTES};
pub use reads::{CursorCell, ScreenText};
pub use reply::{ReplyError, MAX_REPLY_BYTES};
pub use snapshot::{
    snapshot_format, terminal_identity, Continuation, SnapshotDecodeError, SnapshotError,
    TerminalIdentityParts, CONTINUATION_LIMIT,
};

/// The default bytes of undrained clipboard acknowledgements that `vt_write_until_query` accepts.
pub const DEFAULT_ACK_BACKLOG_BYTES: usize = 1024 * 1024;

/// The default limit of one clipboard write, in bytes of all representations (`CoreLimits.clipboard_bytes`, default
/// 1 MiB).
pub const DEFAULT_CLIPBOARD_BYTES: usize = 1024 * 1024;

/// The default limit, in bytes, of the request that a query reports (`CoreLimits.max_query_bytes`, EV-8).
pub const DEFAULT_QUERY_REQUEST_BYTES: usize = 4096;

/// How much scrollback the model keeps. `Off` backs the testkit control `disable_history` (ST-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum History {
    On,
    Off,
}

/// The most bytes of scrollback that `History::On` keeps. It bounds the memory of one session; libghostty drops the
/// oldest pages beyond it.
pub const HISTORY_MAX_BYTES: usize = 16 * 1024 * 1024;

/// An error of the libghostty-vt library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The library could not allocate.
    OutOfMemory,
    /// An argument was out of range.
    InvalidValue,
    /// Acknowledgements of clipboard writes are waiting past `Terminal::set_ack_backlog_limit`: drain them first.
    AckBacklog,
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
    history: History,
    cell_px: Option<botster_core_contract::prelude::CellPx>,
    color_profile: Option<botster_core_contract::prelude::ColorProfile>,
    ack_backlog_limit: usize,
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
    /// A terminal of the given size. A size that the library cannot hold (zero, or above 65535 in either dimension) is
    /// `Error::InvalidValue`; the library reports an allocation failure as `Error::OutOfMemory`, and never aborts.
    pub fn new(size: &Size, history: History) -> Result<Self, Error> {
        let (cols, rows) = cell_dimensions(size)?;
        let mut handle: sys::Terminal = std::ptr::null_mut();
        // SAFETY: `handle` is a valid out pointer, a null allocator selects the default, and a successful call stores
        // a live handle that `Drop` frees.
        check(unsafe { sys::ghostty_terminal_new(std::ptr::null(), &mut handle, cols, rows) })?;
        let handle = NonNull::new(handle).ok_or(Error::InvalidValue)?;

        let shared = NonNull::from(Box::leak(Box::<events::Shared>::default()));
        let mut terminal = Self {
            handle,
            shared,
            history,
            cell_px: size.cell_px,
            color_profile: None,
            ack_backlog_limit: DEFAULT_ACK_BACKLOG_BYTES,
            _not_sync: PhantomData,
        };
        terminal.set_clipboard_limit(DEFAULT_CLIPBOARD_BYTES);
        terminal.set_image_storage_limit_zero()?;
        terminal.register_callbacks()?;
        terminal.set_history_limit()?;
        terminal.set_query_request_limit_default()?;
        terminal.set_continuation_limit()?;
        terminal.set_cell_px(size.cell_px);
        if let Some(px) = size.cell_px {
            terminal.apply_size(cols, rows, px.width, px.height)?;
        }
        Ok(terminal)
    }

    /// Turn the Kitty graphics protocol off before any write (audit H1, ST-6b): a snapshot holds no images or
    /// placements, so the model must hold none either. The library applies the limit to every screen.
    fn set_image_storage_limit_zero(&self) -> Result<(), Error> {
        let limit: u64 = 0;
        // SAFETY: the handle is live, and the option takes a `uint64_t` pointer.
        check(unsafe {
            sys::ghostty_terminal_set(
                self.handle.as_ptr(),
                sys::opt::KITTY_IMAGE_STORAGE_LIMIT,
                (&limit as *const u64).cast(),
            )
        })
    }

    fn set_query_request_limit_default(&self) -> Result<(), Error> {
        let limit = DEFAULT_QUERY_REQUEST_BYTES;
        // SAFETY: the handle is live, and the option takes a `size_t` pointer.
        check(unsafe {
            sys::ghostty_terminal_set(
                self.handle.as_ptr(),
                sys::opt::QUERY_MAX_BYTES,
                (&limit as *const usize).cast(),
            )
        })
    }

    fn set_history_limit(&self) -> Result<(), Error> {
        let limit: usize = match self.history {
            History::On => HISTORY_MAX_BYTES,
            // A limit of zero disables scrollback and erases retained history.
            History::Off => 0,
        };
        // SAFETY: the handle is live, and the SCROLLBACK_MAX_BYTES option takes a `size_t` pointer.
        check(unsafe {
            sys::ghostty_terminal_set(
                self.handle.as_ptr(),
                sys::opt::SCROLLBACK_MAX_BYTES,
                (&limit as *const usize).cast(),
            )
        })
    }

    /// Register the callbacks that fill the event buffer. The title and the working directory are read in their own
    /// callbacks, so this is the one place that sets effects.
    fn register_callbacks(&self) -> Result<(), Error> {
        let handle = self.handle.as_ptr();
        // SAFETY: the handle is live. The userdata is the boxed buffer, valid until `Drop`. Each callback is passed as
        // the option's value, which is how the library takes a function pointer.
        unsafe {
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::USERDATA,
                self.shared.as_ptr().cast(),
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::BELL,
                events::on_bell as sys::BellFn as *const c_void,
            ))?;
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
            // Queries: the kind and the exact bytes, then the reply that the shadow computes, which is held.
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::QUERY,
                events::on_query as sys::QueryFn as *const c_void,
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::WRITE_PTY,
                events::on_write_pty as sys::WritePtyFn as *const c_void,
            ))?;
            // The shadow's size reports (CSI 14, 16 and 18 t, and mode 2048) need the size and the cell size.
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::SIZE,
                events::on_size as sys::SizeFn as *const c_void,
            ))?;
            // Only to learn the selection and the terminator of a clipboard read. It never replies.
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::CLIPBOARD_READ,
                events::on_clipboard_read as sys::ClipboardReadFn as *const c_void,
            ))?;
        }
        Ok(())
    }

    /// Feed one chunk that holds no query to the model. The callbacks run inside this call and fill the event buffer.
    ///
    /// **PTY output never goes through this function.** It goes through `vt_write_until_query`, which stops at each
    /// query so that the client has its turn to answer (EV-8(d)). A query that this call meets gets the library's
    /// shadow reply in `Drained::pty_writes` at once, before any client could answer, and is counted in
    /// `Drained::unrouted_queries`. It is not buffered, so the buffer stays bounded (EV-8).
    pub fn vt_write(&mut self, bytes: &[u8]) {
        // SAFETY: no callback runs now, so this is the only reference to the buffer.
        unsafe { self.shared.as_mut() }.begin_write(false);
        // SAFETY: the handle is live, and the slice is valid for its length for the duration of the call.
        unsafe { sys::ghostty_terminal_vt_write(self.handle.as_ptr(), bytes.as_ptr(), bytes.len()) }
    }

    /// Feed the chunk up to and including the byte that completes the first query. The step says how many bytes the
    /// model took, and the query, if one ended the step: its kind, its exact request bytes (R-17) and the shadow reply
    /// that the model computed at that point. The model never takes a byte past the query, so a later byte cannot
    /// change the state that the shadow reply or a client's answer depends on (EV-8 c, g).
    ///
    /// When the step took fewer bytes than were offered and found no query, the model kept an ESC that may start the
    /// ST of an unfinished string sequence unconsumed; the caller offers it again with the next bytes.
    pub fn vt_write_until_query(&mut self, bytes: &[u8]) -> Result<QueryStep, Error> {
        // SAFETY: no callback runs now, so this is the only reference to the buffer.
        if unsafe { self.shared.as_ref() }.ack_bytes > self.ack_backlog_limit {
            return Err(Error::AckBacklog);
        }
        // SAFETY: no callback runs now, so this is the only reference to the buffer.
        unsafe { self.shared.as_mut() }.begin_write(true);
        let mut consumed: usize = 0;
        // SAFETY: the handle is live, the slice is valid for its length, and `consumed` is a valid out pointer.
        let code = unsafe {
            sys::ghostty_terminal_vt_write_until_query(
                self.handle.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
                &mut consumed,
            )
        };
        // SAFETY: the call returned, so no callback runs.
        let shared = unsafe { self.shared.as_mut() };
        shared.capture = false;
        let query = shared.held.take();
        match code {
            sys::SUCCESS => Ok(QueryStep { consumed, query }),
            sys::NO_VALUE => Ok(QueryStep {
                consumed,
                query: None,
            }),
            other => Err(Error::from_code(other)),
        }
    }

    /// Set the bytes of undrained clipboard acknowledgements (`Drained::clipboard_acks`) above which
    /// `vt_write_until_query` refuses with `Error::AckBacklog` until the caller drains (admission backpressure,
    /// A13-1b). One chunk of PTY output still adds its acknowledgements, and they are bounded by the chunk: each one
    /// answers a write sequence in it.
    pub fn set_ack_backlog_limit(&mut self, bytes: usize) {
        self.ack_backlog_limit = bytes;
    }

    /// Set the largest clipboard write, in bytes of all representations, that the model is answered SUCCESS for
    /// (`CoreLimits.clipboard_bytes`, A13-1b). A larger write is surfaced as `too_large` without its bytes, and the
    /// model gets IO_ERROR. The decision is made inside the native callback, by size alone.
    pub fn set_clipboard_limit(&mut self, bytes: usize) {
        // SAFETY: no callback runs now, so this is the only reference to the buffer.
        unsafe { self.shared.as_mut() }.clipboard_limit = bytes;
    }

    /// Set the largest request, in bytes, that a query reports (`CoreLimits.max_query_bytes`). A longer sequence
    /// reports its first bytes and sets `request_truncated`.
    pub fn set_query_request_limit(&mut self, bytes: usize) -> Result<(), Error> {
        // SAFETY: the handle is live, and the option takes a `size_t` pointer.
        check(unsafe {
            sys::ghostty_terminal_set(
                self.handle.as_ptr(),
                sys::opt::QUERY_MAX_BYTES,
                (&bytes as *const usize).cast(),
            )
        })
    }

    /// Take what the model reported since the last drain, in observation order.
    pub fn drain_events(&mut self) -> Drained {
        // SAFETY: the buffer is live until `Drop`, and `&mut self` means that no callback runs now, so this is the
        // only reference.
        unsafe { self.shared.as_mut() }.drain()
    }

    /// Apply a new size (SZ-1, SZ-3). `cell_px` is taken when given; without it, the library keeps no cell size.
    pub fn resize(&mut self, size: &Size) -> Result<(), Error> {
        let (cols, rows) = cell_dimensions(size)?;
        let (width, height) = size.cell_px.map_or((0, 0), |px| (px.width, px.height));
        self.apply_size(cols, rows, width, height)?;
        self.set_cell_px(size.cell_px);
        Ok(())
    }

    /// Set the colors that the shadow answers color queries with (EV-8): the default foreground, background and cursor,
    /// and the 256-color palette. A missing palette or cursor restores the library's default. Later queries see the
    /// new profile at once. A palette that does not have exactly 256 entries is `Error::InvalidValue`.
    pub fn set_color_profile(
        &mut self,
        profile: &botster_core_contract::prelude::ColorProfile,
    ) -> Result<(), Error> {
        let rgb = |c: &botster_core_contract::prelude::Rgb| sys::ColorRgb {
            r: c.r,
            g: c.g,
            b: c.b,
        };
        let palette: Option<Vec<sys::ColorRgb>> = match &profile.palette {
            Some(entries) if entries.len() == 256 => Some(entries.iter().map(rgb).collect()),
            Some(_) => return Err(Error::InvalidValue),
            None => None,
        };
        let foreground = rgb(&profile.foreground);
        let background = rgb(&profile.background);
        let cursor = profile.cursor.as_ref().map(rgb);
        let pointer = |value: &Option<sys::ColorRgb>| -> *const c_void {
            value
                .as_ref()
                .map_or(std::ptr::null(), |v| (v as *const sys::ColorRgb).cast())
        };
        let handle = self.handle.as_ptr();
        // SAFETY: the handle is live; each color option takes a `GhosttyColorRgb` pointer (null clears it), the palette
        // takes an array of exactly 256 (null restores the default), and the library copies each value.
        unsafe {
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::COLOR_FOREGROUND,
                (&foreground as *const sys::ColorRgb).cast(),
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::COLOR_BACKGROUND,
                (&background as *const sys::ColorRgb).cast(),
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::COLOR_CURSOR,
                pointer(&cursor),
            ))?;
            check(sys::ghostty_terminal_set(
                handle,
                sys::opt::COLOR_PALETTE,
                palette
                    .as_ref()
                    .map_or(std::ptr::null(), |p| p.as_ptr().cast()),
            ))?;
        }
        self.color_profile = Some(profile.clone());
        Ok(())
    }

    /// The typed query kinds that the shadow answers now (`Core::shadow_answerable_kinds`, EV-8). The set depends on
    /// what the host gave: the pixel reports need `cell_px`, and the shadow has no answer for a clipboard read. The
    /// binding asks the library: a scratch terminal with the same cell size and colors gets one query of each kind, and
    /// a kind is listed when the library gave a reply. The kinds that have no typed label (`Other`) are answered or not
    /// per query: see `Query::shadow_reply`.
    pub fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        // One request of each typed kind. The library finds the kind, and the label comes from `Query::label`.
        const REQUESTS: &[&[u8]] = &[
            b"\x1b]52;c;?\x07",
            b"\x1b[14t",
            b"\x1b[16t",
            b"\x1b[15t",
            b"\x1b[14;2t",
            b"\x1b[19t",
            b"\x1b[11t",
            b"\x1b[13t",
            b"\x1b[13;2t",
            b"\x1b[21t",
            b"\x1b[20t",
            b"\x1b[?996n",
        ];
        let size = Size {
            rows: 24,
            cols: 80,
            cell_px: self.cell_px,
        };
        let Ok(mut scratch) = Terminal::new(&size, History::Off) else {
            return Vec::new();
        };
        if let Some(profile) = &self.color_profile {
            if scratch.set_color_profile(profile).is_err() {
                return Vec::new();
            }
        }
        let mut kinds = Vec::new();
        for request in REQUESTS {
            let Ok(step) = scratch.vt_write_until_query(request) else {
                continue;
            };
            if let Some(query) = step.query {
                if let (Some(label), false) = (query.label(), query.shadow_reply.is_empty()) {
                    kinds.push(label);
                }
            }
        }
        kinds
    }

    fn set_cell_px(&mut self, cell_px: Option<botster_core_contract::prelude::CellPx>) {
        self.cell_px = cell_px;
        // SAFETY: no callback runs now, so this is the only reference to the buffer.
        unsafe { self.shared.as_mut() }.cell_px = cell_px.map(|px| (px.width, px.height));
    }

    fn apply_size(
        &self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        // SAFETY: the handle is live.
        check(unsafe {
            sys::ghostty_terminal_resize(
                self.handle.as_ptr(),
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            )
        })
    }

    /// The plain text of the screen (ST-2). With `history`, it starts with the scrollback; a model that keeps none
    /// (`History::Off`) says so in `history_unavailable` and returns the visible screen.
    pub fn screen_text(&self, history: bool) -> Result<ScreenText, Error> {
        let history_available = self.history == History::On;
        let text = reads::screen_text(
            self.handle.as_ptr(),
            history && history_available,
            self.cols(),
            self.rows(),
        )
        .ok_or(Error::InvalidValue)?;
        Ok(ScreenText {
            text,
            history_unavailable: history && !history_available,
        })
    }

    /// The cursor (ST-3).
    pub fn cursor(&self) -> CursorCell {
        reads::cursor(self.handle.as_ptr())
    }

    /// The cells of a visible row, from column 0: the graphemes of the cell, a space for an empty cell, and nothing for
    /// the second cell of a wide character. `None` when the row is outside the screen. The caller applies the trim
    /// rules of ST-3 to this raw text.
    pub fn row_cells(&self, row: u32) -> Option<Vec<String>> {
        // A row past the screen has no cell: the library refuses its grid reference.
        let row = u16::try_from(row).ok()?;
        (0..self.cols())
            .map(|x| reads::cell_text(self.handle.as_ptr(), x, row))
            .collect()
    }

    /// Encode one key event with the terminal's current modes (IN-9, 5.1A). `repeat` is the caller's: it writes the
    /// result that many times.
    pub fn encode_key(&self, input: &KeyInput) -> Result<Vec<u8>, EncodeError> {
        encode::encode_key(encode::Source::Terminal(self.handle.as_ptr()), input)
    }

    /// Encode one mouse event with the terminal's current modes (IN-9, 5.1A). A wheel event with `notches` is that many
    /// reports, one after the other.
    pub fn encode_mouse(&self, input: &MouseInput) -> Result<Vec<u8>, EncodeError> {
        let size = Size {
            rows: u32::from(self.rows()),
            cols: u32::from(self.cols()),
            cell_px: self.cell_px,
        };
        encode::encode_mouse(encode::Source::Terminal(self.handle.as_ptr()), &size, input)
    }

    /// The bytes of a focus event, or `None` when focus reporting is off (IN-9).
    pub fn encode_focus(&self, focused: bool) -> Option<Vec<u8>> {
        encode::encode_focus(self.modes().focus_reporting, focused)
    }

    /// The marker bytes to write around a paste, or `None` when bracketed paste is off (IN-8). The payload is written
    /// between them exactly as it was sent.
    pub fn paste_frame(&self) -> Option<(Vec<u8>, Vec<u8>)> {
        encode::paste_frame(self.modes().bracketed_paste)
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
        let code = unsafe {
            sys::ghostty_terminal_get(self.handle.as_ptr(), key, (&mut out as *mut u16).cast())
        };
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

/// The dimensions of a size as the library takes them. A dimension above 65535 does not fit the library's type; the
/// library itself refuses zero (`InvalidValue`) in `new` and in `resize`.
fn cell_dimensions(size: &Size) -> Result<(u16, u16), Error> {
    let cols = u16::try_from(size.cols).map_err(|_| Error::InvalidValue)?;
    let rows = u16::try_from(size.rows).map_err(|_| Error::InvalidValue)?;
    Ok((cols, rows))
}

/// Encode a key with explicit modes, for an oracle that has no terminal. `ModeFlags` carries every state that the
/// encoder needs except xterm modifyOtherKeys state 2 and the macOS option-as-alt setting; both are off here.
pub fn encode_key_with_modes(modes: &ModeFlags, input: &KeyInput) -> Result<Vec<u8>, EncodeError> {
    encode::encode_key(
        encode::Source::State(encode::EncoderState::from_mode_flags(modes)),
        input,
    )
}

/// Encode a mouse event with explicit modes and size, for an oracle that has no terminal.
pub fn encode_mouse_with_modes(
    modes: &ModeFlags,
    size: &Size,
    input: &MouseInput,
) -> Result<Vec<u8>, EncodeError> {
    encode::encode_mouse(
        encode::Source::State(encode::EncoderState::from_mode_flags(modes)),
        size,
        input,
    )
}

/// The bytes of a focus event with explicit modes, or `None` when focus reporting is off.
pub fn encode_focus_with_modes(modes: &ModeFlags, focused: bool) -> Option<Vec<u8>> {
    encode::encode_focus(modes.focus_reporting, focused)
}

/// The paste frame with explicit modes, or `None` when bracketed paste is off.
pub fn paste_frame_with_modes(modes: &ModeFlags) -> Option<(Vec<u8>, Vec<u8>)> {
    encode::paste_frame(modes.bracketed_paste)
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
#[cfg(test)]
mod tests_encode;
#[cfg(test)]
mod tests_query;

#[cfg(test)]
mod tests_reply;

#[cfg(test)]
mod tests_snapshot;

#[cfg(test)]
mod tests_color;

#[cfg(test)]
mod tests_archive;
