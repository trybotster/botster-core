//! The reads of the model (ST-2, ST-3): the screen text, the cursor and the cells of a row. Every value comes from
//! libghostty; this module only copies it.

use crate::sys;

/// The text of the screen (ST-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenText {
    /// The plain text. With history, it holds the scrollback and then the visible screen. Without it, it holds the
    /// visible screen.
    pub text: String,
    /// True when history was asked for and the model keeps none (`History::Off`). The text is then the visible screen,
    /// and the caller says so; it is never an empty text that stands for the screen.
    pub history_unavailable: bool,
}

/// The cursor (ST-3), in zero-based cells of the visible screen. A wide character takes two cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorCell {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
}

fn get<T: Default>(terminal: sys::Terminal, key: i32) -> T {
    let mut out = T::default();
    // SAFETY: the terminal is live, and each caller passes a key whose output type is `T`.
    let code = unsafe { sys::ghostty_terminal_get(terminal, key, (&mut out as *mut T).cast()) };
    debug_assert_eq!(code, sys::SUCCESS);
    out
}

pub(crate) fn cursor(terminal: sys::Terminal) -> CursorCell {
    CursorCell {
        row: u32::from(get::<u16>(terminal, sys::data::CURSOR_Y)),
        col: u32::from(get::<u16>(terminal, sys::data::CURSOR_X)),
        visible: get::<bool>(terminal, sys::data::CURSOR_VISIBLE),
    }
}

fn grid_ref(terminal: sys::Terminal, tag: i32, x: u16, y: u32) -> Option<sys::GridRef> {
    let mut grid_ref = sys::GridRef::empty();
    // SAFETY: the terminal is live and `grid_ref` is a valid out pointer with its size set.
    let code = unsafe {
        sys::ghostty_terminal_grid_ref(terminal, sys::Point::new(tag, x, y), &mut grid_ref)
    };
    (code == sys::SUCCESS).then_some(grid_ref)
}

/// The text of one cell: its graphemes, a space for an empty cell, nothing for the spacer of a wide character.
pub(crate) fn cell_text(terminal: sys::Terminal, x: u16, row: u16) -> Option<String> {
    let reference = grid_ref(terminal, sys::point_tag::ACTIVE, x, u32::from(row))?;

    let mut cell: sys::Cell = 0;
    // SAFETY: `reference` was just made by the library and `cell` is a valid out pointer.
    if unsafe { sys::ghostty_grid_ref_cell(&reference, &mut cell) } != sys::SUCCESS {
        return None;
    }

    let mut wide: i32 = 0;
    let mut has_text = false;
    // SAFETY: `cell` is a value that the library returned, and each key writes the type that is passed.
    unsafe {
        if sys::ghostty_cell_get(cell, sys::cell_data::WIDE, (&mut wide as *mut i32).cast())
            != sys::SUCCESS
            || sys::ghostty_cell_get(
                cell,
                sys::cell_data::HAS_TEXT,
                (&mut has_text as *mut bool).cast(),
            ) != sys::SUCCESS
        {
            return None;
        }
    }
    if wide == sys::cell_wide::SPACER_TAIL || wide == sys::cell_wide::SPACER_HEAD {
        return Some(String::new());
    }
    if !has_text {
        return Some(" ".to_owned());
    }

    // A grapheme cluster has a handful of codepoints. Ask for the length first.
    let mut len: usize = 0;
    // SAFETY: a null buffer with length 0 asks for the length.
    let code =
        unsafe { sys::ghostty_grid_ref_graphemes(&reference, std::ptr::null_mut(), 0, &mut len) };
    // The cell has text, so the length is at least one and the probe says OUT_OF_SPACE.
    if code != sys::OUT_OF_SPACE {
        return None;
    }
    let mut codepoints = vec![0u32; len];
    // SAFETY: the buffer holds `len` codepoints.
    if unsafe {
        sys::ghostty_grid_ref_graphemes(&reference, codepoints.as_mut_ptr(), len, &mut len)
    } != sys::SUCCESS
    {
        return None;
    }
    Some(codepoints.into_iter().filter_map(char::from_u32).collect())
}

/// The plain text of the screen. `history` asks for the scrollback in front of the visible screen.
pub(crate) fn screen_text(
    terminal: sys::Terminal,
    history: bool,
    cols: u16,
    rows: u16,
) -> Option<String> {
    let (tag, last_row) = if history {
        (
            sys::point_tag::SCREEN,
            get::<usize>(terminal, sys::data::TOTAL_ROWS).saturating_sub(1) as u32,
        )
    } else {
        (sys::point_tag::ACTIVE, u32::from(rows).saturating_sub(1))
    };
    let start = grid_ref(terminal, tag, 0, 0)?;
    let end = grid_ref(terminal, tag, cols.saturating_sub(1), last_row)?;
    let selection = sys::Selection {
        size: std::mem::size_of::<sys::Selection>(),
        start,
        end,
        rectangle: false,
    };

    let options = sys::FormatterTerminalOptions {
        size: std::mem::size_of::<sys::FormatterTerminalOptions>(),
        emit: sys::FORMATTER_FORMAT_PLAIN,
        unwrap: false,
        trim: true,
        extra: sys::FormatterTerminalExtra {
            size: std::mem::size_of::<sys::FormatterTerminalExtra>(),
            palette: false,
            modes: false,
            scrolling_region: false,
            tabstops: false,
            pwd: false,
            keyboard: false,
            screen: sys::FormatterScreenExtra {
                size: std::mem::size_of::<sys::FormatterScreenExtra>(),
                cursor: false,
                style: false,
                hyperlink: false,
                protection: false,
                kitty_keyboard: false,
                charsets: false,
            },
        },
        selection: &selection,
    };

    let mut formatter: sys::Formatter = std::ptr::null_mut();
    // SAFETY: the terminal is live; `options` and the selection it points to are valid for the call, and the library
    // copies them. A successful call stores a formatter that is freed below.
    if unsafe {
        sys::ghostty_formatter_terminal_new(std::ptr::null(), &mut formatter, terminal, options)
    } != sys::SUCCESS
    {
        return None;
    }

    let mut needed: usize = 0;
    // SAFETY: a null buffer asks for the size, which the library stores in `needed`.
    let probe = unsafe {
        sys::ghostty_formatter_format_buf(formatter, std::ptr::null_mut(), 0, &mut needed)
    };
    let text = if probe == sys::SUCCESS || probe == sys::OUT_OF_SPACE {
        let mut buffer = vec![0u8; needed];
        let mut written: usize = 0;
        // SAFETY: the buffer holds `needed` bytes.
        let code = unsafe {
            sys::ghostty_formatter_format_buf(formatter, buffer.as_mut_ptr(), needed, &mut written)
        };
        (code == sys::SUCCESS).then(|| {
            buffer.truncate(written);
            String::from_utf8_lossy(&buffer).into_owned()
        })
    } else {
        None
    };
    // SAFETY: the formatter is live and is freed exactly once.
    unsafe { sys::ghostty_formatter_free(formatter) };
    text
}
