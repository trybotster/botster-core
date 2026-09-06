//! Semantic decode of scheme 2 stream frames for clients.
//!
//! Payload kinds (`Output`, `SnapshotReady`, `SnapshotHistory`) keep the
//! shared frame and expose the body as a borrowed slice. No copy and no
//! base64 occur between the transport buffer and the renderer.

use std::sync::Arc;

use botster_terminal_protocol::{
    decode_attach_state, decode_history_unavailable, decode_input_result, decode_modes,
    decode_process_exit, decode_route_resync, mode_bits, AttachStateCode, HistoryUnavailableReason,
    InputResultBody, ModesBody, ProcessExitBody, RouteResyncBody, TerminalBodyError, TerminalFrame,
    TerminalFrameError, TerminalKind,
};

/// Decoded scheme 2 stream event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Raw PTY bytes. Read them with [`TerminalFrame::body`].
    Output(TerminalFrame),
    /// Raw GHOSTSNP ready-phase bytes. Read them with [`TerminalFrame::body`].
    SnapshotReady(TerminalFrame),
    /// Raw GHOSTSNP history page. Read it with [`TerminalFrame::body`].
    SnapshotHistory(TerminalFrame),
    /// Snapshot delivery finished.
    SnapshotFinish,
    /// Session process exited.
    ProcessExit(ProcessExitBody),
    /// Terminal modes and geometry changed.
    Modes(ModesBody),
    /// Route attach state changed.
    AttachState(AttachStateCode),
    /// Result of one input operation on this route.
    InputResult(InputResultBody),
    /// Retained history is unavailable for this route.
    HistoryUnavailable(HistoryUnavailableReason),
    /// Route egress overflowed. Accept only when `from_epoch` equals the
    /// accepted epoch and the envelope epoch equals `to_epoch`; then reset
    /// decoder state. `MODES` and a fresh `SnapshotReady` follow.
    RouteResync(RouteResyncBody),
}

impl TerminalEvent {
    /// Wire kind of this event.
    #[must_use]
    pub const fn kind(&self) -> TerminalKind {
        match self {
            Self::Output(_) => TerminalKind::Output,
            Self::SnapshotReady(_) => TerminalKind::SnapshotReady,
            Self::SnapshotHistory(_) => TerminalKind::SnapshotHistory,
            Self::SnapshotFinish => TerminalKind::SnapshotFinish,
            Self::ProcessExit(_) => TerminalKind::ProcessExit,
            Self::Modes(_) => TerminalKind::Modes,
            Self::AttachState(_) => TerminalKind::AttachState,
            Self::InputResult(_) => TerminalKind::InputResult,
            Self::HistoryUnavailable(_) => TerminalKind::HistoryUnavailable,
            Self::RouteResync(_) => TerminalKind::RouteResync,
        }
    }

    /// Payload bytes for `Output`, `SnapshotReady`, and `SnapshotHistory`.
    #[must_use]
    pub fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Output(frame) | Self::SnapshotReady(frame) | Self::SnapshotHistory(frame) => {
                Some(frame.body())
            }
            _ => None,
        }
    }
}

/// Decode a validated frame into a typed event.
pub fn decode_terminal_event(frame: &TerminalFrame) -> Result<TerminalEvent, TerminalBodyError> {
    Ok(match frame.kind() {
        TerminalKind::Output => TerminalEvent::Output(frame.clone()),
        TerminalKind::SnapshotReady => TerminalEvent::SnapshotReady(frame.clone()),
        TerminalKind::SnapshotHistory => TerminalEvent::SnapshotHistory(frame.clone()),
        TerminalKind::SnapshotFinish => {
            expect_empty(frame)?;
            TerminalEvent::SnapshotFinish
        }
        TerminalKind::ProcessExit => TerminalEvent::ProcessExit(decode_process_exit(frame)?),
        TerminalKind::Modes => TerminalEvent::Modes(decode_modes(frame)?),
        TerminalKind::AttachState => TerminalEvent::AttachState(decode_attach_state(frame)?),
        TerminalKind::InputResult => TerminalEvent::InputResult(decode_input_result(frame)?),
        TerminalKind::HistoryUnavailable => {
            TerminalEvent::HistoryUnavailable(decode_history_unavailable(frame)?)
        }
        TerminalKind::RouteResync => TerminalEvent::RouteResync(decode_route_resync(frame)?),
    })
}

