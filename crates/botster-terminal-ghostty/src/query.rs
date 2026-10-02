//! Terminal queries (EV-8): what the program asked, the exact bytes of the request, and the reply that the model's own
//! shadow computed at that point. libghostty finds the query, reads its kind and bytes, and computes the shadow reply;
//! this module only keeps them.

use botster_route_codec::prelude::QueryKind as LabelKind;

/// The kind of a query as libghostty reports it (`GhosttyTerminalQueryKind`). The values are the header's; a test
/// compares each with the header of the pinned Ghostty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum QueryKind {
    DeviceAttributesPrimary,
    DeviceAttributesSecondary,
    DeviceAttributesTertiary,
    OperatingStatus,
    CursorPosition,
    ColorScheme,
    Visibility,
    Enquiry,
    KittyKeyboard,
    ModeReport,
    Xtversion,
    Size14T,
    Size16T,
    Size18T,
    Size21T,
    Size11T,
    Size13T,
    Size15T,
    Size19T,
    Size20T,
    Decrqss,
    Xtgettcap,
    OscColor,
    KittyColor,
    ClipboardRead,
    KittyClipboardRead,
    Size14Of2T,
    Size13Of2T,
}

/// The kinds with their `GhosttyTerminalQueryKind` values. A kind that is not here is a library newer than this table.
pub(crate) const KINDS: &[(i32, QueryKind, &str)] = &[
    (1, QueryKind::DeviceAttributesPrimary, "GHOSTTY_TERMINAL_QUERY_DEVICE_ATTRIBUTES_PRIMARY"),
    (2, QueryKind::DeviceAttributesSecondary, "GHOSTTY_TERMINAL_QUERY_DEVICE_ATTRIBUTES_SECONDARY"),
    (3, QueryKind::DeviceAttributesTertiary, "GHOSTTY_TERMINAL_QUERY_DEVICE_ATTRIBUTES_TERTIARY"),
    (4, QueryKind::OperatingStatus, "GHOSTTY_TERMINAL_QUERY_OPERATING_STATUS"),
    (5, QueryKind::CursorPosition, "GHOSTTY_TERMINAL_QUERY_CURSOR_POSITION"),
    (6, QueryKind::ColorScheme, "GHOSTTY_TERMINAL_QUERY_COLOR_SCHEME"),
    (7, QueryKind::Visibility, "GHOSTTY_TERMINAL_QUERY_VISIBILITY"),
    (8, QueryKind::Enquiry, "GHOSTTY_TERMINAL_QUERY_ENQUIRY"),
    (9, QueryKind::KittyKeyboard, "GHOSTTY_TERMINAL_QUERY_KITTY_KEYBOARD"),
    (10, QueryKind::ModeReport, "GHOSTTY_TERMINAL_QUERY_MODE_REPORT"),
    (11, QueryKind::Xtversion, "GHOSTTY_TERMINAL_QUERY_XTVERSION"),
    (12, QueryKind::Size14T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_14_T"),
    (13, QueryKind::Size16T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_16_T"),
    (14, QueryKind::Size18T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_18_T"),
    (15, QueryKind::Size21T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_21_T"),
    (16, QueryKind::Size11T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_11_T"),
    (17, QueryKind::Size13T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_13_T"),
    (18, QueryKind::Size15T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_15_T"),
    (19, QueryKind::Size19T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_19_T"),
    (20, QueryKind::Size20T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_20_T"),
    (21, QueryKind::Decrqss, "GHOSTTY_TERMINAL_QUERY_DECRQSS"),
    (22, QueryKind::Xtgettcap, "GHOSTTY_TERMINAL_QUERY_XTGETTCAP"),
    (23, QueryKind::OscColor, "GHOSTTY_TERMINAL_QUERY_OSC_COLOR"),
    (24, QueryKind::KittyColor, "GHOSTTY_TERMINAL_QUERY_KITTY_COLOR"),
    (25, QueryKind::ClipboardRead, "GHOSTTY_TERMINAL_QUERY_CLIPBOARD_READ"),
    (26, QueryKind::KittyClipboardRead, "GHOSTTY_TERMINAL_QUERY_KITTY_CLIPBOARD_READ"),
    (27, QueryKind::Size14Of2T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_14_2_T"),
    (28, QueryKind::Size13Of2T, "GHOSTTY_TERMINAL_QUERY_SIZE_CSI_13_2_T"),
];

impl QueryKind {
    pub(crate) fn from_c(value: i32) -> Option<Self> {
        KINDS.iter().find(|(known, _, _)| *known == value).map(|(_, kind, _)| *kind)
    }
}

/// The terminator that an OSC 52 request used. A reply to the program uses the same one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terminator {
    St,
    Bel,
}

/// A query that the program made, at the point where the model saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub kind: QueryKind,
    /// The bytes of the recognized query sequence from its first byte to its final byte, without C0 controls that the
    /// terminal executes inside it (steward ruling R-17). `None` when the sequence did not go through
    /// `vt_write_until_query` from its start (a plain `vt_write` does not keep the bytes).
    pub request: Option<Vec<u8>>,
    /// True when the sequence was longer than the request limit and `request` holds its first bytes only.
    pub request_truncated: bool,
    /// The reply that the model's own shadow computed from its state at this point. Empty when the shadow has no
    /// answer (and always empty for a clipboard read: the shadow never answers one, EV-8). It is held: the binding
    /// writes it nowhere.
    pub shadow_reply: Vec<u8>,
    /// True when the shadow reply was longer than `MAX_SHADOW_REPLY_BYTES`: it is then empty.
    pub shadow_reply_overflow: bool,
    /// For a clipboard read: the selection exactly as the program wrote it (`s0` when it left it out).
    pub selection: Option<String>,
    /// For a clipboard read: the terminator of the request.
    pub terminator: Option<Terminator>,
}

/// The most bytes of shadow reply that the binding holds for one query (`max_query_reply_bytes`, EV-8).
pub const MAX_SHADOW_REPLY_BYTES: usize = 64 * 1024;

impl Query {
    pub(crate) fn new(kind: QueryKind, request: Option<Vec<u8>>, request_truncated: bool) -> Self {
        Self {
            kind,
            request,
            request_truncated,
            shadow_reply: Vec::new(),
            shadow_reply_overflow: false,
            selection: None,
            terminator: None,
        }
    }

    /// The contract's label of this query in the `terminal_query` frame, or `None` for the forms that have no typed
    /// kind (every other query is answered by `Encoded` bytes or `Decline`, EV-8).
    pub fn label(&self) -> Option<LabelKind> {
        Some(match self.kind {
            QueryKind::ClipboardRead => LabelKind::ClipboardRead { selection: self.selection.clone().unwrap_or_else(|| "s0".into()) },
            QueryKind::Size14T => LabelKind::TextAreaPixels,
            QueryKind::Size16T => LabelKind::CellPixels,
            QueryKind::Size15T => LabelKind::ScreenPixels,
            QueryKind::Size14Of2T => LabelKind::WindowPixels,
            QueryKind::Size19T => LabelKind::ScreenChars,
            QueryKind::Size11T => LabelKind::WindowState,
            QueryKind::Size13T => LabelKind::WindowPosition { area: "window".into() },
            QueryKind::Size13Of2T => LabelKind::WindowPosition { area: "text_area".into() },
            QueryKind::Size21T => LabelKind::WindowTitle,
            QueryKind::Size20T => LabelKind::IconLabel,
            QueryKind::ColorScheme => LabelKind::ColorScheme,
            _ => return None,
        })
    }
}

/// What one step of `vt_write_until_query` did.
#[derive(Debug, PartialEq, Eq)]
pub struct QueryStep {
    /// The bytes that the model took. Never more than the query's final byte, and fewer than the input when the model
    /// stopped at a query or kept an ESC that may start an ST (the caller offers the rest again).
    pub consumed: usize,
    /// The query that ended the step, if one did.
    pub query: Option<Query>,
}
