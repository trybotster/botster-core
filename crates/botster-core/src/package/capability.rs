//! Extension capability declarations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// A broad capability surface guarded by the hub.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySurface {
    /// Publish or consume UI/entity surfaces.
    Surfaces,
    /// Register session actions.
    SessionActions,
    /// Register MCP tools, prompts, or resources.
    Mcp,
    /// Use plugin-owned durable storage.
    PluginDb,
    /// Use scoped filesystem access.
    Filesystem,
    /// Use outbound network primitives.
    Network,
    /// Register bounded timer or interval callbacks.
    Timers,
    /// Use secret storage by operation.
    Secrets,
    /// Request crypto operations without raw key access.
    Crypto,
    /// Participate in client admission decisions.
    ClientAdmission,
    /// Create or accept pairing invites.
    PairingInvites,
    /// Relay signaling envelopes.
    SignalingRelay,
    /// Publish hub presence to an external registry.
    HubPresence,
    /// Provide or route browser shell access.
    BrowserShell,
}

/// A single capability request.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Capability {
    /// Capability surface.
    pub surface: CapabilitySurface,
    /// Optional narrower scope within the surface.
    pub scope: Option<String>,
}

/// Ordered set of capabilities.
pub type CapabilitySet = BTreeSet<Capability>;
