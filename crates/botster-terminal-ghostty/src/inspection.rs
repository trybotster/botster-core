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
    /// Read the active screen's native image storage limit (Core ST-6b graphics).
    pub fn image_storage_limit(&self) -> Result<u64, Error> {
        let mut limit = 0u64;
        // SAFETY: the data key writes a uint64_t into this live output pointer.
        crate::check(unsafe {
            sys::ghostty_terminal_get(
                self.handle.as_ptr(),
                sys::data::KITTY_IMAGE_STORAGE_LIMIT,
                (&mut limit as *mut u64).cast(),
            )
        })?;
        Ok(limit)
    }

    /// Ask libghostty whether the active screen stores the given image (Core ST-6b graphics).
    pub fn has_image(&self, id: u32) -> Result<bool, Error> {
        let mut graphics = std::ptr::null_mut();
        // SAFETY: the data key writes a borrowed graphics handle, used before any terminal mutation.
        unsafe {
            crate::check(sys::ghostty_terminal_get(
                self.handle.as_ptr(),
                sys::data::KITTY_GRAPHICS,
                (&mut graphics as *mut sys::KittyGraphics).cast(),
            ))?;
            Ok(!sys::ghostty_kitty_graphics_image(graphics, id).is_null())
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::History;
    use botster_core_contract::prelude::Size;

    #[test]
    fn native_cell_fields_and_dynamic_colors_match_the_public_reads() {
        let mut terminal = Terminal::new(
            &Size {
                rows: 3,
                cols: 10,
                cell_px: None,
            },
            History::On,
        )
        .unwrap();
        for input in [
            &b"\x1b[1\"q\x1b[1;44mX"[..],
            &b"\x1b[0m\x1b[48;5;123m\x1b[2K"[..],
            &b"\x1b[48;2;11;22;33m\x1b[2K"[..],
            &b"\x1b]10;rgb:34/56/78\x1b\\\x1b]11;rgb:45/67/89\x1b\\\x1b]12;rgb:56/78/9a\x1b\\"[..],
        ] {
            terminal.vt_write(input);
            let reference =
                reads::grid_ref(terminal.handle.as_ptr(), sys::point_tag::ACTIVE, 0, 0).unwrap();
            let mut cell = 0;
            // SAFETY: each field uses the output type from the pinned C header.
            unsafe {
                crate::check(sys::ghostty_grid_ref_cell(&reference, &mut cell)).unwrap();
                let attrs = terminal.cell_attributes(0, 0, false).unwrap();
                let mut wide = 0i32;
                let mut text = false;
                let mut protected = false;
                let mut semantic = 0i32;
                crate::check(sys::ghostty_cell_get(
                    cell,
                    sys::cell_data::WIDE,
                    (&mut wide as *mut i32).cast(),
                ))
                .unwrap();
                crate::check(sys::ghostty_cell_get(
                    cell,
                    sys::cell_data::HAS_TEXT,
                    (&mut text as *mut bool).cast(),
                ))
                .unwrap();
                crate::check(sys::ghostty_cell_get(
                    cell,
                    sys::cell_data::PROTECTED,
                    (&mut protected as *mut bool).cast(),
                ))
                .unwrap();
                crate::check(sys::ghostty_cell_get(
                    cell,
                    sys::cell_data::SEMANTIC_CONTENT,
                    (&mut semantic as *mut i32).cast(),
                ))
                .unwrap();
                assert_eq!(
                    (
                        attrs.wide,
                        attrs.has_text,
                        attrs.protected,
                        attrs.semantic_content
                    ),
                    (wide, text, protected, semantic)
                );
                let mut tag = 0i32;
                crate::check(sys::ghostty_cell_get(
                    cell,
                    sys::cell_data::CONTENT_TAG,
                    (&mut tag as *mut i32).cast(),
                ))
                .unwrap();
                match tag {
                    2 => {
                        let mut color = 0u8;
                        crate::check(sys::ghostty_cell_get(
                            cell,
                            sys::cell_data::COLOR_PALETTE,
                            (&mut color as *mut u8).cast(),
                        ))
                        .unwrap();
                        assert_eq!(attrs.cell_background, StyleColor::Palette(color));
                    }
                    3 => {
                        let mut color = sys::ColorRgb { r: 0, g: 0, b: 0 };
                        crate::check(sys::ghostty_cell_get(
                            cell,
                            sys::cell_data::COLOR_RGB,
                            (&mut color as *mut sys::ColorRgb).cast(),
                        ))
                        .unwrap();
                        assert_eq!(
                            attrs.cell_background,
                            StyleColor::Rgb(Rgb {
                                r: color.r,
                                g: color.g,
                                b: color.b
                            })
                        );
                    }
                    _ => assert_eq!(attrs.cell_background, StyleColor::None),
                }
                let colors = terminal.colors().unwrap();
                for (key, actual) in [
                    (sys::data::COLOR_FOREGROUND, colors.foreground),
                    (sys::data::COLOR_BACKGROUND, colors.background),
                    (sys::data::COLOR_CURSOR, colors.cursor),
                    (
                        sys::data::COLOR_FOREGROUND_DEFAULT,
                        colors.default_foreground,
                    ),
                    (
                        sys::data::COLOR_BACKGROUND_DEFAULT,
                        colors.default_background,
                    ),
                    (sys::data::COLOR_CURSOR_DEFAULT, colors.default_cursor),
                ] {
                    let mut color = sys::ColorRgb { r: 0, g: 0, b: 0 };
                    let result = sys::ghostty_terminal_get(
                        terminal.handle.as_ptr(),
                        key,
                        (&mut color as *mut sys::ColorRgb).cast(),
                    );
                    if result == sys::NO_VALUE {
                        assert_eq!(actual, None);
                    } else {
                        crate::check(result).unwrap();
                        assert_eq!(
                            actual,
                            Some(Rgb {
                                r: color.r,
                                g: color.g,
                                b: color.b
                            })
                        );
                    }
                }
            }
        }
    }
}
