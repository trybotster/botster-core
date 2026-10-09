//! What the model reports while it runs, copied into a bounded buffer inside the callbacks (EV-1, EV-7).
//!
//! The callbacks run inside `vt_write`. They never block, lock or do I/O: each one copies what it is given into the
//! buffer. The caller drains the buffer after the write, in observation order.

use std::collections::BTreeSet;
use std::ffi::c_void;

use botster_core_contract::prelude::{LostKind, NotificationSource, PromptMarkKind};

use crate::query::{Query, QueryKind, Terminator, MAX_SHADOW_REPLY_BYTES};
use crate::sys;

/// The most events that the buffer holds between two drains.
pub const MAX_BUFFERED_EVENTS: usize = 4096;

/// The most bytes of event text that the buffer holds between two drains.
pub const MAX_BUFFERED_BYTES: usize = 16 * 1024 * 1024;

/// One thing that the model reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// OSC 0 or OSC 2 set the title. OSC 1 does not (Core erratum 2, E2-1).
    Title(String),
    /// OSC 7 set the working directory.
    Cwd(String),
    /// A BEL.
    Bell,
    /// OSC 9 or OSC 777, with the full text. The caller applies the bound of A2-4.
    Notification {
        source: NotificationSource,
        title: Option<String>,
        body: String,
    },
    /// OSC 133.
    PromptMark {
        mark: PromptMarkKind,
        exit_code: Option<i32>,
    },
    /// A clipboard write that the model recognizes: OSC 52, OSC 1337 Copy or OSC 5522 (Core Amendment 13, A13-1).
    ClipboardWrite(ClipboardWrite),
}

impl TerminalEvent {
    fn lost_kind(&self) -> Option<LostKind> {
        match self {
            TerminalEvent::Bell => Some(LostKind::Bell),
            TerminalEvent::PromptMark { .. } => Some(LostKind::PromptMark),
            TerminalEvent::Notification { .. } => Some(LostKind::Notification),
            TerminalEvent::ClipboardWrite(_) => Some(LostKind::ClipboardWrite),
            // Title and cwd are class K: the last value wins, so the model's own title and cwd reads stay exact.
            TerminalEvent::Title(_) | TerminalEvent::Cwd(_) => None,
        }
    }

    fn size(&self) -> usize {
        match self {
            TerminalEvent::Title(text) | TerminalEvent::Cwd(text) => text.len(),
            TerminalEvent::Bell | TerminalEvent::PromptMark { .. } => 0,
            TerminalEvent::Notification { title, body, .. } => {
                title.as_ref().map_or(0, String::len) + body.len()
            }
            TerminalEvent::ClipboardWrite(write) => write.size(),
        }
    }
}

/// Where the model says a clipboard write goes (`GhosttyClipboardLocation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardLocation {
    /// The standard clipboard.
    Standard,
    /// The selection clipboard (OSC 52 `s`).
    Selection,
    /// The primary selection (OSC 52 `p`).
    Primary,
    /// A location that this binding does not know (a library newer than the binding).
    Other(i32),
}

/// One representation of a clipboard value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardEntry {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// A clipboard write as the callback reports it (A13-1). The binding answers the model inside the callback: SUCCESS
/// when `total_bytes` is within the limit (`Terminal::set_clipboard_limit`), IO_ERROR when it is over (A13-1b).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardWrite {
    pub location: ClipboardLocation,
    /// The OSC 52 selection exactly as the program wrote it. `None` when it left it out and for the other protocols.
    pub selection: Option<String>,
    /// The terminator of the request (the other protocols report ST).
    pub terminator: Terminator,
    /// Every representation, in the model's order. `Some(vec![])` clears the destination, which is not the same as one
    /// entry with empty bytes. `None` when `too_large`: the bytes are not kept.
    pub contents: Option<Vec<ClipboardEntry>>,
    /// The sum of the byte lengths of all representations. For a write that the model did not keep, the decoded size of
    /// the whole transaction, as the model counted it (R-32): every decoded byte, including a representation that a later
    /// chunk of the same MIME type replaced.
    pub total_bytes: u64,
    /// The write is over the limit (Core A14-2): the model did not keep it, because its decoded size is over the
    /// model's limit, which is the same limit (step 1, R-32), or its contents size is over the limit (step 2). The model
    /// got IO_ERROR. `total_bytes` is the size of the step that decided.
    pub too_large: bool,
}

