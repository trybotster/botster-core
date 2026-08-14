//! Transport-neutral ingress and egress frames.
//!
//! These enums are semantic drain-path frames. They are not the content-blind
//! adapter trait. That contract lives in
//! [`crate::contract::terminal_adapter`].

use serde::{Deserialize, Serialize};

use crate::actor::TerminalAttachState;
use crate::boundary::BoundaryJson;
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
    /// Resize a terminal session.
    Resize {
        /// Target session.
        session_id: SessionId,
        /// New row count.
        rows: u16,
        /// New column count.
        cols: u16,
    },
    /// Request a terminal snapshot.
    RequestSnapshot {
        /// Request correlation id.
        request_id: RequestId,
        /// Target session.
        session_id: SessionId,
    },
    /// Prepare and write a send-file payload.
    SendFile {
        /// Request correlation id.
        request_id: RequestId,
        /// Target session.
        session_id: SessionId,
        /// Send-file bytes.
        data: Vec<u8>,
    },
    /// Update terminal focus state.
    Focus {
        /// Target session.
        session_id: SessionId,
        /// Whether the client is focused.
        focused: bool,
    },
    /// Transport heartbeat.
    Heartbeat {
        /// Request correlation id.
        request_id: RequestId,
    },
    /// Relay or plugin-owned ingress payload.
    BoundaryPayload {
        /// Peer-local route id.
        route_id: String,
        /// Opaque payload.
        payload: BoundaryJson,
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
        /// Subscription route receiving the output.
        subscription_id: SubscriptionId,
        /// Output bytes.
        data: Vec<u8>,
    },
    /// Terminal snapshot payload.
    Snapshot {
        /// Source session.
        session_id: SessionId,
        /// Subscription route receiving the snapshot.
        subscription_id: SubscriptionId,
        /// Opaque snapshot payload.
        data: Vec<u8>,
    },
    /// Scrollback payload.
    Scrollback {
        /// Source session.
        session_id: SessionId,
        /// Subscription route receiving the scrollback.
        subscription_id: SubscriptionId,
        /// Opaque scrollback bytes.
        data: Vec<u8>,
    },
    /// Session process exit.
    ProcessExit {
        /// Source session.
        session_id: SessionId,
        /// Subscription route receiving the exit notification.
        subscription_id: SubscriptionId,
        /// Process exit code.
        code: Option<i32>,
    },
    /// Terminal attach state changed.
    AttachState {
        /// Source session.
        session_id: SessionId,
        /// Subscription route receiving the attach state.
        subscription_id: SubscriptionId,
        /// Transport-neutral attach state.
        state: TerminalAttachState,
    },
    /// Terminal focus changed.
    FocusChanged {
        /// Source session.
        session_id: SessionId,
        /// Subscription route receiving the focus change.
        subscription_id: SubscriptionId,
        /// Whether focus is active.
        focused: bool,
    },
    /// Binary payload owned by the concrete adapter.
    Binary {
        /// Payload bytes.
        data: Vec<u8>,
    },
    /// Relay or plugin-owned egress payload.
    BoundaryPayload {
        /// Peer-local route id.
        route_id: String,
        /// Opaque payload.
        payload: BoundaryJson,
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
