//! Transport-neutral routed envelope contracts for multiplexer coordination.

use serde::{Deserialize, Serialize};

use crate::actor::{BoundedQueueConfig, PluginKey};
use crate::boundary::BoundaryJson;
use crate::client::ClientId;
use crate::session::{SessionId, SubscriptionId};

/// Stable identifier for a multiplexer endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(pub String);

/// Stable identifier for a routed envelope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EnvelopeId(pub String);

/// In-memory monotonic cursor assigned by the routed envelope primitive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnvelopeCursor(pub u64);

/// Core-recognized target families for routed envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EnvelopeTarget {
    /// Route to a stable generic endpoint.
    Endpoint {
        /// Target endpoint id.
        endpoint_id: EndpointId,
    },
    /// Route to a connected client.
    Client {
        /// Target client id.
        client_id: ClientId,
    },
    /// Route to a core session.
    Session {
        /// Target session id.
        session_id: SessionId,
    },
    /// Route to one client subscription on one session.
    Subscription {
        /// Target session id.
        session_id: SessionId,
        /// Target subscription id.
        subscription_id: SubscriptionId,
    },
    /// Route to a plugin-owned runtime boundary.
    Plugin {
        /// Target plugin key.
        plugin_key: PluginKey,
    },
    /// Route to a named stream owned by the embedding host or plugin.
    Stream {
        /// Stable stream name.
        stream: String,
    },
    /// Route to a named topic owned by the embedding host or plugin.
    Topic {
        /// Stable topic name.
        topic: String,
    },
}

/// Payload metadata whose schema remains owned above core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelopePayload {
    /// Stable media type or host-defined payload family.
    pub content_type: String,
    /// Opaque payload bytes. Core routes and accounts for bytes, but does not inspect semantics.
    pub body: Vec<u8>,
    /// Extension-owned structured payload schema, opaque to core.
    pub extension: Option<BoundaryJson>,
}

/// Envelope submitted to the routed envelope primitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelope {
    /// Stable envelope id assigned by the caller.
    pub id: EnvelopeId,
    /// Source endpoint submitting the envelope.
    pub source: EndpointId,
    /// Requested target routes.
    pub targets: Vec<EnvelopeTarget>,
    /// Opaque payload plus typed metadata.
    pub payload: RoutedEnvelopePayload,
    /// Deterministic creation timestamp supplied by the host.
    pub created_at: u64,
    /// Cursor assigned by core when queued.
    pub cursor: Option<EnvelopeCursor>,
}

impl RoutedEnvelope {
    /// Build a routed envelope with no assigned cursor.
    #[must_use]
    pub fn new(
        id: EnvelopeId,
        source: EndpointId,
        targets: Vec<EnvelopeTarget>,
        payload: RoutedEnvelopePayload,
        created_at: u64,
    ) -> Self {
        Self {
            id,
            source,
            targets,
            payload,
            created_at,
            cursor: None,
        }
    }
}

/// Policy-free delivery state for one envelope at one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeDeliveryStatus {
    /// Envelope was accepted into a target queue.
    Queued,
    /// Envelope was drained by its target.
    Delivered,
    /// Envelope was acknowledged by its target.
    Acknowledged,
    /// Envelope was dropped by host policy.
    Dropped,
    /// Envelope expired before delivery.
    Expired,
    /// Target queue was full when publish attempted delivery.
    Backpressured,
    /// Delivery failed at a caller-owned boundary.
    Failed,
}

/// Delivery state for one target copy of an envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeDeliveryState {
    /// Envelope id.
    pub envelope_id: EnvelopeId,
    /// Target whose delivery state is tracked.
    pub target: EnvelopeTarget,
    /// Cursor assigned to this target delivery.
    pub cursor: EnvelopeCursor,
    /// Current delivery status.
    pub status: EnvelopeDeliveryStatus,
}

/// Queue settings for the routed envelope primitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelopeQueueConfig {
    /// Maximum queued envelopes per target.
    pub per_target_capacity: usize,
}

impl RoutedEnvelopeQueueConfig {
    /// Build a queue config.
    #[must_use]
    pub const fn new(per_target_capacity: usize) -> Self {
        Self {
            per_target_capacity,
        }
    }
}

impl Default for RoutedEnvelopeQueueConfig {
    fn default() -> Self {
        Self {
            per_target_capacity: 128,
        }
    }
}

impl From<BoundedQueueConfig> for RoutedEnvelopeQueueConfig {
    fn from(config: BoundedQueueConfig) -> Self {
        Self {
            per_target_capacity: config.capacity,
        }
    }
}

/// Observable routed envelope decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutedEnvelopeObservation {
    /// A subscriber was registered for a route.
    Subscribed {
        /// Route being observed.
        route: EnvelopeTarget,
        /// Endpoint that now receives route fanout.
        subscriber: EnvelopeTarget,
    },
    /// A subscriber was removed from a route.
    Unsubscribed {
        /// Route being observed.
        route: EnvelopeTarget,
        /// Endpoint removed from route fanout.
        subscriber: EnvelopeTarget,
    },
    /// One target copy was queued.
    Queued {
        /// Envelope id.
        envelope_id: EnvelopeId,
        /// Delivery target.
        target: EnvelopeTarget,
        /// Assigned cursor.
        cursor: EnvelopeCursor,
    },
    /// One target queue reported pressure.
    Backpressured {
        /// Envelope id.
        envelope_id: EnvelopeId,
        /// Delivery target.
        target: EnvelopeTarget,
        /// Queue capacity.
        capacity: usize,
        /// Queue depth at publish time.
        depth: usize,
    },
    /// One target copy was delivered.
    Delivered {
        /// Envelope id.
        envelope_id: EnvelopeId,
        /// Delivery target.
        target: EnvelopeTarget,
        /// Delivered cursor.
        cursor: EnvelopeCursor,
    },
    /// One target copy was acknowledged.
    Acknowledged {
        /// Envelope id.
        envelope_id: EnvelopeId,
        /// Delivery target.
        target: EnvelopeTarget,
    },
}

/// Result of publishing one routed envelope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelopePublishOutcome {
    /// Per-target delivery states created or updated by publish.
    pub deliveries: Vec<EnvelopeDeliveryState>,
    /// Observations emitted while publishing.
    pub observations: Vec<RoutedEnvelopeObservation>,
}

/// Result of draining one target queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutedEnvelopeDrainOutcome {
    /// Envelopes delivered to the target.
    pub envelopes: Vec<RoutedEnvelope>,
    /// Cursor the caller can use to resume after this drain.
    pub next_cursor: Option<EnvelopeCursor>,
    /// Observations emitted while draining.
    pub observations: Vec<RoutedEnvelopeObservation>,
}