impl ClipboardWrite {
    fn size(&self) -> usize {
        self.selection.as_ref().map_or(0, String::len)
            + self.contents.as_ref().map_or(0, |entries| {
                entries.iter().map(|e| e.mime.len() + e.bytes.len()).sum()
            })
    }
}

/// The events of one drain, and what was dropped.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Drained {
    pub events: Vec<TerminalEvent>,
    /// How many events the buffer dropped because it was full. The dropped events are never silent.
    pub dropped: u64,
    /// The kinds of the dropped events that Core reports as lost (EV-2).
    pub dropped_kinds: BTreeSet<LostKind>,
    /// Bytes that libghostty wrote to the pty outside a query reply (an in-band size report when mode 2048 is set, for
    /// example). The binding writes them nowhere; the caller decides.
    pub pty_writes: Vec<u8>,
    /// The acknowledgements that the model wrote for OSC 5522 writes, one entry per write, in the order of the writes
    /// (A13-1b). They are not events: the event buffer drops its entries when it is full (class D), and an
    /// acknowledgement is never dropped. The worker writes each entry as one contiguous input transaction through its
    /// admission point, between transactions. The binding writes them nowhere. A caller that does not drain them gets
    /// `Error::AckBacklog` from `vt_write_until_query` once they pass `Terminal::set_ack_backlog_limit`.
    pub clipboard_acks: Vec<Vec<u8>>,
    /// How many queries a plain `vt_write` met. PTY output goes through `vt_write_until_query`, which stops at each
    /// query and returns it, so a nonzero count is a caller error. The count keeps the buffer bounded; a query that a
    /// plain write met got no answer from the caller (the library's shadow reply went to `pty_writes`).
    pub unrouted_queries: u64,
}

/// The buffer that the callbacks fill. It lives on the heap, at an address that does not move, and the terminal holds
/// a raw pointer to it as the callbacks' userdata.
#[derive(Default)]
pub(crate) struct Shared {
    events: Vec<TerminalEvent>,
    bytes: usize,
    dropped: u64,
    dropped_kinds: BTreeSet<LostKind>,
    pty_writes: Vec<u8>,
    unrouted_queries: u64,
    /// The largest clipboard write, in bytes of all representations, that the callback answers with SUCCESS.
    pub(crate) clipboard_limit: usize,
    /// While the callback runs the model's reply, the bytes that the model writes to answer the program.
    ack: Option<Vec<u8>>,
    /// The finished acknowledgements, in order. They are kept apart from `events` so that no event bound drops one.
    clipboard_acks: Vec<Vec<u8>>,
    /// The bytes in `clipboard_acks`.
    pub(crate) ack_bytes: usize,
    /// True while `vt_write_until_query` runs: the query goes to `held` instead of the event buffer.
    pub(crate) capture: bool,
    pub(crate) held: Option<Query>,
    /// The cell size in pixels that the host gave, which the shadow's size reports need (EV-8).
    pub(crate) cell_px: Option<(u32, u32)>,
}

impl Shared {
    /// Forget the query of the previous call. Every write starts here.
    pub(crate) fn begin_write(&mut self, capture: bool) {
        self.capture = capture;
        self.held = None;
    }

    fn open_query(&mut self) -> Option<&mut Query> {
        self.held.as_mut()
    }

    fn push_query(&mut self, query: Query) {
        if self.capture {
            self.held = Some(query);
        } else {
            self.unrouted_queries += 1;
        }
    }

    fn push(&mut self, event: TerminalEvent) {
        let size = event.size();
        if self.events.len() >= MAX_BUFFERED_EVENTS || self.bytes + size > MAX_BUFFERED_BYTES {
            self.dropped += 1;
            if let Some(kind) = event.lost_kind() {
                self.dropped_kinds.insert(kind);
            }
            return;
        }
        self.bytes += size;
        self.events.push(event);
    }

