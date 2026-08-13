//! Ghostty shadow-terminal adapter boundary for Botster hosts.
//!
//! `botster-core` intentionally keeps the reusable terminal screen contract
//! backend-neutral. This crate is the home for Botster's blessed core-side
//! Ghostty shadow-terminal path: the concrete adapter that owns authoritative
//! terminal screen and snapshot truth for tmux-like attach, detach, recovery,
//! and replay behavior.
//!
//! The default public surface documents the crate boundary without requiring
//! Ghostty or Zig. Enabling `libghostty-vt` exposes the safe native runtime.
//!
//! Enabling the `libghostty-vt` feature builds pinned upstream Ghostty from
//! `vendor/ghostty` and links its static `libghostty-vt` archive. Default
//! builds of this crate leave that native path disabled. First-party host
//! profiles may enable it as part of their default production feature set.
//!
//! Snapshots use upstream's `GHOSTSNP` snapshot format via
//! `ghostty_snapshot_encode_alloc` and the one-shot decoder. This workspace pins
//! trybotster/ghostty (not ghostty-org/ghostty) for Botster production embeds;
//! old alternate snapshot formats are not readable and decoding fails closed.
//!
//! restty remains a web/client rendering path. Clients may consume terminal
//! state and streams, but restty must not become core shadow-terminal
//! infrastructure or the authoritative parser/snapshot owner.
//!
//! ```
//! use botster_core::contract::terminal_screen::{
//!     TerminalOutputChunk, TerminalScreenSize, TerminalScreenState,
//!     TerminalSnapshotPayload,
//! };
//! use botster_core::engine::TerminalScreenRuntime;
//! use botster_terminal_ghostty::{
//!     GhosttyAdapterConfig, GhosttyTerminalRuntime, GHOSTTY_SNAPSHOT_FORMAT,
//! };
//!
//! struct AdapterShape;
//!
//! impl TerminalScreenRuntime for AdapterShape {
//!     fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
//!         TerminalOutputChunk::new(bytes.to_vec())
//!     }
//!
//!     fn resize(&mut self, _size: TerminalScreenSize) {}
//!
//!     fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
//!         TerminalSnapshotPayload::new(
//!             Vec::new(),
//!             TerminalScreenSize::new(24, 80),
//!             Some(GHOSTTY_SNAPSHOT_FORMAT.to_owned()),
//!         )
//!     }
//!
//!     fn replay_snapshot(&mut self, _payload: TerminalSnapshotPayload) {}
//!
//!     fn screen_state(&self) -> TerminalScreenState {
//!         TerminalScreenState::new(TerminalScreenSize::new(24, 80), String::new())
//!     }
//! }
//!
//! fn accepts_ghostty_runtime<R: GhosttyTerminalRuntime>(_runtime: &R) {}
//!
//! accepts_ghostty_runtime(&AdapterShape);
//! ```

use botster_core::engine::TerminalScreenRuntime;

#[cfg(feature = "libghostty-vt")]
mod sys;

#[cfg(feature = "libghostty-vt")]
mod client;

/// Snapshot format label reserved for Ghostty-owned opaque snapshot payloads.
pub const GHOSTTY_SNAPSHOT_FORMAT: &str = "ghostty-terminal-snapshot-v1";

/// Configuration for a Ghostty-backed terminal screen adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GhosttyAdapterConfig {
    snapshot_format: &'static str,
    max_scrollback: usize,
}

impl GhosttyAdapterConfig {
    /// Build the default Ghostty adapter configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            snapshot_format: GHOSTTY_SNAPSHOT_FORMAT,
            max_scrollback: 0,
        }
    }

    /// Build a Ghostty adapter configuration with explicit scrollback bytes.
    ///
    /// Ghostty interprets this as a byte budget for retained scrollback page
    /// allocation. The effective line count is page-quantized and depends on
    /// terminal width; `0` disables scrollback beyond the visible screen.
    #[must_use]
    pub const fn with_max_scrollback_bytes(max_scrollback: usize) -> Self {
        Self {
            snapshot_format: GHOSTTY_SNAPSHOT_FORMAT,
            max_scrollback,
        }
    }

    /// Return the host-owned snapshot format label for Ghostty payloads.
    #[must_use]
    pub const fn snapshot_format(self) -> &'static str {
        self.snapshot_format
    }

    /// Return the maximum scrollback page-allocation bytes retained by Ghostty.
    #[must_use]
    pub const fn max_scrollback(self) -> usize {
        self.max_scrollback
    }
}

impl Default for GhosttyAdapterConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker for runtimes that implement Botster's Ghostty shadow-terminal path.
///
/// This trait deliberately adds no behavior beyond [`TerminalScreenRuntime`].
/// The authoritative runtime contract remains in `botster-core`; this crate
/// only names the concrete Ghostty adapter home.
pub trait GhosttyTerminalRuntime: TerminalScreenRuntime {}

impl<T> GhosttyTerminalRuntime for T where T: TerminalScreenRuntime {}

#[cfg(feature = "libghostty-vt")]
pub(crate) mod native {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fmt;
    use std::ptr::{self, NonNull};

    use botster_core::contract::terminal_screen::{
        TerminalBackendError, TerminalOutputChunk, TerminalScreenSize, TerminalScreenState,
        TerminalSnapshotPayload,
    };
    use botster_core::engine::TerminalScreenRuntime;
    use botster_core::ModeFlags;

