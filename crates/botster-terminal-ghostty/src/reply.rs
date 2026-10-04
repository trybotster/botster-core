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
    sys::GString {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    }
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
    // Every reply writes at least its introducer, so a valid reply makes the empty probe say OUT_OF_SPACE.
    if call(std::ptr::null_mut(), 0, &mut written) != sys::OUT_OF_SPACE {
        return Err(ReplyError::Invalid);
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

/// Decode standard base64 with canonical padding and no stray bits: the strict form (the maintained `base64` crate's
/// `STANDARD` engine). Anything else is `Invalid`.
pub(crate) fn base64_decode(text: &str) -> Result<Vec<u8>, ReplyError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| ReplyError::Invalid)
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
            QueryReply::Clipboard {
                selection,
                data_base64,
            } => {
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
                fill(|buf, len, out| unsafe {
                    sys::ghostty_color_scheme_report_encode(scheme, buf, len, out)
                })
            }
        }
    }
}