    pub(crate) fn drain(&mut self) -> Drained {
        self.bytes = 0;
        Drained {
            events: std::mem::take(&mut self.events),
            dropped: std::mem::take(&mut self.dropped),
            dropped_kinds: std::mem::take(&mut self.dropped_kinds),
            pty_writes: std::mem::take(&mut self.pty_writes),
            unrouted_queries: std::mem::take(&mut self.unrouted_queries),
            clipboard_acks: {
                self.ack_bytes = 0;
                std::mem::take(&mut self.clipboard_acks)
            },
        }
    }
}

/// The text of a string that the library reports, as a `String`. The library cuts a title at a byte count, which can
/// fall inside a UTF-8 character, so an incomplete tail is dropped and any other invalid byte becomes U+FFFD.
fn text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(error) if error.error_len().is_none() => {
            // SAFETY-free: the prefix up to `valid_up_to` is valid by the check.
            String::from_utf8_lossy(&bytes[..error.valid_up_to()]).into_owned()
        }
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Read a string datum of the terminal (the title or the working directory).
pub(crate) unsafe fn read_string(terminal: sys::Terminal, key: i32) -> String {
    let mut out = sys::GString {
        ptr: std::ptr::null(),
        len: 0,
    };
    // SAFETY: the caller passes the live terminal that the library handed to its callback; `out` is a valid
    // `GhosttyString` out pointer for the string keys.
    let code =
        unsafe { sys::ghostty_terminal_get(terminal, key, (&mut out as *mut sys::GString).cast()) };
    if code != sys::SUCCESS {
        return String::new();
    }
    // SAFETY: the library keeps the string valid until the next call that changes the terminal; it is copied now.
    text(unsafe { out.bytes() })
}

fn shared<'a>(userdata: *mut c_void) -> &'a mut Shared {
    // SAFETY: `userdata` is the `Shared` that `Terminal` boxed and registered. The terminal is `!Sync` and every call
    // that reaches a callback takes `&mut self`, and the terminal creates no other reference to `Shared` during such
    // a call, so this is the only live reference.
    unsafe { &mut *userdata.cast::<Shared>() }
}

pub(crate) unsafe extern "C" fn on_bell(_: sys::Terminal, userdata: *mut c_void) {
    shared(userdata).push(TerminalEvent::Bell);
}

pub(crate) unsafe extern "C" fn on_title_changed(terminal: sys::Terminal, userdata: *mut c_void) {
    // SAFETY: `terminal` is the live handle of the callback.
    let title = unsafe { read_string(terminal, sys::data::TITLE) };
    shared(userdata).push(TerminalEvent::Title(title));
}

pub(crate) unsafe extern "C" fn on_pwd_changed(terminal: sys::Terminal, userdata: *mut c_void) {
    // SAFETY: `terminal` is the live handle of the callback.
    let cwd = unsafe { read_string(terminal, sys::data::PWD) };
    shared(userdata).push(TerminalEvent::Cwd(cwd));
}

pub(crate) unsafe extern "C" fn on_notification(
    _: sys::Terminal,
    userdata: *mut c_void,
    notification: *const sys::DesktopNotification,
) {
    // SAFETY: the library passes a valid struct for the duration of the callback, and the strings with it.
    let notification = unsafe { &*notification };
    let (title, body) = unsafe {
        (
            text(notification.title.bytes()),
            text(notification.body.bytes()),
        )
    };
    let (source, title) = if notification.source == sys::notification_source::OSC777 {
        (NotificationSource::Osc777, Some(title))
    } else {
        // OSC 9 always has an empty title.
        (NotificationSource::Osc9, None)
    };
    shared(userdata).push(TerminalEvent::Notification {
        source,
        title,
        body,
    });
}

