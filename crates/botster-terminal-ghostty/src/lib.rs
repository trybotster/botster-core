//! Ghostty shadow-terminal adapter boundary for Botster hosts.
//!
//! `botster-core` intentionally keeps the reusable terminal screen contract
//! backend-neutral. This crate is the home for Botster's blessed core-side
//! Ghostty shadow-terminal path: the future concrete adapter that owns
//! authoritative terminal screen and snapshot truth for tmux-like attach,
//! detach, recovery, and replay behavior.
//!
//! The default public surface documents the crate boundary without requiring
//! Ghostty or Zig. Enabling `libghostty-vt` exposes the safe native runtime.
//!
//! Enabling the `libghostty-vt` feature builds the pinned trybotster Ghostty
//! fork from `vendor/ghostty` and links its static `libghostty-vt` archive.
//! Default builds leave that native path disabled so workspace tests do not
//! require Ghostty or Zig.
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

/// Snapshot format label reserved for Ghostty-owned opaque snapshot payloads.
pub const GHOSTTY_SNAPSHOT_FORMAT: &str = "ghostty-terminal-snapshot-v1";

/// Configuration for a future Ghostty-backed terminal screen adapter.
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

    /// Build a Ghostty adapter configuration with explicit retained scrollback.
    #[must_use]
    pub const fn with_max_scrollback(max_scrollback: usize) -> Self {
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

    /// Return the maximum scrollback lines retained by Ghostty.
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
mod native {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fmt;
    use std::ptr::{self, NonNull};

    use botster_core::contract::terminal_screen::{
        TerminalOutputChunk, TerminalScreenSize, TerminalScreenState, TerminalSnapshotPayload,
    };
    use botster_core::engine::TerminalScreenRuntime;

    use crate::sys::{
        ghostty_formatter_format_alloc, ghostty_formatter_free, ghostty_formatter_terminal_new,
        ghostty_free, ghostty_terminal_free, ghostty_terminal_new, ghostty_terminal_resize,
        ghostty_terminal_snapshot_export, ghostty_terminal_snapshot_import,
        ghostty_terminal_vt_write, GhosttyFormatter, GhosttyFormatterFormat,
        GhosttyFormatterScreenExtra, GhosttyFormatterTerminalExtra,
        GhosttyFormatterTerminalOptions, GhosttyResult, GhosttyTerminalOptions, GHOSTTY_SUCCESS,
    };
    use crate::{GhosttyAdapterConfig, GHOSTTY_SNAPSHOT_FORMAT};

    /// Safe owner for a libghostty-vt terminal handle.
    pub struct GhosttyTerminal {
        handle: NonNull<c_void>,
        size: TerminalScreenSize,
        config: GhosttyAdapterConfig,
        last_error: RefCell<Option<GhosttyTerminalError>>,
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
            let result = unsafe {
                ghostty_terminal_new(
                    ptr::null(),
                    &mut terminal,
                    GhosttyTerminalOptions {
                        cols: size.cols,
                        rows: size.rows,
                        max_scrollback: config.max_scrollback(),
                    },
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("new", result));
            }

            let Some(handle) = NonNull::new(terminal) else {
                return Err(GhosttyTerminalError::NullHandle { operation: "new" });
            };

            Ok(Self {
                handle,
                size,
                config,
                last_error: RefCell::new(None),
            })
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

            self.clear_last_error();
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
                ghostty_terminal_snapshot_export(
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

        /// Import an opaque Ghostty terminal snapshot.
        pub fn import_snapshot(
            &mut self,
            payload: &TerminalSnapshotPayload,
        ) -> Result<(), GhosttyTerminalError> {
            let result = unsafe {
                ghostty_terminal_snapshot_import(
                    self.handle.as_ptr(),
                    payload.bytes.as_ptr(),
                    payload.bytes.len(),
                )
            };

            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("snapshot_import", result));
            }

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
            match self.plain_text() {
                Ok(plain_text) => TerminalScreenState::new(self.size, plain_text),
                Err(error) => {
                    self.record_error(error);
                    TerminalScreenState::new(self.size, String::new())
                }
            }
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
        const fn operation(operation: &'static str, result: GhosttyResult) -> Self {
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
        }
    }

    #[cfg(test)]
    mod tests {
        use botster_core::contract::terminal_screen::TerminalScreenSize;

        use super::GhosttyTerminal;

        #[test]
        fn raw_linkage_is_hidden_behind_safe_constructor() {
            let runtime = GhosttyTerminal::new(TerminalScreenSize::new(24, 80))
                .expect("create Ghostty terminal");

            assert_eq!(runtime.size(), TerminalScreenSize::new(24, 80));
        }
    }
}

#[cfg(feature = "libghostty-vt")]
pub use native::{GhosttyTerminal, GhosttyTerminalError};
