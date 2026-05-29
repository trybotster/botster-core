//! Transport-neutral ingress and egress frames.

use serde::{Deserialize, Serialize};

use crate::client::{ClientId, ClientState};
use crate::session::{RequestId, SessionId, SubscriptionId};

/// Input from a concrete client transport into the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportIngress {
    /// Subscribe a client to a session stream.
    SubscribeSession {
        /// Client requesting the subscription.
        client_id: ClientId,
        /// Session to subscribe to.
        session_id: SessionId,
        /// Transport-local subscription id.
        subscription_id: SubscriptionId,
    },
    /// Unsubscribe a client from a session stream.
    UnsubscribeSession {
        /// Client requesting unsubscribe.
        client_id: ClientId,
        /// Session to unsubscribe from.
        session_id: SessionId,
        /// Transport-local subscription id.
        subscription_id: SubscriptionId,
    },
    /// Raw terminal input bytes.
    TerminalInput {
        /// Target session.
        session_id: SessionId,
        /// Input bytes.
        data: Vec<u8>,
    },
    /// Client liveness update.
    ClientState {
        /// Reporting client.
        client_id: ClientId,
        /// New state.
        state: ClientState,
    },
    /// Request/response ping.
    Ping {
        /// Request correlation id.
        request_id: RequestId,
    },
}

/// Output from the runtime to a concrete client transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportEgress {
    /// Raw terminal output bytes.
    TerminalOutput {
        /// Source session.
        session_id: SessionId,
        /// Output bytes.
        data: Vec<u8>,
    },
    /// Terminal snapshot payload.
    Snapshot {
        /// Source session.
        session_id: SessionId,
        /// Opaque snapshot payload.
        data: Vec<u8>,
    },
    /// Request/response pong.
    Pong {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Close the concrete transport.
    Close {
        /// Human-readable close reason.
        reason: String,
    },
}