pub(crate) unsafe extern "C" fn on_semantic_prompt(
    _: sys::Terminal,
    userdata: *mut c_void,
    prompt: *const sys::SemanticPrompt,
) {
    // SAFETY: the library passes a valid struct for the duration of the callback.
    let prompt = unsafe { &*prompt };
    let mark = match prompt.kind {
        sys::semantic_prompt_kind::PROMPT_START => PromptMarkKind::PromptStart,
        sys::semantic_prompt_kind::INPUT_START => PromptMarkKind::CommandStart,
        sys::semantic_prompt_kind::OUTPUT_START => PromptMarkKind::CommandExecuted,
        sys::semantic_prompt_kind::COMMAND_END => PromptMarkKind::CommandFinished,
        _ => return,
    };
    let exit_code = prompt.has_exit_code.then_some(prompt.exit_code);
    shared(userdata).push(TerminalEvent::PromptMark { mark, exit_code });
}

pub(crate) unsafe extern "C" fn on_clipboard_write(
    _: sys::Terminal,
    userdata: *mut c_void,
    write: *const sys::ClipboardWrite,
) {
    // SAFETY: the library passes a valid request for the duration of the callback; its contents and strings are
    // borrowed for that time and copied here. The pointer is not kept after the callback.
    let request = unsafe { &*write };
    let selection = unsafe { request.selection.bytes() };
    let selection = (!selection.is_empty()).then(|| text(selection));
    let entries: &[sys::ClipboardContent] =
        if request.contents.is_null() || request.contents_len == 0 {
            &[]
        } else {
            // SAFETY: `contents` points at `contents_len` entries.
            unsafe { std::slice::from_raw_parts(request.contents, request.contents_len) }
        };
    // A write over the model's own transaction limit (R-32) carries no contents, only its length. The fields follow
    // `terminator`, so they are read only when `size` covers them.
    let model_too_large = request.size
        >= std::mem::offset_of!(sys::ClipboardWrite, total_len) + std::mem::size_of::<u64>()
        && request.too_large;
    let total_bytes: u64 = if model_too_large {
        request.total_len
    } else {
        // SAFETY: each entry's strings are valid for the callback.
        entries
            .iter()
            .map(|e| unsafe { e.data.bytes() }.len() as u64)
            .sum()
    };
    let limit = shared(userdata).clipboard_limit;
    let too_large = model_too_large || total_bytes > limit as u64;
    let contents = (!too_large).then(|| {
        entries
            .iter()
            .map(|e| ClipboardEntry {
                // SAFETY: as above.
                mime: text(unsafe { e.mime.bytes() }),
                bytes: unsafe { e.data.bytes() }.to_vec(),
            })
            .collect()
    });
    let location = match request.location {
        0 => ClipboardLocation::Standard,
        1 => ClipboardLocation::Selection,
        2 => ClipboardLocation::Primary,
        other => ClipboardLocation::Other(other),
    };
    let terminator = if request.terminator == 1 {
        Terminator::Bel
    } else {
        Terminator::St
    };

    // The reply must come before the callback returns, and a return without one denies the write. The binding decides
    // here (A13-1b, R-41): see `write_result`. The model writes the OSC 5522 acknowledgement while it handles the reply;
    // it is captured, not written.
    let reply = sys::ClipboardWriteReply {
        size: std::mem::size_of::<sys::ClipboardWriteReply>(),
        result: write_result(too_large, location),
        remember: false,
    };
    shared(userdata).ack = Some(Vec::new());
    // SAFETY: `request` and the reply are valid for the call, and this is the one reply of the callback.
    unsafe { (request.reply)(write, &reply) };
    // The acknowledgement is its own item. It does not travel in the event, which the bounded buffer may drop.
    let shared_state = shared(userdata);
    if let Some(ack) = shared_state.ack.take().filter(|ack| !ack.is_empty()) {
        shared_state.ack_bytes += ack.len();
        shared_state.clipboard_acks.push(ack);
    }

    shared(userdata).push(TerminalEvent::ClipboardWrite(ClipboardWrite {
        location,
        selection,
        terminator,
        contents,
        total_bytes,
        too_large,
    }));
}

