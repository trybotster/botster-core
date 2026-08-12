//! Client-facing read-only Ghostty projection for Hub GHOSTSNP install.
//!
//! This module is the pin surface first-party TUI consumers use: install
//! opaque Hub Snapshot bytes, apply later TerminalOutput, and project
//! viewport cells/styles without owning a PTY or answering OSC queries.

use std::collections::HashMap;
use std::ffi::{c_int, c_void};
use std::fmt;
use std::ptr::{self, NonNull};

use botster_core::contract::terminal_screen::TerminalScreenSize;
use botster_core::{ModeFlags, Rgb, TerminalColorProfile};

use crate::native::{
    GhosttyTerminalError, COLOR_INDEX_BACKGROUND, COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND,
};
use crate::sys::{
    ghostty_cell_get, ghostty_render_state_free, ghostty_render_state_get,
    ghostty_render_state_new, ghostty_render_state_row_cells_free,
    ghostty_render_state_row_cells_get, ghostty_render_state_row_cells_new,
    ghostty_render_state_row_cells_next, ghostty_render_state_row_get,
    ghostty_render_state_row_iterator_free, ghostty_render_state_row_iterator_new,
    ghostty_render_state_row_iterator_next, ghostty_render_state_update,
    ghostty_snapshot_decoder_decode, ghostty_snapshot_decoder_free,
    ghostty_snapshot_decoder_new_buf, ghostty_terminal_free, ghostty_terminal_get,
    ghostty_terminal_new, ghostty_terminal_resize, ghostty_terminal_scroll_viewport,
    ghostty_terminal_set, ghostty_terminal_vt_write, GhosttyBuffer, GhosttyCell, GhosttyCellWide,
    GhosttyColorRgb, GhosttyKittyKeyFlags, GhosttyMode, GhosttyRenderState,
    GhosttyRenderStateCursorVisualStyle, GhosttyRenderStateRowCells, GhosttyRenderStateRowIterator,
    GhosttySnapshotDecoder, GhosttyStyle, GhosttyStyleColor, GhosttyStyleColorTag,
    GhosttyStyleColorValue, GhosttyTerminalModeConfig, GhosttyTerminalScrollViewport,
    GhosttyTerminalScrollViewportTag, GhosttyTerminalScrollViewportValue, GhosttyTerminalScrollbar,
    GHOSTTY_CELL_DATA_WIDE, GHOSTTY_INVALID_VALUE, GHOSTTY_MODE_ALT_SCREEN,
    GHOSTTY_MODE_ALT_SCREEN_SAVE, GHOSTTY_MODE_ANY_MOUSE, GHOSTTY_MODE_BRACKETED_PASTE,
    GHOSTTY_MODE_BUTTON_MOUSE, GHOSTTY_MODE_CURSOR_VISIBLE, GHOSTTY_MODE_DECCKM,
    GHOSTTY_MODE_FOCUS_EVENT, GHOSTTY_MODE_NORMAL_MOUSE, GHOSTTY_MODE_SGR_MOUSE, GHOSTTY_NO_VALUE,
    GHOSTTY_OUT_OF_SPACE, GHOSTTY_RENDER_STATE_DATA_COLOR_BACKGROUND,
    GHOSTTY_RENDER_STATE_DATA_COLOR_FOREGROUND, GHOSTTY_RENDER_STATE_DATA_COLS,
    GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE,
    GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_X, GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_Y,
    GHOSTTY_RENDER_STATE_DATA_CURSOR_VISIBLE, GHOSTTY_RENDER_STATE_DATA_CURSOR_VISUAL_STYLE,
    GHOSTTY_RENDER_STATE_DATA_ROWS, GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR,
    GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR,
    GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8, GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW,
    GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE, GHOSTTY_RENDER_STATE_ROW_DATA_CELLS,
    GHOSTTY_SUCCESS, GHOSTTY_TERMINAL_DATA_COLOR_BACKGROUND, GHOSTTY_TERMINAL_DATA_COLOR_CURSOR,
    GHOSTTY_TERMINAL_DATA_COLOR_FOREGROUND, GHOSTTY_TERMINAL_DATA_COLOR_PALETTE,
    GHOSTTY_TERMINAL_DATA_COLS, GHOSTTY_TERMINAL_DATA_CURSOR_VISIBLE,
    GHOSTTY_TERMINAL_DATA_KITTY_KEYBOARD_FLAGS, GHOSTTY_TERMINAL_DATA_MODE,
    GHOSTTY_TERMINAL_DATA_ROWS, GHOSTTY_TERMINAL_DATA_SCROLLBAR,
    GHOSTTY_TERMINAL_OPT_CONTINUATION_MAX_BYTES, GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
};
use crate::GhosttyAdapterConfig;

