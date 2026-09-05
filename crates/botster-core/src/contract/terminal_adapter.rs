//! Content-blind duplex terminal adapter contract.
//!
//! This is an advanced host/adapter seam. The embedder start-here path remains
//! spawn → attach → drain → input → shutdown through [`crate::prelude`].
//!
//! [`TerminalAdapter`] is not a replacement for [`super::transport::TransportEgress`].
//! Those enums remain the current semantic drain-path frames for unbound
//! subscriptions. Bound subscriptions leave through ClientWorker `try_write`.
//!
//! Public enums in this module are exhaustive at `0.1.0`. Adding a variant is a
//! breaking change.

use botster_terminal_protocol::TerminalFrame;

/// Minimum complete ingress frames a conforming adapter must hold before it
/// may report [`TerminalIngress::Lost`].
///
/// Equal to Core's per-subscription intake budget so a host that drains every
/// tick can take everything a conforming adapter is required to hold.
pub const MIN_ADAPTER_INGRESS_BUFFER_FRAMES: usize = 64;

/// Content-blind duplex terminal adapter.
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
/// Close, whether local [`Self::close`] or transport-side death, abandons the
/// adapter slot and any unsent transport copy. Terminal frames do not retry.
///
/// A transport may finish an envelope with positive write progress to preserve
/// stream framing. It must use the writer's existing buffer, without replaying
/// the frame or starting another envelope from the closed adapter. A nonblocking
/// write already in progress may complete concurrently with close. If that
/// attempt accepts bytes, the transport may finish that envelope. If it makes
/// no progress, the transport must not retry it after close. The transport must
/// finish a partial envelope or end the affected stream. It must not leave an
/// incomplete envelope on a live stream.
///
/// During normal operation, the slot remains occupied until the complete
/// envelope is written. Partial write progress must not report the slot Ready.
///
/// [`Self::close`] and [`Drop`] must return without waiting for transport I/O.
/// They set [`TerminalAdapterPressure::Closed`] and abandon the in-flight slot.
/// They must not perform transport I/O waits or take a lock that can wait on
/// the transport writer. A blocking `close()` is an illegal adapter and fails
/// the published conformance harness. Core calls `close()` synchronously and
/// does not spawn a closer thread.
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
    /// Idempotent and non-blocking. After close, [`Self::try_write`] returns
    /// [`TerminalAdapterWriteError::Closed`] and [`Self::pressure`] is
    /// [`TerminalAdapterPressure::Closed`]. [`Self::try_read`] returns
    /// [`TerminalIngress::Closed`] permanently and buffered ingress is dropped.
    /// Unsent frames are abandoned and must not start or retry after close.
    /// An already-started transport envelope follows the framing rule above.
    /// Return without waiting for transport I/O or for a lock held by the
    /// transport writer, including completion of a partial envelope.
    fn close(&mut self);

    /// Current transport pressure.
    fn pressure(&self) -> TerminalAdapterPressure;

    /// Take the next ingress event. Never blocks.
    fn try_read(&mut self) -> TerminalIngress;
}

/// Typed ingress outcome from [`TerminalAdapter::try_read`].
///
/// Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is breaking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalIngress {
    /// No complete frame is buffered. The stream is still contiguous.
    Empty,
    /// One complete frame, in arrival order.
    Frame(Vec<u8>),
    /// The transport dropped at least one frame. The stream is no longer
    /// contiguous. Carries no payload and no count of lost bytes.
    Lost,
    /// The adapter is closed. Terminal state.
    Closed,
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
