//! Public reads of cell attributes, colors and cursor appearance (Core ST-6b).
//! Each read copies data from libghostty. This module parses no terminal bytes.

use crate::{reads, sys, Error, Terminal};
use std::ffi::c_void;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A color as libghostty classifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleColor {
    None,
    Palette(u8),
    Rgb(Rgb),
}

/// The visual attributes of one cell, copied from `GhosttyStyle`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellStyle {
    pub foreground: StyleColor,
    pub background: StyleColor,
    pub underline_color: StyleColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    /// The library's `GhosttySgrUnderline` value.
    pub underline: i32,
}

/// Logical cell data. Storage identifiers and native struct padding are excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellAttributes {
    pub style: CellStyle,
    /// The library's `GhosttyCellWide` value.
    pub wide: i32,
    pub has_text: bool,
    pub protected: bool,
    /// The library's `GhosttyCellSemanticContent` value.
    pub semantic_content: i32,
    /// A background stored directly in the cell, rather than in its style.
    pub cell_background: StyleColor,
}

/// The current and default palette and dynamic colors (Core ST-6b item 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    pub palette: [Rgb; 256],
    pub default_palette: [Rgb; 256],
    pub foreground: Option<Rgb>,
    pub background: Option<Rgb>,
    pub cursor: Option<Rgb>,
    pub default_foreground: Option<Rgb>,
    pub default_background: Option<Rgb>,
    pub default_cursor: Option<Rgb>,
}

/// The library's cursor appearance (Core ST-6b item 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorAppearance {
    /// The library's `GhosttyRenderStateCursorVisualStyle` value.
    pub shape: i32,
    pub blinking: bool,
}

fn rgb(value: sys::ColorRgb) -> Rgb {
    Rgb {
        r: value.r,
        g: value.g,
        b: value.b,
    }
}

fn style_color(value: sys::StyleColor) -> Result<StyleColor, Error> {
    // SAFETY: libghostty initializes the union member selected by the tag.
    match value.tag {
        0 => Ok(StyleColor::None),
        1 => Ok(StyleColor::Palette(unsafe { value.value.palette })),
        2 => Ok(StyleColor::Rgb(rgb(unsafe { value.value.rgb }))),
        _ => Err(Error::InvalidValue),
    }
}

/// Read a native cell field. Each caller supplies the type specified by the C header.
fn cell_field<T: Default>(cell: sys::Cell, key: i32) -> Result<T, Error> {
    let mut out = T::default();
    // SAFETY: every caller pairs the data key with its output type.
    crate::check(unsafe { sys::ghostty_cell_get(cell, key, (&mut out as *mut T).cast()) })?;
    Ok(out)
}