    use crate::sys::{
        ghostty_formatter_format_alloc, ghostty_formatter_free, ghostty_formatter_terminal_new,
        ghostty_free, ghostty_snapshot_decoder_decode, ghostty_snapshot_decoder_free,
        ghostty_snapshot_decoder_new_buf, ghostty_snapshot_encode_alloc, ghostty_terminal_free,
        ghostty_terminal_get, ghostty_terminal_new, ghostty_terminal_resize, ghostty_terminal_set,
        ghostty_terminal_vt_write, GhosttyColorRgb, GhosttyFormatter, GhosttyFormatterFormat,
        GhosttyFormatterScreenExtra, GhosttyFormatterTerminalExtra,
        GhosttyFormatterTerminalOptions, GhosttyKittyKeyFlags, GhosttyMode, GhosttyResult,
        GhosttySnapshotDecoder, GhosttyTerminalModeConfig, GhosttyWriter, GHOSTTY_MODE_ALT_SCREEN,
        GHOSTTY_MODE_ALT_SCREEN_SAVE, GHOSTTY_MODE_ANY_MOUSE, GHOSTTY_MODE_BRACKETED_PASTE,
        GHOSTTY_MODE_BUTTON_MOUSE, GHOSTTY_MODE_CURSOR_VISIBLE, GHOSTTY_MODE_DECCKM,
        GHOSTTY_MODE_FOCUS_EVENT, GHOSTTY_MODE_NORMAL_MOUSE, GHOSTTY_MODE_SGR_MOUSE,
        GHOSTTY_NO_VALUE, GHOSTTY_SUCCESS, GHOSTTY_TERMINAL_DATA_COLOR_BACKGROUND,
        GHOSTTY_TERMINAL_DATA_COLOR_CURSOR, GHOSTTY_TERMINAL_DATA_COLOR_FOREGROUND,
        GHOSTTY_TERMINAL_DATA_COLOR_PALETTE, GHOSTTY_TERMINAL_DATA_CURSOR_VISIBLE,
        GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS, GHOSTTY_TERMINAL_DATA_MODE,
        GHOSTTY_TERMINAL_OPT_COLOR_BACKGROUND, GHOSTTY_TERMINAL_OPT_COLOR_CURSOR,
        GHOSTTY_TERMINAL_OPT_COLOR_FOREGROUND, GHOSTTY_TERMINAL_OPT_COLOR_PALETTE,
        GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES, GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
        GHOSTTY_TERMINAL_OPT_USERDATA, GHOSTTY_TERMINAL_OPT_WRITE_PTY,
    };
    use crate::{GhosttyAdapterConfig, GHOSTTY_SNAPSHOT_FORMAT};
    use botster_core::{Rgb, TerminalColorProfile};
    use std::collections::HashMap;

    /// Bytes of unfinished VT sequence retained for snapshot continuation.
    ///
    /// Snapshot encode fails with `GHOSTTY_INVALID_VALUE` when the parser is
    /// mid-sequence and continuation tracking was never enabled, so every
    /// terminal this wrapper owns enables it before any VT byte is written.
    /// The value matches upstream's `c-vt-snapshot` reference example.
    const CONTINUATION_MAX_BYTES: usize = 1024;

    const SNAPSHOT_ENVELOPE_LEN: usize = 10;
    const SNAPSHOT_RECORD_HEADER_LEN: usize = 10;
    const SNAPSHOT_TAG_PAGE: u16 = 3;
    const SNAPSHOT_TAG_READY: u16 = 5;
    const SNAPSHOT_TAG_FINISH: u16 = 6;

