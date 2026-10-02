//! The bytes of a reply to a query. The host gives a typed reply (`QueryReply`); libghostty writes the bytes, so this
//! crate never writes terminal protocol bytes itself (EV-8). The reply must answer the kind of the query it is for.

use botster_route_codec::prelude::{ColorScheme, QueryReply};

use crate::query::{Query, QueryKind, Terminator};
use crate::sys;

/// Why a reply gave no bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyError {
    /// The reply does not answer this kind of query.
    Mismatch,
    /// A field is outside what the wire allows: a control character in a title, a selection that the request did not
    /// name, a position above 65535, or text that is not base64.
    Invalid,
}

/// The largest reply that the binding accepts, in bytes of the encoded form.
pub const MAX_REPLY_BYTES: usize = 1 << 20;

fn string(bytes: &[u8]) -> sys::GString {
    sys::GString { ptr: bytes.as_ptr(), len: bytes.len() }
}

fn blank(kind: i32) -> sys::QueryReply {
    let empty: &[u8] = &[];
    sys::QueryReply {
        size: std::mem::size_of::<sys::QueryReply>(),
        kind,
        width: 0,
        height: 0,
        rows: 0,
        cols: 0,
        x: 0,
        y: 0,
        iconified: false,
        text: string(empty),
        selection: string(empty),
        terminator: 0,
    }
}

/// Run one of the library's size-then-fill encoders.
fn fill(call: impl Fn(*mut u8, usize, *mut usize) -> sys::Result) -> Result<Vec<u8>, ReplyError> {
    let mut written = 0usize;
    match call(std::ptr::null_mut(), 0, &mut written) {
        sys::SUCCESS => return Ok(Vec::new()),
        sys::OUT_OF_SPACE => {}
        _ => return Err(ReplyError::Invalid),
    }
    if written > MAX_REPLY_BYTES {
        return Err(ReplyError::Invalid);
    }
    let mut out = vec![0u8; written];
    let mut done = 0usize;
    if call(out.as_mut_ptr(), out.len(), &mut done) != sys::SUCCESS {
        return Err(ReplyError::Invalid);
    }
    out.truncate(done);
    Ok(out)
}

fn run(reply: &sys::QueryReply) -> Result<Vec<u8>, ReplyError> {
    // SAFETY: `reply` and the strings in it are valid for the call, and the library does not keep them.
    fill(|buf, len, out| unsafe { sys::ghostty_query_reply_encode(reply, buf, len, out) })
}

fn position(value: u32) -> Result<u16, ReplyError> {
    u16::try_from(value).map_err(|_| ReplyError::Invalid)
}

/// Decode standard base64 with padding. Anything else is `Invalid`.
pub(crate) fn base64_decode(text: &str) -> Result<Vec<u8>, ReplyError> {
    let bytes = text.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(ReplyError::Invalid);
    }
    let value = |b: u8| -> Result<u32, ReplyError> {
        match b {
            b'A'..=b'Z' => Ok(u32::from(b - b'A')),
            b'a'..=b'z' => Ok(u32::from(b - b'a') + 26),
            b'0'..=b'9' => Ok(u32::from(b - b'0') + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(ReplyError::Invalid),
        }
    };
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let groups = bytes.len() / 4;
    for (index, group) in bytes.chunks(4).enumerate() {
        let last = index + 1 == groups;
        let pad = group.iter().rev().take_while(|b| **b == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return Err(ReplyError::Invalid);
        }
        let mut n = 0u32;
        for (i, &b) in group.iter().enumerate() {
            n <<= 6;
            if i < 4 - pad {
                n |= value(b)?;
            }
        }
        let triple = n.to_be_bytes();
        out.extend_from_slice(&triple[1..4 - pad]);
        // The unused low bits of a padded group must be zero, so that one byte string has one encoding.
        if pad > 0 && (n >> (pad * 6)) & ((1 << (pad * 2)) - 1) != 0 {
            return Err(ReplyError::Invalid);
        }
    }
    Ok(out)
}

impl Query {
    /// The bytes to write to the program for `reply`. A `Decline` gives no bytes. An `Encoded` reply is the host's own
    /// bytes and goes through unchanged, for the queries that have no typed reply.
    pub fn reply_bytes(&self, reply: &QueryReply) -> Result<Vec<u8>, ReplyError> {
        use sys::reply_kind as kind;
        match reply {
            QueryReply::Decline => Ok(Vec::new()),
            QueryReply::Encoded { bytes_base64 } => base64_decode(bytes_base64),
            QueryReply::Pixels { width, height } => {
                let which = match self.kind {
                    QueryKind::Size14T | QueryKind::Size14Of2T => kind::PIXELS_TEXT_AREA,
                    QueryKind::Size16T => kind::PIXELS_CELL,
                    QueryKind::Size15T => kind::PIXELS_SCREEN,
                    _ => return Err(ReplyError::Mismatch),
                };
                let mut r = blank(which);
                r.width = *width;
                r.height = *height;
                run(&r)
            }
            QueryReply::Chars { rows, cols } => {
                if self.kind != QueryKind::Size19T {
                    return Err(ReplyError::Mismatch);
                }
                let mut r = blank(kind::CHARS_SCREEN);
                r.rows = *rows;
                r.cols = *cols;
                run(&r)
            }
            QueryReply::WindowState { iconified } => {
                if self.kind != QueryKind::Size11T {
                    return Err(ReplyError::Mismatch);
                }
                let mut r = blank(kind::WINDOW_STATE);
                r.iconified = *iconified;
                run(&r)
            }
            QueryReply::Position { x, y } => {
                if !matches!(self.kind, QueryKind::Size13T | QueryKind::Size13Of2T) {
                    return Err(ReplyError::Mismatch);
                }
                let mut r = blank(kind::WINDOW_POSITION);
                r.x = position(*x)?;
                r.y = position(*y)?;
                run(&r)
            }
            QueryReply::Text { text } => {
                let which = match self.kind {
                    QueryKind::Size21T => kind::WINDOW_TITLE,
                    QueryKind::Size20T => kind::ICON_LABEL,
                    _ => return Err(ReplyError::Mismatch),
                };
                let mut r = blank(which);
                r.text = string(text.as_bytes());
                run(&r)
            }
            QueryReply::Clipboard { selection, data_base64 } => {
                if self.kind != QueryKind::ClipboardRead {
                    return Err(ReplyError::Mismatch);
                }
                // The reply names the selection of the request, and uses the terminator of the request.
                let asked = self.selection.as_deref().unwrap_or("s0");
                if selection != asked {
                    return Err(ReplyError::Mismatch);
                }
                let data = base64_decode(data_base64)?;
                let mut r = blank(kind::CLIPBOARD);
                r.text = string(&data);
                r.selection = string(selection.as_bytes());
                r.terminator = match self.terminator.unwrap_or(Terminator::St) {
                    Terminator::St => 0,
                    Terminator::Bel => 1,
                };
                run(&r)
            }
            QueryReply::ColorScheme(scheme) => {
                if self.kind != QueryKind::ColorScheme {
                    return Err(ReplyError::Mismatch);
                }
                let scheme = match scheme {
                    ColorScheme::Dark => sys::COLOR_SCHEME_DARK,
                    ColorScheme::Light => sys::COLOR_SCHEME_LIGHT,
                };
                // SAFETY: only the buffer arguments are pointers, and `fill` gives valid ones.
                fill(|buf, len, out| unsafe { sys::ghostty_color_scheme_report_encode(scheme, buf, len, out) })
            }
        }
    }
}
