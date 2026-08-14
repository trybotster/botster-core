//! Content-blind terminal egress adapter contract.
//!
//! This is an advanced host/adapter seam. The embedder start-here path remains
//! spawn → attach → drain → input → shutdown through [`crate::prelude`].
//!
//! [`TerminalAdapter`] is not a replacement for [`super::transport::TransportEgress`].
//! Those enums remain the current semantic drain-path frames. This trait is the
//! bounded, content-blind write/close/pressure contract later ClientWorker and
//! Hub adapters will implement.
//!
//! Public enums in this module are exhaustive at `0.1.0`. Adding a variant is a
//! breaking change.

use botster_terminal_protocol::TerminalFrame;

/// Content-blind terminal egress adapter.
///
/// The adapter owns at most one transport-internal active write. That slot is
/// transport state, not a policy queue. Implementations must not enqueue extra
/// frames, retry rejected writes, reorder accepted frames, or inspect
/// [`TerminalFrame`] bodies. Serializing with [`TerminalFrame::to_bytes`] is
/// allowed.
///
/// `Ok(())` means the frame occupies the single active-write slot until the
/// transport finishes that write. It does not mean a client received the frame.
///
/// Close, whether local [`Self::close`] or transport-side death, abandons any
/// in-flight frame. Terminal frames do not retry.
pub trait TerminalAdapter {
    /// Attempt a non-blocking write of one opaque terminal frame.
    ///
    /// # Errors
    ///
    /// Returns [`TerminalAdapterWriteError::WouldBlock`] when the write slot is
    /// empty but the transport is not ready, [`TerminalAdapterWriteError::Full`]
    /// when the one active-write slot is occupied, or
    /// [`TerminalAdapterWriteError::Closed`] after close.
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError>;

    /// Close the adapter.
    ///
    /// Idempotent. After close, [`Self::try_write`] returns
    /// [`TerminalAdapterWriteError::Closed`] and [`Self::pressure`] is
    /// [`TerminalAdapterPressure::Closed`]. An in-flight frame is abandoned and
    /// must not be delivered later.
    fn close(&mut self);

    /// Current transport pressure.
    fn pressure(&self) -> TerminalAdapterPressure;
}

/// Typed rejection from [`TerminalAdapter::try_write`].
///
/// Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAdapterWriteError {
    /// Transport is not ready even though the write slot is empty.
    ///
    /// The adapter must not retain the rejected frame.
    WouldBlock,
    /// The one active-write slot is occupied.
    ///
    /// The adapter must not retain the rejected frame.
    Full,
    /// Adapter is closed. Further writes stay `Closed`.
    ///
    /// The adapter must not retain the rejected frame.
    Closed,
}

/// Pollable pressure from [`TerminalAdapter::pressure`].
///
/// Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAdapterPressure {
    /// The write slot is empty and the transport can accept a frame.
    Ready,
    /// The write slot is empty but the transport is not ready.
    WouldBlock,
    /// The one active-write slot is occupied.
    Full,
    /// Adapter is closed.
    Closed,
}