    /// A record-aware incremental GHOSTSNP delivery boundary.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct GhosttySnapshotFrame {
        /// The semantic boundary that ends this opaque byte frame.
        pub kind: GhosttySnapshotFrameKind,
        /// Opaque GHOSTSNP bytes. Only this crate interprets their records.
        pub bytes: Vec<u8>,
    }

    /// Semantic boundary for one incremental GHOSTSNP frame.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GhosttySnapshotFrameKind {
        /// Envelope and active terminal state through READY.
        Ready,
        /// Zero or more HISTORY manifests through exactly one PAGE.
        History,
        /// Remaining zero-page HISTORY manifests through FINISH.
        Finish,
    }

    /// Special `TerminalColorProfile` index for default foreground.
    pub const COLOR_INDEX_FOREGROUND: u16 = 0x1000;
    /// Special `TerminalColorProfile` index for default background.
    pub const COLOR_INDEX_BACKGROUND: u16 = 0x1001;
    /// Special `TerminalColorProfile` index for default cursor color.
    pub const COLOR_INDEX_CURSOR: u16 = 0x1002;

    /// Heap-stable effect state passed to libghostty-vt callbacks as userdata.
    struct EffectsState {
        pty_writes: RefCell<Vec<u8>>,
    }

    /// Safe owner for a libghostty-vt terminal handle.
    pub struct GhosttyTerminal {
        handle: NonNull<c_void>,
        size: TerminalScreenSize,
        config: GhosttyAdapterConfig,
        last_error: RefCell<Option<GhosttyTerminalError>>,
        effects: Box<EffectsState>,
    }

    impl fmt::Debug for GhosttyTerminal {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("GhosttyTerminal")
                .field("size", &self.size)
                .field("config", &self.config)
                .field("last_error", &self.last_error())
                .finish_non_exhaustive()
        }
    }

    impl GhosttyTerminal {
        /// Create a new Ghostty terminal for the given size.
        pub fn new(size: TerminalScreenSize) -> Result<Self, GhosttyTerminalError> {
            Self::with_config(size, GhosttyAdapterConfig::default())
        }

        /// Create a new Ghostty terminal with explicit adapter configuration.
        pub fn with_config(
            size: TerminalScreenSize,
            config: GhosttyAdapterConfig,
        ) -> Result<Self, GhosttyTerminalError> {
            let mut terminal = ptr::null_mut();
            let result =
                unsafe { ghostty_terminal_new(ptr::null(), &mut terminal, size.cols, size.rows) };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("new", result));
            }

            let Some(handle) = NonNull::new(terminal) else {
                return Err(GhosttyTerminalError::NullHandle { operation: "new" });
            };

            // Own the handle before any further fallible call so an error path
            // cannot leak it.
            let owned = Self {
                handle,
                size,
                config,
                last_error: RefCell::new(None),
                effects: Box::new(EffectsState {
                    pty_writes: RefCell::new(Vec::new()),
                }),
            };

            let max_scrollback = config.max_scrollback();
            let result = unsafe {
                ghostty_terminal_set(
                    owned.handle.as_ptr(),
                    GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
                    (&raw const max_scrollback).cast(),
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("set_scrollback", result));
            }

            owned.enable_continuation_tracking()?;
            owned.install_effects()?;
            // Color defaults are host policy. Production daemons supply a profile
            // through TerminalScreenRuntime::set_color_profile after construction.

            Ok(owned)
        }

        /// Arm snapshot continuation tracking on the owned handle.
        ///
        /// Must run before any VT byte reaches the parser, and again after a
        /// snapshot import, because a decoded terminal comes back with
        /// continuation tracking disabled.
        fn enable_continuation_tracking(&self) -> Result<(), GhosttyTerminalError> {
            let limit = CONTINUATION_MAX_BYTES;
            let result = unsafe {
                ghostty_terminal_set(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES,
                    (&raw const limit).cast(),
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("set_continuation", result));
            }

            Ok(())
        }

        /// Install userdata + write_pty so query responses can leave the session.
        fn install_effects(&self) -> Result<(), GhosttyTerminalError> {
            let userdata = self.effects.as_ref() as *const EffectsState as *mut c_void;
            let result = unsafe {
                ghostty_terminal_set(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_OPT_USERDATA,
                    userdata.cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("set_userdata", result));
            }

            let callback: crate::sys::GhosttyTerminalWritePtyFn = on_write_pty;
            let result = unsafe {
                ghostty_terminal_set(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_OPT_WRITE_PTY,
                    callback as *const c_void,
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("set_write_pty", result));
            }
            Ok(())
        }

        /// Return the current terminal size known by the wrapper.
        #[must_use]
        pub const fn size(&self) -> TerminalScreenSize {
            self.size
        }

        /// Return the most recent fallible-operation error recorded by an
        /// infallible runtime trait method.
        #[must_use]
        pub fn last_error(&self) -> Option<GhosttyTerminalError> {
            *self.last_error.borrow()
        }

        /// Feed raw terminal output bytes into Ghostty and return them
        /// byte-identically for downstream Botster processing.
        pub fn write_output_bytes(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
            if !bytes.is_empty() {
                unsafe {
                    ghostty_terminal_vt_write(self.handle.as_ptr(), bytes.as_ptr(), bytes.len())
                };
            }

            TerminalOutputChunk::new(bytes.to_vec())
        }

        /// Resize the Ghostty terminal.
        pub fn resize_terminal(
            &mut self,
            size: TerminalScreenSize,
        ) -> Result<(), GhosttyTerminalError> {
            let result = unsafe {
                ghostty_terminal_resize(self.handle.as_ptr(), size.cols, size.rows, 0, 0)
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("resize", result));
            }

            self.size = size;
            self.clear_last_error();
            Ok(())
        }

        /// Export an opaque Ghostty terminal snapshot.
        pub fn export_snapshot(&mut self) -> Result<TerminalSnapshotPayload, GhosttyTerminalError> {
            let bytes = self.export_snapshot_bytes()?;
            self.clear_last_error();
            Ok(TerminalSnapshotPayload::new(
                bytes,
                self.size,
                Some(self.config.snapshot_format().to_owned()),
            ))
        }

        /// Export an opaque Ghostty terminal snapshot as raw bytes.
        pub fn export_snapshot_bytes(&self) -> Result<Vec<u8>, GhosttyTerminalError> {
            let mut out_ptr = ptr::null_mut();
            let mut out_len = 0;
            let result = unsafe {
                ghostty_snapshot_encode_alloc(
                    self.handle.as_ptr(),
                    ptr::null(),
                    &mut out_ptr,
                    &mut out_len,
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("snapshot_export", result));
            }

            let bytes = copy_ghostty_buffer(out_ptr, out_len);
            self.clear_last_error();
            Ok(bytes)
        }

        /// Encode one GHOSTSNP and emit record-aware incremental frames.
        ///
        /// Ghostty writer callback boundaries are arbitrary. This method parses
        /// the fixed envelope and record headers before it emits any frame.
        /// The callback runs synchronously inside `ghostty_snapshot_encode`.
        pub fn export_snapshot_frames<F>(&self, emit: F) -> Result<(), GhosttyTerminalError>
        where
            F: FnMut(GhosttySnapshotFrame) -> bool,
        {
            let mut writer = SnapshotFrameWriter::new(emit);
            let destination = GhosttyWriter {
                write: Some(snapshot_frame_write::<F>),
                userdata: (&raw mut writer).cast(),
            };
            let result =
                unsafe { crate::sys::ghostty_snapshot_encode(self.handle.as_ptr(), destination) };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation(
                    "snapshot_export_stream",
                    result,
                ));
            }
            if !writer.finished || !writer.buffer.is_empty() {
                return Err(GhosttyTerminalError::operation(
                    "snapshot_export_framing",
                    crate::sys::GHOSTTY_INVALID_VALUE,
                ));
            }
            self.clear_last_error();
            Ok(())
        }

        /// Import an opaque Ghostty terminal snapshot.
        ///
        /// Upstream decoding produces a new caller-owned terminal rather than
        /// restoring into an existing one, so this swaps the wrapper's handle
        /// and frees the previous terminal. The wrapper's own handle is
        /// replaced only after decoding succeeds, so a rejected snapshot
        /// leaves the current terminal intact.
        pub fn import_snapshot(
            &mut self,
            payload: &TerminalSnapshotPayload,
        ) -> Result<(), GhosttyTerminalError> {
            let mut decoder: GhosttySnapshotDecoder = ptr::null_mut();
            let result = unsafe {
                ghostty_snapshot_decoder_new_buf(
                    ptr::null(),
                    &mut decoder,
                    payload.bytes.as_ptr(),
                    payload.bytes.len(),
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("snapshot_decoder", result));
            }

            let decoder = DecoderGuard(decoder);
            let mut decoded = ptr::null_mut();
            let result = unsafe { ghostty_snapshot_decoder_decode(decoder.0, &mut decoded) };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("snapshot_import", result));
            }

            let Some(decoded) = NonNull::new(decoded) else {
                return Err(GhosttyTerminalError::NullHandle {
                    operation: "snapshot_import",
                });
            };

            let previous = std::mem::replace(&mut self.handle, decoded);
            unsafe { ghostty_terminal_free(previous.as_ptr()) };

            // A decoded terminal comes back with continuation tracking off; a
            // host that re-exports would otherwise fail on unfinished VT.
            self.enable_continuation_tracking()?;
            // Effect callbacks are terminal-handle local and must be reinstalled.
            self.install_effects()?;

            self.size = payload.size;
            self.clear_last_error();
            Ok(())
        }

        /// Read Ghostty's active screen as plain text.
        pub fn plain_text(&self) -> Result<String, GhosttyTerminalError> {
            let bytes = self.format_plain_bytes()?;
            let plain_text = String::from_utf8_lossy(&bytes).into_owned();
            self.clear_last_error();
            Ok(plain_text)
        }

        fn format_plain_bytes(&self) -> Result<Vec<u8>, GhosttyTerminalError> {
            self.format_with_options(plain_formatter_options())
        }

        fn format_with_options(
            &self,
            options: GhosttyFormatterTerminalOptions,
        ) -> Result<Vec<u8>, GhosttyTerminalError> {
            let mut formatter: GhosttyFormatter = ptr::null_mut();
            let result = unsafe {
                ghostty_formatter_terminal_new(
                    ptr::null(),
                    &mut formatter,
                    self.handle.as_ptr(),
                    options,
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("formatter_new", result));
            }

            let formatter = FormatterGuard(formatter);
            let mut out_ptr = ptr::null_mut();
            let mut out_len = 0;
            let result = unsafe {
                ghostty_formatter_format_alloc(formatter.0, ptr::null(), &mut out_ptr, &mut out_len)
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("formatter_format", result));
            }

            let bytes = copy_ghostty_buffer(out_ptr, out_len);
            self.clear_last_error();
            Ok(bytes)
        }

        fn record_error(&self, error: GhosttyTerminalError) {
            *self.last_error.borrow_mut() = Some(error);
        }

        fn clear_last_error(&self) {
            *self.last_error.borrow_mut() = None;
        }

        fn mode_is_set(&self, mode: GhosttyMode) -> Result<bool, GhosttyTerminalError> {
            let mut query = GhosttyTerminalModeConfig { mode, value: false };
            let result = unsafe {
                ghostty_terminal_get(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_DATA_MODE,
                    (&raw mut query).cast(),
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("mode_get", result));
            }

            Ok(query.value)
        }

        /// Read the load-bearing mouse mode bitmask.
        pub fn mouse_mode(&self) -> Result<u8, GhosttyTerminalError> {
            let mut mouse_mode = 0;
            if self.mode_is_set(GHOSTTY_MODE_NORMAL_MOUSE)? {
                mouse_mode |= 1;
            }
            if self.mode_is_set(GHOSTTY_MODE_ANY_MOUSE)? {
                mouse_mode |= 2;
            }
            if self.mode_is_set(GHOSTTY_MODE_BUTTON_MOUSE)? {
                mouse_mode |= 4;
            }
            if self.mode_is_set(GHOSTTY_MODE_SGR_MOUSE)? {
                mouse_mode |= 8;
            }

            Ok(mouse_mode)
        }

        /// Read complete production mode flags from the native terminal.
        pub fn read_mode_flags(&self) -> Result<ModeFlags, GhosttyTerminalError> {
            let mut kitty_flags: GhosttyKittyKeyFlags = 0;
            let result = unsafe {
                ghostty_terminal_get(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS,
                    (&raw mut kitty_flags).cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("kitty_flags", result));
            }

            let mut cursor_visible = true;
            let cursor_result = unsafe {
                ghostty_terminal_get(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_DATA_CURSOR_VISIBLE,
                    (&raw mut cursor_visible).cast(),
                )
            };
            if cursor_result != GHOSTTY_SUCCESS {
                // Fall back to the DEC mode probe when the dedicated data id fails.
                cursor_visible = self.mode_is_set(GHOSTTY_MODE_CURSOR_VISIBLE)?;
            }

            let alt_screen = self.mode_is_set(GHOSTTY_MODE_ALT_SCREEN)?
                || self.mode_is_set(GHOSTTY_MODE_ALT_SCREEN_SAVE)?;

            Ok(ModeFlags {
                kitty_enabled: kitty_flags != 0,
                cursor_visible,
                bracketed_paste: self.mode_is_set(GHOSTTY_MODE_BRACKETED_PASTE)?,
                mouse_mode: self.mouse_mode()?,
                alt_screen,
                focus_reporting: self.mode_is_set(GHOSTTY_MODE_FOCUS_EVENT)?,
                application_cursor: self.mode_is_set(GHOSTTY_MODE_DECCKM)?,
            })
        }

        /// Drain PTY query responses generated during the last VT write batch.
        pub fn drain_pty_writes(&mut self) -> Vec<u8> {
            self.effects.pty_writes.borrow_mut().drain(..).collect()
        }

        /// Apply a Botster color profile as Ghostty defaults.
        pub fn apply_color_profile(
            &mut self,
            profile: &TerminalColorProfile,
        ) -> Result<(), GhosttyTerminalError> {
            if let Some(color) = profile.colors.get(&COLOR_INDEX_FOREGROUND) {
                self.set_default_color(GHOSTTY_TERMINAL_OPT_COLOR_FOREGROUND, *color)?;
            }
            if let Some(color) = profile.colors.get(&COLOR_INDEX_BACKGROUND) {
                self.set_default_color(GHOSTTY_TERMINAL_OPT_COLOR_BACKGROUND, *color)?;
            }
            if let Some(color) = profile.colors.get(&COLOR_INDEX_CURSOR) {
                self.set_default_color(GHOSTTY_TERMINAL_OPT_COLOR_CURSOR, *color)?;
            }

            let mut palette = [GhosttyColorRgb { r: 0, g: 0, b: 0 }; 256];
            let result = unsafe {
                ghostty_terminal_get(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_DATA_COLOR_PALETTE,
                    palette.as_mut_ptr().cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("palette_get", result));
            }

            let mut palette_changed = false;
            for (index, color) in &profile.colors {
                if *index < 256 {
                    palette[*index as usize] = GhosttyColorRgb {
                        r: color.r,
                        g: color.g,
                        b: color.b,
                    };
                    palette_changed = true;
                }
            }
            if palette_changed {
                let result = unsafe {
                    ghostty_terminal_set(
                        self.handle.as_ptr(),
                        GHOSTTY_TERMINAL_OPT_COLOR_PALETTE,
                        palette.as_ptr().cast(),
                    )
                };
                if result != GHOSTTY_SUCCESS {
                    return Err(GhosttyTerminalError::operation("palette_set", result));
                }
            }

            self.clear_last_error();
            Ok(())
        }

        fn set_default_color(
            &self,
            option: crate::sys::GhosttyTerminalOption,
            color: Rgb,
        ) -> Result<(), GhosttyTerminalError> {
            let rgb = GhosttyColorRgb {
                r: color.r,
                g: color.g,
                b: color.b,
            };
            let result = unsafe {
                ghostty_terminal_set(self.handle.as_ptr(), option, (&raw const rgb).cast())
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("color_set", result));
            }
            Ok(())
        }

        /// Read Ghostty-owned palette + special colors as a Botster profile.
        pub fn read_color_profile(&self) -> Result<TerminalColorProfile, GhosttyTerminalError> {
            let mut colors = HashMap::new();
            let mut palette = [GhosttyColorRgb { r: 0, g: 0, b: 0 }; 256];
            let result = unsafe {
                ghostty_terminal_get(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_DATA_COLOR_PALETTE,
                    palette.as_mut_ptr().cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("palette_get", result));
            }
            for (index, entry) in palette.iter().enumerate() {
                colors.insert(
                    index as u16,
                    Rgb {
                        r: entry.r,
                        g: entry.g,
                        b: entry.b,
                    },
                );
            }

            self.read_optional_color(
                GHOSTTY_TERMINAL_DATA_COLOR_FOREGROUND,
                COLOR_INDEX_FOREGROUND,
                &mut colors,
            )?;
            self.read_optional_color(
                GHOSTTY_TERMINAL_DATA_COLOR_BACKGROUND,
                COLOR_INDEX_BACKGROUND,
                &mut colors,
            )?;
            self.read_optional_color(
                GHOSTTY_TERMINAL_DATA_COLOR_CURSOR,
                COLOR_INDEX_CURSOR,
                &mut colors,
            )?;

            Ok(TerminalColorProfile { colors })
        }

        fn read_optional_color(
            &self,
            data: crate::sys::GhosttyTerminalData,
            index: u16,
            colors: &mut HashMap<u16, Rgb>,
        ) -> Result<(), GhosttyTerminalError> {
            let mut rgb = GhosttyColorRgb { r: 0, g: 0, b: 0 };
            let result =
                unsafe { ghostty_terminal_get(self.handle.as_ptr(), data, (&raw mut rgb).cast()) };
            if result == GHOSTTY_NO_VALUE {
                return Ok(());
            }
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("color_get", result));
            }
            colors.insert(
                index,
                Rgb {
                    r: rgb.r,
                    g: rgb.g,
                    b: rgb.b,
                },
            );
            Ok(())
        }
    }

    unsafe extern "C" fn on_write_pty(
        _terminal: *mut c_void,
        userdata: *mut c_void,
        data: *const u8,
        len: usize,
    ) {
        if userdata.is_null() || data.is_null() || len == 0 {
            return;
        }
        // SAFETY: userdata is the stable EffectsState boxed by GhosttyTerminal for
        // the lifetime of the handle; data/len are valid for this callback only.
        unsafe {
            let effects = &*(userdata as *const EffectsState);
            let bytes = std::slice::from_raw_parts(data, len);
            effects.pty_writes.borrow_mut().extend_from_slice(bytes);
        }
    }

    struct SnapshotFrameWriter<F> {
        emit: F,
        buffer: Vec<u8>,
        parsed: usize,
        envelope_seen: bool,
        ready_seen: bool,
        finished: bool,
    }

    impl<F> SnapshotFrameWriter<F>
    where
        F: FnMut(GhosttySnapshotFrame) -> bool,
    {
        fn new(emit: F) -> Self {
            Self {
                emit,
                buffer: Vec::new(),
                parsed: 0,
                envelope_seen: false,
                ready_seen: false,
                finished: false,
            }
        }

        fn write(&mut self, bytes: &[u8]) -> bool {
            if self.finished {
                return false;
            }
            self.buffer.extend_from_slice(bytes);
            self.parse_complete_records()
        }

        fn parse_complete_records(&mut self) -> bool {
            if !self.envelope_seen {
                if self.buffer.len() < SNAPSHOT_ENVELOPE_LEN {
                    return true;
                }
                if &self.buffer[..8] != b"GHOSTSNP" || self.buffer[8..10] != [1, 0] {
                    return false;
                }
                self.parsed = SNAPSHOT_ENVELOPE_LEN;
                self.envelope_seen = true;
            }

            loop {
                let remaining = &self.buffer[self.parsed..];
                if remaining.len() < SNAPSHOT_RECORD_HEADER_LEN {
                    return true;
                }
                let tag = u16::from_le_bytes([remaining[0], remaining[1]]);
                let payload_len =
                    u32::from_le_bytes([remaining[2], remaining[3], remaining[4], remaining[5]])
                        as usize;
                let Some(record_len) = SNAPSHOT_RECORD_HEADER_LEN.checked_add(payload_len) else {
                    return false;
                };
                if remaining.len() < record_len {
                    return true;
                }
                self.parsed += record_len;

                let kind = if tag == SNAPSHOT_TAG_READY {
                    if self.ready_seen || payload_len != 0 {
                        return false;
                    }
                    self.ready_seen = true;
                    Some(GhosttySnapshotFrameKind::Ready)
                } else if tag == SNAPSHOT_TAG_PAGE && self.ready_seen {
                    Some(GhosttySnapshotFrameKind::History)
                } else if tag == SNAPSHOT_TAG_FINISH {
                    if !self.ready_seen || payload_len != 0 {
                        return false;
                    }
                    self.finished = true;
                    Some(GhosttySnapshotFrameKind::Finish)
                } else {
                    None
                };

                if let Some(kind) = kind {
                    let frame = GhosttySnapshotFrame {
                        kind,
                        bytes: self.buffer.drain(..self.parsed).collect(),
                    };
                    self.parsed = 0;
                    if !(self.emit)(frame) {
                        return false;
                    }
                }
            }
        }
    }

    unsafe extern "C" fn snapshot_frame_write<F>(
        userdata: *mut c_void,
        data: *const u8,
        len: usize,
    ) -> bool
    where
        F: FnMut(GhosttySnapshotFrame) -> bool,
    {
        if userdata.is_null() || data.is_null() || len == 0 {
            return false;
        }
        let writer = unsafe { &mut *userdata.cast::<SnapshotFrameWriter<F>>() };
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        writer.write(bytes)
    }

    impl TerminalScreenRuntime for GhosttyTerminal {
        fn write_output(&mut self, bytes: &[u8]) -> TerminalOutputChunk {
            self.write_output_bytes(bytes)
        }

        fn resize(&mut self, size: TerminalScreenSize) {
            if let Err(error) = self.resize_terminal(size) {
                self.record_error(error);
            }
        }

        fn capture_snapshot(&mut self) -> TerminalSnapshotPayload {
            match self.export_snapshot() {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.record_error(error);
                    TerminalSnapshotPayload::new(
                        Vec::new(),
                        self.size,
                        Some(GHOSTTY_SNAPSHOT_FORMAT.to_owned()),
                    )
                }
            }
        }

        fn replay_snapshot(&mut self, payload: TerminalSnapshotPayload) {
            if let Err(error) = self.import_snapshot(&payload) {
                self.record_error(error);
            }
        }

        fn screen_state(&self) -> TerminalScreenState {
            let plain_text = match self.plain_text() {
                Ok(plain_text) => plain_text,
                Err(error) => {
                    self.record_error(error);
                    String::new()
                }
            };
            let mode_flags = self.read_mode_flags().unwrap_or_else(|error| {
                self.record_error(error);
                ModeFlags::default()
            });
            let color_profile = self.read_color_profile().ok();
            TerminalScreenState {
                size: self.size,
                plain_text,
                title: None,
                cwd: None,
                mode_flags,
                color_profile,
            }
        }

        fn mode_flags(&self) -> Result<ModeFlags, TerminalBackendError> {
            self.read_mode_flags().map_err(|error| {
                TerminalBackendError::operation_failed("mode_flags", error.to_string())
            })
        }

        fn set_color_profile(
            &mut self,
            profile: TerminalColorProfile,
        ) -> Result<(), TerminalBackendError> {
            self.apply_color_profile(&profile).map_err(|error| {
                TerminalBackendError::operation_failed("set_color_profile", error.to_string())
            })
        }

        fn color_profile(&self) -> Result<Option<TerminalColorProfile>, TerminalBackendError> {
            self.read_color_profile().map(Some).map_err(|error| {
                TerminalBackendError::operation_failed("color_profile", error.to_string())
            })
        }

        fn drain_pty_writes(&mut self) -> Vec<u8> {
            GhosttyTerminal::drain_pty_writes(self)
        }

        fn last_error(&self) -> Option<String> {
            GhosttyTerminal::last_error(self).map(|error| error.to_string())
        }
    }

    impl Drop for GhosttyTerminal {
        fn drop(&mut self) {
            unsafe { ghostty_terminal_free(self.handle.as_ptr()) };
        }
    }

    /// Typed errors returned by the safe Ghostty terminal wrapper.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GhosttyTerminalError {
        /// Ghostty reported a non-success result code.
        OperationFailed {
            /// Operation that failed.
            operation: &'static str,
            /// Raw libghostty-vt result code.
            result: GhosttyResult,
        },
        /// Ghostty reported success but returned a null handle.
        NullHandle {
            /// Operation that failed.
            operation: &'static str,
        },
    }

    impl GhosttyTerminalError {
        pub(crate) const fn operation(operation: &'static str, result: GhosttyResult) -> Self {
            Self::OperationFailed { operation, result }
        }
    }

    impl fmt::Display for GhosttyTerminalError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::OperationFailed { operation, result } => {
                    write!(f, "{operation} failed with Ghostty result {result}")
                }
                Self::NullHandle { operation } => {
                    write!(f, "{operation} returned a null Ghostty handle")
                }
            }
        }
    }

    impl std::error::Error for GhosttyTerminalError {}

    struct FormatterGuard(GhosttyFormatter);

    impl Drop for FormatterGuard {
        fn drop(&mut self) {
            unsafe { ghostty_formatter_free(self.0) };
        }
    }

    struct DecoderGuard(GhosttySnapshotDecoder);

    impl Drop for DecoderGuard {
        fn drop(&mut self) {
            unsafe { ghostty_snapshot_decoder_free(self.0) };
        }
    }

    fn copy_ghostty_buffer(ptr: *mut u8, len: usize) -> Vec<u8> {
        let bytes = if ptr.is_null() || len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
        };

        if !ptr.is_null() {
            unsafe { ghostty_free(ptr::null(), ptr, len) };
        }

        bytes
    }

    fn plain_formatter_options() -> GhosttyFormatterTerminalOptions {
        let screen = GhosttyFormatterScreenExtra {
            size: std::mem::size_of::<GhosttyFormatterScreenExtra>(),
            cursor: false,
            style: false,
            hyperlink: false,
            protection: false,
            kitty_keyboard: false,
            charsets: false,
        };
        let extra = GhosttyFormatterTerminalExtra {
            size: std::mem::size_of::<GhosttyFormatterTerminalExtra>(),
            palette: false,
            modes: false,
            scrolling_region: false,
            tabstops: false,
            pwd: false,
            keyboard: false,
            screen,
        };

        GhosttyFormatterTerminalOptions {
            size: std::mem::size_of::<GhosttyFormatterTerminalOptions>(),
            emit: GhosttyFormatterFormat::Plain,
            unwrap: false,
            trim: true,
            extra,
            // Format the whole screen rather than a selection range.
            selection: ptr::null(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            GhosttySnapshotFrameKind, SnapshotFrameWriter, SNAPSHOT_TAG_FINISH, SNAPSHOT_TAG_PAGE,
            SNAPSHOT_TAG_READY,
        };

        fn framed_record(tag: u16, payload: &[u8]) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(10 + payload.len());
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.extend_from_slice(payload);
            bytes
        }

        fn callback_boundary_fixture() -> Vec<u8> {
            let mut bytes = b"GHOSTSNP\x01\x00".to_vec();
            bytes.extend(framed_record(1, b"terminal"));
            bytes.extend(framed_record(SNAPSHOT_TAG_READY, b""));
            bytes.extend(framed_record(4, b"history"));
            bytes.extend(framed_record(SNAPSHOT_TAG_PAGE, b"page-one"));
            bytes.extend(framed_record(SNAPSHOT_TAG_PAGE, b"page-two"));
            bytes.extend(framed_record(SNAPSHOT_TAG_FINISH, b""));
            bytes
        }

        #[test]
        fn snapshot_framer_ignores_writer_callback_boundaries() {
            let fixture = callback_boundary_fixture();
            let mut one_callback = Vec::new();
            let mut writer = SnapshotFrameWriter::new(|frame| {
                one_callback.push(frame);
                true
            });
            assert!(writer.write(&fixture));
            assert!(writer.finished);
            assert!(writer.buffer.is_empty());

            let mut one_byte_callbacks = Vec::new();
            let mut writer = SnapshotFrameWriter::new(|frame| {
                one_byte_callbacks.push(frame);
                true
            });
            for byte in fixture.iter().copied() {
                assert!(writer.write(&[byte]));
            }
            assert!(writer.finished);
            assert!(writer.buffer.is_empty());

            assert_eq!(one_callback, one_byte_callbacks);
            assert_eq!(
                one_callback
                    .iter()
                    .map(|frame| frame.kind)
                    .collect::<Vec<_>>(),
                vec![
                    GhosttySnapshotFrameKind::Ready,
                    GhosttySnapshotFrameKind::History,
                    GhosttySnapshotFrameKind::History,
                    GhosttySnapshotFrameKind::Finish,
                ]
            );
            assert_eq!(
                one_callback
                    .iter()
                    .filter(|frame| frame.kind == GhosttySnapshotFrameKind::History)
                    .map(|frame| {
                        frame
                            .bytes
                            .windows(2)
                            .filter(|bytes| *bytes == SNAPSHOT_TAG_PAGE.to_le_bytes())
                            .count()
                    })
                    .collect::<Vec<_>>(),
                vec![1, 1]
            );
        }

        use botster_core::contract::terminal_screen::TerminalScreenSize;
        use botster_core::engine::TerminalScreenRuntime;

        use super::GhosttyTerminal;

        #[test]
        fn raw_linkage_is_hidden_behind_safe_constructor() {
            let runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");

            assert_eq!(runtime.size(), TerminalScreenSize::new(24, 80));
        }

        #[test]
        fn mouse_mode_tracks_decset_and_decrst_on_the_native_terminal() {
            let mut runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");

            assert_eq!(runtime.mouse_mode().expect("query default modes"), 0);

            for (mode, expected) in [(1000, 1), (1003, 2), (1002, 4), (1006, 8)] {
                runtime.write_output(format!("\x1b[?{mode}h").as_bytes());
                assert_eq!(
                    runtime.mouse_mode().expect("query set mouse mode"),
                    expected
                );

                runtime.write_output(format!("\x1b[?{mode}l").as_bytes());
                assert_eq!(runtime.mouse_mode().expect("query reset mouse mode"), 0);
            }

            runtime.write_output(b"\x1b[?1000h\x1b[?1006h");
            assert_eq!(runtime.mouse_mode().expect("query combined modes"), 9);
            assert_eq!(
                runtime
                    .mode_flags()
                    .expect("query authoritative mode flags")
                    .mouse_mode,
                9
            );

            runtime.write_output(b"\x1b[?1000l\x1b[?1006l");
            assert_eq!(runtime.mouse_mode().expect("query fully reset modes"), 0);
            assert_eq!(
                runtime
                    .mode_flags()
                    .expect("query reset authoritative mode flags")
                    .mouse_mode,
                0
            );
        }

        #[test]
        fn mode_flags_track_kitty_cursor_paste_alt_focus_and_app_cursor() {
            let mut runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");

            let defaults = runtime.mode_flags().expect("default mode flags");
            assert!(!defaults.kitty_enabled);
            assert!(!defaults.bracketed_paste);
            assert!(!defaults.alt_screen);
            assert!(!defaults.focus_reporting);
            assert!(!defaults.application_cursor);

            runtime.write_output(b"\x1b[=1;1u");
            assert!(runtime.mode_flags().expect("kitty on").kitty_enabled);

            runtime.write_output(b"\x1b[?25l");
            assert!(!runtime.mode_flags().expect("cursor hide").cursor_visible);
            runtime.write_output(b"\x1b[?25h");
            assert!(runtime.mode_flags().expect("cursor show").cursor_visible);

            runtime.write_output(b"\x1b[?2004h");
            assert!(runtime.mode_flags().expect("paste on").bracketed_paste);
            runtime.write_output(b"\x1b[?2004l");
            assert!(!runtime.mode_flags().expect("paste off").bracketed_paste);

            runtime.write_output(b"\x1b[?1049h");
            assert!(runtime.mode_flags().expect("alt on").alt_screen);
            runtime.write_output(b"\x1b[?1049l");
            assert!(!runtime.mode_flags().expect("alt off").alt_screen);

            runtime.write_output(b"\x1b[?1004h");
            assert!(runtime.mode_flags().expect("focus on").focus_reporting);
            runtime.write_output(b"\x1b[?1004l");
            assert!(!runtime.mode_flags().expect("focus off").focus_reporting);

            runtime.write_output(b"\x1b[?1h");
            assert!(
                runtime
                    .mode_flags()
                    .expect("app cursor on")
                    .application_cursor
            );
            runtime.write_output(b"\x1b[?1l");
            assert!(
                !runtime
                    .mode_flags()
                    .expect("app cursor off")
                    .application_cursor
            );
        }

        #[test]
        fn write_pty_captures_osc_color_query_replies() {
            use botster_core::{Rgb, TerminalColorProfile};
            use std::collections::HashMap;

            let mut runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");
            let mut colors = HashMap::new();
            colors.insert(
                super::COLOR_INDEX_FOREGROUND,
                Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33,
                },
            );
            colors.insert(
                super::COLOR_INDEX_BACKGROUND,
                Rgb {
                    r: 0x44,
                    g: 0x55,
                    b: 0x66,
                },
            );
            colors.insert(
                super::COLOR_INDEX_CURSOR,
                Rgb {
                    r: 0x77,
                    g: 0x88,
                    b: 0x99,
                },
            );
            runtime
                .set_color_profile(TerminalColorProfile { colors })
                .expect("apply defaults");

            let _ = runtime.drain_pty_writes();
            runtime.write_output(b"\x1b]10;?\x1b\\");
            runtime.write_output(b"\x1b]11;?\x1b\\");
            runtime.write_output(b"\x1b]12;?\x1b\\");
            let replies = runtime.drain_pty_writes();
            let text = String::from_utf8_lossy(&replies);
            assert!(
                text.contains("11") || text.contains("22") || text.contains("rgb:"),
                "expected OSC color replies in write_pty output, got {text:?}"
            );
            assert!(
                !replies.is_empty(),
                "write_pty must capture pre-attach replies"
            );
        }

        #[test]
        fn color_profile_round_trips_palette_and_special_colors() {
            use botster_core::{Rgb, TerminalColorProfile};
            use std::collections::HashMap;

            let mut runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");
            let mut colors = HashMap::new();
            colors.insert(1, Rgb { r: 255, g: 0, b: 0 });
            colors.insert(super::COLOR_INDEX_FOREGROUND, Rgb { r: 1, g: 2, b: 3 });
            runtime
                .set_color_profile(TerminalColorProfile {
                    colors: colors.clone(),
                })
                .expect("set profile");

            let read = runtime
                .color_profile()
                .expect("read profile")
                .expect("some");
            assert_eq!(read.colors.get(&1), Some(&Rgb { r: 255, g: 0, b: 0 }));
            assert_eq!(
                read.colors.get(&super::COLOR_INDEX_FOREGROUND),
                Some(&Rgb { r: 1, g: 2, b: 3 })
            );
        }
    }
}

#[cfg(feature = "libghostty-vt")]
pub use client::{
    CursorProjection, CursorStyle, GhosttyClientProjection, GhosttySnapshotDecodeProgress,
    ProjectedCell, ProjectedWide, ScrollOp, ScrollbarState, ViewportProjection, GHOSTSNP_MAGIC,
};

#[cfg(feature = "libghostty-vt")]
pub use native::{
    GhosttySnapshotFrame, GhosttySnapshotFrameKind, GhosttyTerminal, GhosttyTerminalError,
    COLOR_INDEX_BACKGROUND, COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND,
};