/// Upstream GHOSTSNP snapshot envelope magic.
pub const GHOSTSNP_MAGIC: &[u8] = b"GHOSTSNP";

/// Bytes of unfinished VT sequence retained for snapshot continuation.
const CONTINUATION_MAX_BYTES: usize = 1024;

/// Wide-cell kind projected for a client renderer map.
///
/// Marked `non_exhaustive` so adding renderer-facing wide kinds is not a
/// silent exhaustive-match break for downstream TUI pins
/// ([[botster core public enums are breaking until non exhaustive is decided]]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProjectedWide {
    /// Single-column cell.
    Narrow,
    /// Wide character head (occupies two columns).
    Wide,
    /// Spacer after a wide character; do not paint text.
    SpacerTail,
    /// Spacer head at soft-wrap for a wide character.
    SpacerHead,
}

/// Cursor visual style projected for a client renderer.
///
/// Marked `non_exhaustive` so pin consumers must handle future styles without
/// a coordinated breaking upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CursorStyle {
    /// Block cursor.
    Block,
    /// Bar cursor.
    Bar,
    /// Underline cursor.
    Underline,
    /// Hollow block cursor.
    Hollow,
}

/// Scroll navigation operations against Ghostty-owned viewport state.
///
/// Marked `non_exhaustive` so additional Ghostty scroll behaviors can land
/// without forcing exhaustive match rewrites on every TUI pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollOp {
    /// Jump to the top of retained history.
    Top,
    /// Jump to the live bottom edge.
    Bottom,
    /// Relative scroll; negative moves into history (up).
    Delta(i32),
}

/// One projected viewport cell with resolved colors and style attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedCell {
    /// Base + combining grapheme cluster as one UTF-8 string (empty if none).
    pub grapheme: String,
    /// Wide-cell kind.
    pub wide: ProjectedWide,
    /// Resolved foreground RGB ready for a renderer map.
    pub fg: Rgb,
    /// Resolved background RGB ready for a renderer map.
    pub bg: Rgb,
    /// Bold attribute.
    pub bold: bool,
    /// Italic attribute.
    pub italic: bool,
    /// Underline attribute (any non-none underline style).
    pub underline: bool,
    /// Inverse attribute.
    pub inverse: bool,
    /// Faint attribute.
    pub faint: bool,
    /// Strikethrough attribute.
    pub strikethrough: bool,
}

/// Cursor projection for the current viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorProjection {
    /// Whether the cursor is mode-visible.
    pub visible: bool,
    /// Whether the cursor lies inside the current viewport.
    pub in_viewport: bool,
    /// Viewport column when [`Self::in_viewport`] is true.
    pub x: u16,
    /// Viewport row when [`Self::in_viewport`] is true.
    pub y: u16,
    /// Visual cursor style.
    pub style: CursorStyle,
}

/// Full viewport projection: dimensions, row-major cells, and cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportProjection {
    /// Viewport columns.
    pub cols: u16,
    /// Viewport rows.
    pub rows: u16,
    /// Row-major cells; `len == cols * rows`.
    pub cells: Vec<ProjectedCell>,
    /// Cursor projection for this viewport.
    pub cursor: CursorProjection,
}

/// Ghostty scrollbar geometry (source of truth for scroll UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarState {
    /// Total scrollable area in rows.
    pub total: usize,
    /// Viewport offset from the top of retained history.
    pub offset: usize,
    /// Visible length in rows.
    pub len: usize,
}

