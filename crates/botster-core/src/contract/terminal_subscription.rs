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

/// Complete inventory admitted against caller-owned logical byte capacity.
///
/// Logical bytes count this result wrapper, occupied row storage (including inline
/// string and capability-set headers), identifier UTF-8 bytes, and one `String`
/// header plus UTF-8 bytes per capability token. They exclude allocator metadata,
/// spare capacity, and B-tree node padding/links.
/// This is an exact logical size, not a measurement of allocator-resident bytes.
/// Sorting is in place and needs no additional heap storage. The caller must
/// retain its reservation while it retains these records.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalSubscriptionInventory {
    /// All live rows, ordered by session, subscription, and generation.
    pub records: Vec<TerminalSubscriptionRecord>,
    /// Logical bytes charged by the producer before constructing the rows.
    pub logical_bytes: usize,
}

/// Inventory refusal before any output rows are allocated or cloned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TerminalSubscriptionInventoryError {
    /// The complete inventory exceeds the caller's allowance; no partial rows.
    #[error(
        "terminal inventory requires {required_bytes} logical bytes; allowance is {max_bytes}"
    )]
    BudgetTooSmall {
        /// Complete inventory's logical size.
        required_bytes: usize,
        /// Caller-supplied allowance.
        max_bytes: usize,
    },
    /// Logical sizing overflowed `usize`.
    #[error("terminal inventory logical byte size overflowed")]
    SizeOverflow,
}

/// Typed rejection from `bind_waking_terminal_adapter`.
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

/// Typed rejection from `record_attach` before any owner is created.
///
/// Not `#[non_exhaustive]`. Adding a variant is breaking.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AttachTerminalRouteError {
    /// The subscription id is not a valid scheme 2 route id.
    #[error("subscription id is not a valid terminal route: {subscription_id:?}")]
    InvalidRoute {
        /// Subscription presented to attach.
        subscription_id: SubscriptionId,
    },
    /// The shared generation allocator is exhausted for this incarnation.
    #[error("terminal route generations are exhausted")]
    GenerationExhausted,
}

/// One admitted client input operation staged for the worker.
///
/// Core already validated the frame and reserved lane capacity. `body` is the
/// worker-side body for `kind`: the client body bytes for raw, key, mouse,
/// focus, and resize operations, and `[u8 allow_unsafe][paste bytes]` for an
/// assembled paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedTerminalInput {
    /// Client that owns the subscription.
    pub client_id: ClientId,
    /// Session the operation targets.
    pub session_id: SessionId,
    /// Subscription that submitted the operation.
    pub subscription_id: SubscriptionId,
    /// Live generation at staging time.
    pub generation: TerminalSubscriptionGeneration,
    /// Core-unique key echoed by the worker result.
    pub operation_key: u64,
    /// Client-chosen operation id.
    pub operation_id: u64,
    /// Worker input kind.
    pub kind: crate::session_protocol::WorkerInputKind,
    /// Worker-side operation body.
    pub body: Vec<u8>,
    /// Client payload bytes Core admitted, reported back in the result.
    pub accepted_payload_bytes: u64,
}