impl Terminal {
    /// Read the attributes of a cell (Core ST-6b item 1).
    /// With `history`, row zero starts at the oldest scrollback row. Otherwise it starts at the visible screen.
    pub fn cell_attributes(
        &self,
        row: u32,
        col: u32,
        history: bool,
    ) -> Result<CellAttributes, Error> {
        let col = u16::try_from(col).map_err(|_| Error::InvalidValue)?;
        let tag = if history {
            sys::point_tag::SCREEN
        } else {
            sys::point_tag::ACTIVE
        };
        let reference =
            reads::grid_ref(self.handle.as_ptr(), tag, col, row).ok_or(Error::InvalidValue)?;
        let color = sys::StyleColor {
            tag: 0,
            value: sys::StyleColorValue { padding: 0 },
        };
        let mut style = sys::Style {
            size: std::mem::size_of::<sys::Style>(),
            fg_color: color,
            bg_color: color,
            underline_color: color,
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
        let mut cell = 0;
        // SAFETY: the reference is live and both output pointers have the header's types.
        unsafe {
            crate::check(sys::ghostty_grid_ref_cell(&reference, &mut cell))?;
            crate::check(sys::ghostty_grid_ref_style(&reference, &mut style))?;
        }
        let background = match cell_field::<i32>(cell, sys::cell_data::CONTENT_TAG)? {
            2 => StyleColor::Palette(cell_field(cell, sys::cell_data::COLOR_PALETTE)?),
            3 => StyleColor::Rgb(rgb(cell_rgb(cell)?)),
            _ => StyleColor::None,
        };
        Ok(CellAttributes {
            style: CellStyle {
                foreground: style_color(style.fg_color)?,
                background: style_color(style.bg_color)?,
                underline_color: style_color(style.underline_color)?,
                bold: style.bold,
                italic: style.italic,
                faint: style.faint,
                blink: style.blink,
                inverse: style.inverse,
                invisible: style.invisible,
                strikethrough: style.strikethrough,
                overline: style.overline,
                underline: style.underline,
            },
            wide: cell_field(cell, sys::cell_data::WIDE)?,
            has_text: cell_field(cell, sys::cell_data::HAS_TEXT)?,
            protected: cell_field(cell, sys::cell_data::PROTECTED)?,
            semantic_content: cell_field(cell, sys::cell_data::SEMANTIC_CONTENT)?,
            cell_background: background,
        })
    }

    /// Read current and default colors through libghostty (Core ST-6b item 2).
    pub fn colors(&self) -> Result<Colors, Error> {
        Ok(Colors {
            palette: palette(self.handle.as_ptr(), sys::data::COLOR_PALETTE)?,
            default_palette: palette(self.handle.as_ptr(), sys::data::COLOR_PALETTE_DEFAULT)?,
            foreground: optional_color(self.handle.as_ptr(), sys::data::COLOR_FOREGROUND)?,
            background: optional_color(self.handle.as_ptr(), sys::data::COLOR_BACKGROUND)?,
            cursor: optional_color(self.handle.as_ptr(), sys::data::COLOR_CURSOR)?,
            default_foreground: optional_color(
                self.handle.as_ptr(),
                sys::data::COLOR_FOREGROUND_DEFAULT,
            )?,
            default_background: optional_color(
                self.handle.as_ptr(),
                sys::data::COLOR_BACKGROUND_DEFAULT,
            )?,
            default_cursor: optional_color(self.handle.as_ptr(), sys::data::COLOR_CURSOR_DEFAULT)?,
        })
    }

    /// Read cursor shape and blink through the renderer (Core ST-6b item 3).
    /// This method consumes the terminal's render dirty state, as a renderer update does.
    pub fn cursor_appearance(&mut self) -> Result<CursorAppearance, Error> {
        let mut render = std::ptr::null_mut();
        // SAFETY: a successful call stores a live render state that the guard frees.
        crate::check(unsafe { sys::ghostty_render_state_new(std::ptr::null(), &mut render) })?;
        let render = Render(render);
        let (mut shape, mut blinking) = (0i32, false);
        // SAFETY: both handles are live and each data key receives its documented output type.
        unsafe {
            crate::check(sys::ghostty_render_state_update(
                render.0,
                self.handle.as_ptr(),
            ))?;
            crate::check(sys::ghostty_render_state_get(
                render.0,
                10,
                (&mut shape as *mut i32).cast(),
            ))?;
            crate::check(sys::ghostty_render_state_get(
                render.0,
                12,
                (&mut blinking as *mut bool).cast(),
            ))?;
        }
        Ok(CursorAppearance { shape, blinking })
    }
}

fn cell_rgb(cell: sys::Cell) -> Result<sys::ColorRgb, Error> {
    let mut out = sys::ColorRgb { r: 0, g: 0, b: 0 };
    // SAFETY: the key writes a GhosttyColorRgb. The caller checked the cell's content tag.
    crate::check(unsafe {
        sys::ghostty_cell_get(
            cell,
            sys::cell_data::COLOR_RGB,
            (&mut out as *mut sys::ColorRgb).cast(),
        )
    })?;
    Ok(out)
}

fn palette(terminal: sys::Terminal, key: i32) -> Result<[Rgb; 256], Error> {
    let mut out = [sys::ColorRgb { r: 0, g: 0, b: 0 }; 256];
    // SAFETY: both palette keys write exactly 256 GhosttyColorRgb values.
    crate::check(unsafe { sys::ghostty_terminal_get(terminal, key, out.as_mut_ptr().cast()) })?;
    Ok(out.map(rgb))
}

fn optional_color(terminal: sys::Terminal, key: i32) -> Result<Option<Rgb>, Error> {
    let mut out = sys::ColorRgb { r: 0, g: 0, b: 0 };
    // SAFETY: every caller supplies a color key that writes a GhosttyColorRgb.
    let code = unsafe {
        sys::ghostty_terminal_get(
            terminal,
            key,
            (&mut out as *mut sys::ColorRgb).cast::<c_void>(),
        )
    };
    if code == sys::NO_VALUE {
        return Ok(None);
    }
    crate::check(code)?;
    Ok(Some(rgb(out)))
}

struct Render(sys::RenderState);
impl Drop for Render {
    fn drop(&mut self) {
        // SAFETY: this guard owns the live render state and frees it once.
        unsafe { sys::ghostty_render_state_free(self.0) };
    }
}