/// The status of a clipboard write (A13-1b), decided by size alone: SUCCESS within the limit, IO_ERROR over it or over
/// the model's own limit. It is never DENIED, UNSUPPORTED or BUSY. A location that A13-1 names no letter for is
/// IO_ERROR (R-41): the worker posts no `ClipboardWrite` for it, only the loss marker.
pub(crate) fn write_result(too_large: bool, location: ClipboardLocation) -> i32 {
    if too_large || matches!(location, ClipboardLocation::Other(_)) {
        sys::CLIPBOARD_WRITE_IO_ERROR
    } else {
        sys::CLIPBOARD_WRITE_SUCCESS
    }
}

pub(crate) unsafe extern "C" fn on_query(
    _: sys::Terminal,
    userdata: *mut c_void,
    query: *const sys::Query,
) {
    // SAFETY: the library passes a valid struct, and the bytes in it, for the duration of the callback.
    let query = unsafe { &*query };
    let shared = shared(userdata);
    let Some(kind) = QueryKind::from_c(query.kind) else {
        // A kind that this binding does not know is a library newer than its table. It is counted, never silent.
        shared.dropped += 1;
        return;
    };
    // SAFETY: as above.
    let request = query
        .request_available
        .then(|| unsafe { query.request.bytes() }.to_vec());
    shared.push_query(Query::new(kind, request, query.request_truncated));
}

pub(crate) unsafe extern "C" fn on_clipboard_read(
    _: sys::Terminal,
    userdata: *mut c_void,
    read: *const sys::ClipboardRead,
) {
    // SAFETY: the library passes a valid request for the duration of the callback.
    let read = unsafe { &*read };
    // SAFETY: as above.
    let selection = unsafe { read.selection.bytes() };
    let selection = if selection.is_empty() {
        "s0".to_owned()
    } else {
        text(selection)
    };
    let terminator = if read.terminator == 1 {
        Terminator::Bel
    } else {
        Terminator::St
    };
    if let Some(query) = shared(userdata).open_query() {
        query.selection = Some(selection);
        query.terminator = Some(terminator);
    }
    // No reply. The shadow never answers a clipboard read (EV-8); the library then writes an empty answer to the pty,
    // and `on_write_pty` drops it.
}

pub(crate) unsafe extern "C" fn on_write_pty(
    _: sys::Terminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    // SAFETY: the library passes `len` valid bytes for the duration of the callback.
    let bytes = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    let shared = shared(userdata);
    if let Some(ack) = shared.ack.as_mut() {
        // The acknowledgement of a clipboard write. The library writes the status and the echoed id of the request,
        // so it is as long as the request allows, and it is never cut.
        ack.extend_from_slice(bytes);
        return;
    }
    if let Some(query) = shared.open_query() {
        // The shadow never answers a clipboard read.
        if matches!(
            query.kind,
            QueryKind::ClipboardRead | QueryKind::KittyClipboardRead
        ) {
            return;
        }
        if query.shadow_reply.len() + bytes.len() > MAX_SHADOW_REPLY_BYTES {
            query.shadow_reply.clear();
            query.shadow_reply_overflow = true;
        } else if !query.shadow_reply_overflow {
            query.shadow_reply.extend_from_slice(bytes);
        }
        return;
    }
    if shared.pty_writes.len() + bytes.len() > MAX_SHADOW_REPLY_BYTES {
        shared.dropped += 1;
    } else {
        shared.pty_writes.extend_from_slice(bytes);
    }
}

pub(crate) unsafe extern "C" fn on_size(
    terminal: sys::Terminal,
    userdata: *mut c_void,
    out: *mut sys::SizeReportSize,
) -> bool {
    // The pixel reports need the cell size, and an unknown cell size gives no answer (EV-8: external).
    let Some((cell_width, cell_height)) = shared(userdata).cell_px else {
        return false;
    };
    let mut cols: u16 = 0;
    let mut rows: u16 = 0;
    // SAFETY: the terminal is the live handle of the callback, and each key writes a `u16`.
    unsafe {
        sys::ghostty_terminal_get(terminal, sys::data::COLS, (&mut cols as *mut u16).cast());
        sys::ghostty_terminal_get(terminal, sys::data::ROWS, (&mut rows as *mut u16).cast());
        *out = sys::SizeReportSize {
            rows,
            columns: cols,
            cell_width,
            cell_height,
        };
    }
    true
}
