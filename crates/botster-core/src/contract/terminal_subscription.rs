//! Control-plane terminal subscription inventory and bind/detach types.
//!
//! These records are identity only. They do not duplicate attach phases,
//! snapshot bytes, queue contents, or decoder state.

use serde::{Deserialize, Serialize};

use crate::client::ClientId;
use crate::session::{SessionId, SubscriptionId};

pub use botster_terminal_protocol::{TerminalCapabilitySet, TerminalCapabilitySetError};

/// Monotonic generation assigned by Core on attach.
///
/// Reuse of the same `subscription_id` after teardown receives `generation + 1`.
/// Adding fields later is additive; this newtype is exhaustive at `0.1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TerminalSubscriptionGeneration(pub u64);

/// Control-plane inventory row for one live terminal subscription.
///
/// Forbidden fields: READY/PAGE/FINISH, attach phase, snapshot bytes, queue
/// contents, and decoder state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSubscriptionRecord {
    /// Client that owns the subscription.
    pub client_id: ClientId,
    /// Session the subscription is attached to.
    pub session_id: SessionId,
    /// Host-chosen subscription identity.
    pub subscription_id: SubscriptionId,
    /// Core-assigned generation for this live owner.
    pub generation: TerminalSubscriptionGeneration,
    /// Whether a [`crate::contract::terminal_adapter::TerminalAdapter`] is bound.
    pub adapter_bound: bool,
    /// Bound negotiated tokens. `None` before bind. Bound empty is `Some` empty.
    pub capabilities: Option<TerminalCapabilitySet>,
}

/// Typed rejection from `bind_terminal_adapter`.
///
/// Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is breaking.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindTerminalAdapterError {
    /// Bind was attempted before attach created an inventory row.
    #[error("bind before attach for session {session_id:?} subscription {subscription_id:?}")]
    BindBeforeAttach {
        /// Session presented to bind.
        session_id: SessionId,
        /// Subscription presented to bind.
        subscription_id: SubscriptionId,
    },
    /// No live inventory row matches the presented identity.
    #[error("unknown terminal subscription {subscription_id:?} on session {session_id:?}")]
    UnknownSubscription {
        /// Session presented to bind.
        session_id: SessionId,
        /// Subscription presented to bind.
        subscription_id: SubscriptionId,
    },
    /// Bind carried a generation that is not the live attach generation.
    #[error("stale terminal subscription generation: live {live:?}, requested {requested:?}")]
    StaleGeneration {
        /// Live generation, if any.
        live: Option<TerminalSubscriptionGeneration>,
        /// Generation presented to bind.
        requested: TerminalSubscriptionGeneration,
    },
    /// The live generation already has a bound adapter.
    #[error(
        "adapter already bound for session {session_id:?} subscription {subscription_id:?} generation {generation:?}"
    )]
    AlreadyBound {
        /// Session of the live owner.
        session_id: SessionId,
        /// Subscription of the live owner.
        subscription_id: SubscriptionId,
        /// Live generation that already holds an adapter.
        generation: TerminalSubscriptionGeneration,
    },
    /// The session control plane has failed and admits no new owner.
    #[error("control plane failed for session {session_id:?}")]
    ControlPlaneFailed {
        /// Session whose control plane failed.
        session_id: SessionId,
    },
}

/// Result of a generation-aware detach.
///
/// Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is breaking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetachTerminalSubscriptionResult {
    /// The matching live generation was torn down.
    Detached {
        /// Generation that was removed.
        generation: TerminalSubscriptionGeneration,
    },
    /// No live owner existed for that identity.
    AlreadyGone,
    /// A live owner exists, but it is a different generation.
    GenerationMismatch {
        /// Generation currently live.
        live: TerminalSubscriptionGeneration,
        /// Generation presented to detach.
        requested: TerminalSubscriptionGeneration,
    },
}

pub use botster_terminal_protocol_client::TerminalInputCommand;

/// One Core-owned terminal input operation ready for Stage B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInputOperation {
    /// One existing single-frame terminal command.
    Command(TerminalInputCommand),
    /// One fully assembled bounded paste.
    Paste(PasteOperation),
}

/// One complete paste after Stage A validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteOperation {
    /// Client-chosen operation id.
    pub operation_id: u32,
    /// Worker ownership epoch expected at atomic admission.
    pub mode_generation: u64,
    /// Complete-ModeFlags counter expected at atomic admission.
    pub mode_revision: u64,
    /// Complete content bytes before optional bracket wrapping.
    pub data: Vec<u8>,
}

/// One dequeued ingress command ready to apply on the production tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInputDelivery {
    /// Client that owns the subscription.
    pub client_id: ClientId,
    /// Session the command targets.
    pub session_id: SessionId,
    /// Subscription that submitted the command.
    pub subscription_id: SubscriptionId,
    /// Live generation at dequeue time.
    pub generation: TerminalSubscriptionGeneration,
    /// Decoded and validated operation.
    pub command: TerminalInputOperation,
}