/// Client-facing Ghostty projection without PTY ownership or OSC answering.
///
/// # Install contract
///
/// [`Self::install_ghostsnp`] accepts **opaque GHOSTSNP bytes only** — the
/// shape produced by Hub `DaemonEvent::Snapshot.history.decoded_bytes()`.
/// Dimensions come from the decoded terminal, not from Hub metadata. Never
/// pass Scrollback payloads.
///
/// # Non-goals
///
/// - No `write_pty` effect callback (no OSC query answers).
/// - No PTY ownership.
/// - No Hub DTO types.
pub struct GhosttyClientProjection {
    handle: NonNull<c_void>,
    size: TerminalScreenSize,
    config: GhosttyAdapterConfig,
    render_state: NonNull<c_void>,
    row_iter: NonNull<c_void>,
    row_cells: NonNull<c_void>,
}

impl fmt::Debug for GhosttyClientProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GhosttyClientProjection")
            .field("size", &self.size)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl GhosttyClientProjection {
    /// Create a blank client projection terminal at the given size.
    pub fn new(size: TerminalScreenSize) -> Result<Self, GhosttyTerminalError> {
        Self::with_config(size, GhosttyAdapterConfig::default())
    }

    /// Create a client projection with explicit scrollback configuration.
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

        let mut render_state: GhosttyRenderState = ptr::null_mut();
        let result = unsafe { ghostty_render_state_new(ptr::null(), &mut render_state) };
        if result != GHOSTTY_SUCCESS {
            unsafe { ghostty_terminal_free(handle.as_ptr()) };
            return Err(GhosttyTerminalError::operation("render_state_new", result));
        }
        let Some(render_state) = NonNull::new(render_state) else {
            unsafe { ghostty_terminal_free(handle.as_ptr()) };
            return Err(GhosttyTerminalError::NullHandle {
                operation: "render_state_new",
            });
        };

        let mut row_iter: GhosttyRenderStateRowIterator = ptr::null_mut();
        let result = unsafe { ghostty_render_state_row_iterator_new(ptr::null(), &mut row_iter) };
        if result != GHOSTTY_SUCCESS {
            unsafe {
                ghostty_render_state_free(render_state.as_ptr());
                ghostty_terminal_free(handle.as_ptr());
            }
            return Err(GhosttyTerminalError::operation("row_iterator_new", result));
        }
        let Some(row_iter) = NonNull::new(row_iter) else {
            unsafe {
                ghostty_render_state_free(render_state.as_ptr());
                ghostty_terminal_free(handle.as_ptr());
            }
            return Err(GhosttyTerminalError::NullHandle {
                operation: "row_iterator_new",
            });
        };

        let mut row_cells: GhosttyRenderStateRowCells = ptr::null_mut();
        let result = unsafe { ghostty_render_state_row_cells_new(ptr::null(), &mut row_cells) };
        if result != GHOSTTY_SUCCESS {
            unsafe {
                ghostty_render_state_row_iterator_free(row_iter.as_ptr());
                ghostty_render_state_free(render_state.as_ptr());
                ghostty_terminal_free(handle.as_ptr());
            }
            return Err(GhosttyTerminalError::operation("row_cells_new", result));
        }
        let Some(row_cells) = NonNull::new(row_cells) else {
            unsafe {
                ghostty_render_state_row_iterator_free(row_iter.as_ptr());
                ghostty_render_state_free(render_state.as_ptr());
                ghostty_terminal_free(handle.as_ptr());
            }
            return Err(GhosttyTerminalError::NullHandle {
                operation: "row_cells_new",
            });
        };

        let owned = Self {
            handle,
            size,
            config,
            render_state,
            row_iter,
            row_cells,
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
        // Intentionally omit write_pty: clients must not answer OSC queries.
        Ok(owned)
    }

    /// Current dimensions known after construction, install, or resize.
    #[must_use]
    pub const fn dimensions(&self) -> TerminalScreenSize {
        self.size
    }

    /// Install opaque GHOSTSNP snapshot bytes (Hub Snapshot history shape).
    ///
    /// Fail-closed: empty, non-`GHOSTSNP` magic, corrupt body, or decode
    /// failure leaves the previous handle intact.
    pub fn install_ghostsnp(&mut self, bytes: &[u8]) -> Result<(), GhosttyTerminalError> {
        if bytes.is_empty() {
            return Err(GhosttyTerminalError::operation(
                "install_ghostsnp_empty",
                GHOSTTY_INVALID_VALUE,
            ));
        }
        if !bytes.starts_with(GHOSTSNP_MAGIC) {
            return Err(GhosttyTerminalError::operation(
                "install_ghostsnp_magic",
                GHOSTTY_INVALID_VALUE,
            ));
        }

        let mut decoder: GhosttySnapshotDecoder = ptr::null_mut();
        let result = unsafe {
            ghostty_snapshot_decoder_new_buf(ptr::null(), &mut decoder, bytes.as_ptr(), bytes.len())
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

        self.enable_continuation_tracking()?;
        // Scrollback after decode:
        // - Snapshot decode restores the producer max_scrollback_bytes from the
        //   GHOSTSNP header. That policy governs retained history and later
        //   growth until the client overrides it.
        // - config.max_scrollback == 0 means "no client-side override" (default
        //   new()). Do not call SCROLLBACK_MAX_BYTES with 0: Ghostty treats 0 as
        //   "erase retained history", which would destroy the imported buffer.
        // - config.max_scrollback > 0 means "override decoded policy now". A
        //   tighter override can prune history immediately.
        let max_scrollback = self.config.max_scrollback();
        if max_scrollback > 0 {
            let result = unsafe {
                ghostty_terminal_set(
                    self.handle.as_ptr(),
                    GHOSTTY_TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
                    (&raw const max_scrollback).cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("set_scrollback", result));
            }
        }

        self.size = self.query_dimensions()?;
        Ok(())
    }

    /// Apply live TerminalOutput-shaped bytes into the installed state.
    pub fn apply_terminal_output(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        unsafe {
            ghostty_terminal_vt_write(self.handle.as_ptr(), bytes.as_ptr(), bytes.len());
        }
    }

    /// Resize the client viewport (policy-owned size, not install authenticity).
    pub fn resize(&mut self, size: TerminalScreenSize) -> Result<(), GhosttyTerminalError> {
        let result =
            unsafe { ghostty_terminal_resize(self.handle.as_ptr(), size.cols, size.rows, 0, 0) };
        if result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("resize", result));
        }
        self.size = size;
        Ok(())
    }

    /// Query Ghostty scrollbar state at read time.
    pub fn scrollbar(&self) -> Result<ScrollbarState, GhosttyTerminalError> {
        let mut bar = GhosttyTerminalScrollbar {
            total: 0,
            offset: 0,
            len: 0,
        };
        let result = unsafe {
            ghostty_terminal_get(
                self.handle.as_ptr(),
                GHOSTTY_TERMINAL_DATA_SCROLLBAR,
                (&raw mut bar).cast(),
            )
        };
        if result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("scrollbar", result));
        }
        Ok(ScrollbarState {
            total: bar.total as usize,
            offset: bar.offset as usize,
            len: bar.len as usize,
        })
    }

    /// Scroll the Ghostty-owned viewport.
    pub fn scroll(&mut self, op: ScrollOp) {
        let behavior = match op {
            ScrollOp::Top => GhosttyTerminalScrollViewport {
                tag: GhosttyTerminalScrollViewportTag::Top,
                value: GhosttyTerminalScrollViewportValue { _padding: [0; 2] },
            },
            ScrollOp::Bottom => GhosttyTerminalScrollViewport {
                tag: GhosttyTerminalScrollViewportTag::Bottom,
                value: GhosttyTerminalScrollViewportValue { _padding: [0; 2] },
            },
            ScrollOp::Delta(delta) => GhosttyTerminalScrollViewport {
                tag: GhosttyTerminalScrollViewportTag::Delta,
                value: GhosttyTerminalScrollViewportValue {
                    delta: delta as isize,
                },
            },
        };
        unsafe {
            ghostty_terminal_scroll_viewport(self.handle.as_ptr(), behavior);
        }
    }

    /// Project the current viewport into owned cells, styles, and cursor.
    pub fn project_viewport(&mut self) -> Result<ViewportProjection, GhosttyTerminalError> {
        let result = unsafe {
            ghostty_render_state_update(self.render_state.as_ptr(), self.handle.as_ptr())
        };
        if result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation(
                "render_state_update",
                result,
            ));
        }

        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        self.render_get(GHOSTTY_RENDER_STATE_DATA_COLS, &mut cols)?;
        self.render_get(GHOSTTY_RENDER_STATE_DATA_ROWS, &mut rows)?;
        self.size = TerminalScreenSize::new(rows, cols);

        let mut default_fg = GhosttyColorRgb {
            r: 0xc0,
            g: 0xc0,
            b: 0xc0,
        };
        let mut default_bg = GhosttyColorRgb { r: 0, g: 0, b: 0 };
        let fg_result = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_COLOR_FOREGROUND,
                (&raw mut default_fg).cast(),
            )
        };
        if fg_result != GHOSTTY_SUCCESS && fg_result != GHOSTTY_NO_VALUE {
            return Err(GhosttyTerminalError::operation(
                "render_fg_default",
                fg_result,
            ));
        }
        let bg_result = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_COLOR_BACKGROUND,
                (&raw mut default_bg).cast(),
            )
        };
        if bg_result != GHOSTTY_SUCCESS && bg_result != GHOSTTY_NO_VALUE {
            return Err(GhosttyTerminalError::operation(
                "render_bg_default",
                bg_result,
            ));
        }

        let cursor = self.project_cursor()?;

        let mut row_iter = self.row_iter.as_ptr();
        let result = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_ROW_ITERATOR,
                (&raw mut row_iter).cast(),
            )
        };
        if result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("row_iterator", result));
        }

        let capacity = (cols as usize).saturating_mul(rows as usize);
        let mut cells = Vec::with_capacity(capacity);

        while unsafe { ghostty_render_state_row_iterator_next(self.row_iter.as_ptr()) } {
            let mut cells_handle = self.row_cells.as_ptr();
            let result = unsafe {
                ghostty_render_state_row_get(
                    self.row_iter.as_ptr(),
                    GHOSTTY_RENDER_STATE_ROW_DATA_CELLS,
                    (&raw mut cells_handle).cast(),
                )
            };
            if result != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("row_cells", result));
            }

            while unsafe { ghostty_render_state_row_cells_next(self.row_cells.as_ptr()) } {
                cells.push(self.project_current_cell(default_fg, default_bg)?);
            }
        }

        if cells.len() != capacity {
            return Err(GhosttyTerminalError::operation(
                "project_cell_count",
                GHOSTTY_INVALID_VALUE,
            ));
        }

        Ok(ViewportProjection {
            cols,
            rows,
            cells,
            cursor,
        })
    }

    /// Read Ghostty palette + special colors after live/OSC mutations.
    pub fn color_profile(&self) -> Result<TerminalColorProfile, GhosttyTerminalError> {
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

    /// Read mode flags available on the runtime.
    pub fn mode_flags(&self) -> Result<ModeFlags, GhosttyTerminalError> {
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
            cursor_visible = self.mode_is_set(GHOSTTY_MODE_CURSOR_VISIBLE)?;
        }

        let alt_screen = self.mode_is_set(GHOSTTY_MODE_ALT_SCREEN)?
            || self.mode_is_set(GHOSTTY_MODE_ALT_SCREEN_SAVE)?;

        let mut mouse_mode = 0u8;
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

        Ok(ModeFlags {
            kitty_enabled: kitty_flags != 0,
            cursor_visible,
            bracketed_paste: self.mode_is_set(GHOSTTY_MODE_BRACKETED_PASTE)?,
            mouse_mode,
            alt_screen,
            focus_reporting: self.mode_is_set(GHOSTTY_MODE_FOCUS_EVENT)?,
            application_cursor: self.mode_is_set(GHOSTTY_MODE_DECCKM)?,
        })
    }

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

    fn query_dimensions(&self) -> Result<TerminalScreenSize, GhosttyTerminalError> {
        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        let cols_result = unsafe {
            ghostty_terminal_get(
                self.handle.as_ptr(),
                GHOSTTY_TERMINAL_DATA_COLS,
                (&raw mut cols).cast(),
            )
        };
        if cols_result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("cols", cols_result));
        }
        let rows_result = unsafe {
            ghostty_terminal_get(
                self.handle.as_ptr(),
                GHOSTTY_TERMINAL_DATA_ROWS,
                (&raw mut rows).cast(),
            )
        };
        if rows_result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("rows", rows_result));
        }
        Ok(TerminalScreenSize::new(rows, cols))
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

    fn render_get<T>(&self, data: c_int, out: &mut T) -> Result<(), GhosttyTerminalError> {
        let result = unsafe {
            ghostty_render_state_get(self.render_state.as_ptr(), data, (out as *mut T).cast())
        };
        if result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("render_get", result));
        }
        Ok(())
    }

    fn project_cursor(&self) -> Result<CursorProjection, GhosttyTerminalError> {
        let mut visible = true;
        let mut in_viewport = false;
        let mut x: u16 = 0;
        let mut y: u16 = 0;
        let mut style = GhosttyRenderStateCursorVisualStyle::Block;

        let _ = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_CURSOR_VISIBLE,
                (&raw mut visible).cast(),
            )
        };
        let _ = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_HAS_VALUE,
                (&raw mut in_viewport).cast(),
            )
        };
        if in_viewport {
            let _ = unsafe {
                ghostty_render_state_get(
                    self.render_state.as_ptr(),
                    GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_X,
                    (&raw mut x).cast(),
                )
            };
            let _ = unsafe {
                ghostty_render_state_get(
                    self.render_state.as_ptr(),
                    GHOSTTY_RENDER_STATE_DATA_CURSOR_VIEWPORT_Y,
                    (&raw mut y).cast(),
                )
            };
        }
        let _ = unsafe {
            ghostty_render_state_get(
                self.render_state.as_ptr(),
                GHOSTTY_RENDER_STATE_DATA_CURSOR_VISUAL_STYLE,
                (&raw mut style).cast(),
            )
        };

        Ok(CursorProjection {
            visible,
            in_viewport,
            x,
            y,
            style: match style {
                GhosttyRenderStateCursorVisualStyle::Bar => CursorStyle::Bar,
                GhosttyRenderStateCursorVisualStyle::Block => CursorStyle::Block,
                GhosttyRenderStateCursorVisualStyle::Underline => CursorStyle::Underline,
                GhosttyRenderStateCursorVisualStyle::BlockHollow => CursorStyle::Hollow,
            },
        })
    }

    fn project_current_cell(
        &self,
        default_fg: GhosttyColorRgb,
        default_bg: GhosttyColorRgb,
    ) -> Result<ProjectedCell, GhosttyTerminalError> {
        let mut style = GhosttyStyle {
            size: std::mem::size_of::<GhosttyStyle>(),
            fg_color: GhosttyStyleColor {
                tag: GhosttyStyleColorTag::None,
                value: GhosttyStyleColorValue { _padding: 0 },
            },
            bg_color: GhosttyStyleColor {
                tag: GhosttyStyleColorTag::None,
                value: GhosttyStyleColorValue { _padding: 0 },
            },
            underline_color: GhosttyStyleColor {
                tag: GhosttyStyleColorTag::None,
                value: GhosttyStyleColorValue { _padding: 0 },
            },
            bold: false,
            italic: false,
            faint: false,
            blink: false,
            inverse: false,
            invisible: false,
            strikethrough: false,
            overline: false,
            underline: 0,
        };
        let style_result = unsafe {
            ghostty_render_state_row_cells_get(
                self.row_cells.as_ptr(),
                GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_STYLE,
                (&raw mut style).cast(),
            )
        };
        if style_result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("cell_style", style_result));
        }

        let mut fg = default_fg;
        let fg_result = unsafe {
            ghostty_render_state_row_cells_get(
                self.row_cells.as_ptr(),
                GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_FG_COLOR,
                (&raw mut fg).cast(),
            )
        };
        if fg_result != GHOSTTY_SUCCESS && fg_result != GHOSTTY_INVALID_VALUE {
            return Err(GhosttyTerminalError::operation("cell_fg", fg_result));
        }
        if fg_result == GHOSTTY_INVALID_VALUE {
            fg = default_fg;
        }

        let mut bg = default_bg;
        let bg_result = unsafe {
            ghostty_render_state_row_cells_get(
                self.row_cells.as_ptr(),
                GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_BG_COLOR,
                (&raw mut bg).cast(),
            )
        };
        if bg_result != GHOSTTY_SUCCESS && bg_result != GHOSTTY_INVALID_VALUE {
            return Err(GhosttyTerminalError::operation("cell_bg", bg_result));
        }
        if bg_result == GHOSTTY_INVALID_VALUE {
            bg = default_bg;
        }

        let mut cell: GhosttyCell = 0;
        let raw_result = unsafe {
            ghostty_render_state_row_cells_get(
                self.row_cells.as_ptr(),
                GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_RAW,
                (&raw mut cell).cast(),
            )
        };
        if raw_result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("cell_raw", raw_result));
        }
        let mut wide = GhosttyCellWide::Narrow;
        let wide_result =
            unsafe { ghostty_cell_get(cell, GHOSTTY_CELL_DATA_WIDE, (&raw mut wide).cast()) };
        if wide_result != GHOSTTY_SUCCESS {
            return Err(GhosttyTerminalError::operation("cell_wide", wide_result));
        }

        let grapheme = self.read_grapheme_utf8()?;

        Ok(ProjectedCell {
            grapheme,
            wide: match wide {
                GhosttyCellWide::Narrow => ProjectedWide::Narrow,
                GhosttyCellWide::Wide => ProjectedWide::Wide,
                GhosttyCellWide::SpacerTail => ProjectedWide::SpacerTail,
                GhosttyCellWide::SpacerHead => ProjectedWide::SpacerHead,
            },
            fg: Rgb {
                r: fg.r,
                g: fg.g,
                b: fg.b,
            },
            bg: Rgb {
                r: bg.r,
                g: bg.g,
                b: bg.b,
            },
            bold: style.bold,
            italic: style.italic,
            underline: style.underline != 0,
            inverse: style.inverse,
            faint: style.faint,
            strikethrough: style.strikethrough,
        })
    }

    fn read_grapheme_utf8(&self) -> Result<String, GhosttyTerminalError> {
        let mut scratch = [0u8; 64];
        let mut buf = GhosttyBuffer {
            ptr: scratch.as_mut_ptr(),
            cap: scratch.len(),
            len: 0,
        };
        let result = unsafe {
            ghostty_render_state_row_cells_get(
                self.row_cells.as_ptr(),
                GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8,
                (&raw mut buf).cast(),
            )
        };
        if result == GHOSTTY_SUCCESS {
            return Ok(String::from_utf8_lossy(&scratch[..buf.len]).into_owned());
        }
        if result == GHOSTTY_OUT_OF_SPACE {
            let needed = buf.len.max(1);
            let mut owned = vec![0u8; needed];
            let mut big = GhosttyBuffer {
                ptr: owned.as_mut_ptr(),
                cap: owned.len(),
                len: 0,
            };
            let retry = unsafe {
                ghostty_render_state_row_cells_get(
                    self.row_cells.as_ptr(),
                    GHOSTTY_RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8,
                    (&raw mut big).cast(),
                )
            };
            if retry != GHOSTTY_SUCCESS {
                return Err(GhosttyTerminalError::operation("grapheme_utf8", retry));
            }
            return Ok(String::from_utf8_lossy(&owned[..big.len]).into_owned());
        }
        Err(GhosttyTerminalError::operation("grapheme_utf8", result))
    }

    fn read_optional_color(
        &self,
        data: c_int,
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

impl Drop for GhosttyClientProjection {
    fn drop(&mut self) {
        unsafe {
            ghostty_render_state_row_cells_free(self.row_cells.as_ptr());
            ghostty_render_state_row_iterator_free(self.row_iter.as_ptr());
            ghostty_render_state_free(self.render_state.as_ptr());
            ghostty_terminal_free(self.handle.as_ptr());
        }
    }
}

struct DecoderGuard(GhosttySnapshotDecoder);

impl Drop for DecoderGuard {
    fn drop(&mut self) {
        unsafe { ghostty_snapshot_decoder_free(self.0) };
    }
}