/// Validate complete `TerminalBody` bytes and decode them into a typed event.
pub fn decode_terminal_body(bytes: &[u8]) -> Result<TerminalEvent, TerminalEventError> {
    let frame = TerminalFrame::from_bytes(bytes)?;
    Ok(decode_terminal_event(&frame)?)
}

/// Adopt shared `TerminalBody` bytes and decode them into a typed event.
pub fn decode_shared_terminal_body(bytes: Arc<[u8]>) -> Result<TerminalEvent, TerminalEventError> {
    let frame = TerminalFrame::from_shared(bytes)?;
    Ok(decode_terminal_event(&frame)?)
}

fn expect_empty(frame: &TerminalFrame) -> Result<(), TerminalBodyError> {
    if !frame.body().is_empty() {
        return Err(TerminalBodyError::BodyLength {
            expected: 0,
            actual: frame.body().len(),
        });
    }
    Ok(())
}

/// Header or body failure while decoding a complete `TerminalBody`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalEventError {
    /// Header validation failed.
    #[error(transparent)]
    Frame(#[from] TerminalFrameError),
    /// Body decode failed.
    #[error(transparent)]
    Body(#[from] TerminalBodyError),
}

/// Boolean view of a `MODES` bit set. Mirrors Core `ModeFlags` one for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalModeFlags {
    /// Kitty keyboard protocol is enabled.
    pub kitty_enabled: bool,
    /// Cursor is visible.
    pub cursor_visible: bool,
    /// Bracketed paste is enabled.
    pub bracketed_paste: bool,
    /// Normal mouse tracking (mode 1000).
    pub mouse_normal: bool,
    /// Any-event mouse tracking (mode 1003).
    pub mouse_any: bool,
    /// Button-event mouse tracking (mode 1002).
    pub mouse_button: bool,
    /// SGR mouse encoding (mode 1006).
    pub mouse_sgr: bool,
    /// Alternate screen is active.
    pub alt_screen: bool,
    /// Focus reporting is enabled.
    pub focus_reporting: bool,
    /// Application cursor keys are enabled.
    pub application_cursor: bool,
}

impl TerminalModeFlags {
    /// Expand a `MODES` bit set.
    #[must_use]
    pub const fn from_mode_bits(bits: u32) -> Self {
        Self {
            kitty_enabled: bits & mode_bits::KITTY_KEYBOARD != 0,
            cursor_visible: bits & mode_bits::CURSOR_VISIBLE != 0,
            bracketed_paste: bits & mode_bits::BRACKETED_PASTE != 0,
            mouse_normal: bits & mode_bits::MOUSE_NORMAL != 0,
            mouse_any: bits & mode_bits::MOUSE_ANY != 0,
            mouse_button: bits & mode_bits::MOUSE_BUTTON != 0,
            mouse_sgr: bits & mode_bits::MOUSE_SGR != 0,
            alt_screen: bits & mode_bits::ALT_SCREEN != 0,
            focus_reporting: bits & mode_bits::FOCUS_REPORTING != 0,
            application_cursor: bits & mode_bits::APPLICATION_CURSOR != 0,
        }
    }

    /// Collapse to a `MODES` bit set.
    #[must_use]
    pub const fn to_mode_bits(self) -> u32 {
        let mut bits = 0;
        if self.kitty_enabled {
            bits |= mode_bits::KITTY_KEYBOARD;
        }
        if self.cursor_visible {
            bits |= mode_bits::CURSOR_VISIBLE;
        }
        if self.bracketed_paste {
            bits |= mode_bits::BRACKETED_PASTE;
        }
        if self.mouse_normal {
            bits |= mode_bits::MOUSE_NORMAL;
        }
        if self.mouse_any {
            bits |= mode_bits::MOUSE_ANY;
        }
        if self.mouse_button {
            bits |= mode_bits::MOUSE_BUTTON;
        }
        if self.mouse_sgr {
            bits |= mode_bits::MOUSE_SGR;
        }
        if self.alt_screen {
            bits |= mode_bits::ALT_SCREEN;
        }
        if self.focus_reporting {
            bits |= mode_bits::FOCUS_REPORTING;
        }
        if self.application_cursor {
            bits |= mode_bits::APPLICATION_CURSOR;
        }
        bits
    }

    /// Whether any mouse tracking mode is active.
    #[must_use]
    pub const fn mouse_tracking(self) -> bool {
        self.mouse_normal || self.mouse_any || self.mouse_button
    }
}
