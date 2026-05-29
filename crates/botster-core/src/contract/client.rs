//! Client identifiers, scopes, and liveness state.

use serde::{Deserialize, Serialize};

/// Stable identifier for a connected Botster client.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientId(pub String);

/// Scope granted to a connected client.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientScope {
    /// Client can read terminal/session output.
    TerminalRead,
    /// Client can write terminal input.
    TerminalWrite,
    /// Client can read entity frames.
    EntityRead,
    /// Client can invoke registered actions.
    ActionInvoke,
}

/// Transport-neutral client state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientState {
    /// Client is connected and ready.
    Ready,
    /// Client is alive but slow or congested.
    Backpressured,
    /// Client is reconnecting.
    Reconnecting,
    /// Client disconnected.
    Disconnected,
}
