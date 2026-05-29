//! Session identifiers, vocabulary, and portable activity state.

use serde::{Deserialize, Serialize};

use crate::actor::SessionLifecycleState;

/// Stable identifier for a Botster session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

/// Stable identifier for a client subscription to a session or stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(pub String);

/// Opaque request correlation identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

/// Transport-neutral session kind.
///
/// Core keeps the vocabulary broad enough for common Botster embeddings while
/// preserving a custom escape hatch for hosts with their own session taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    /// Interactive terminal or shell session.
    Terminal,
    /// Generic process session without terminal-specific assumptions.
    Process,
    /// Agent CLI session. Botster treats this as one use of the multiplexer,
    /// not as a product-only core assumption.
    Agent,
    /// Session owned by a plugin.
    Plugin {
        /// Stable plugin key.
        plugin_key: String,
    },
    /// Embedder-owned kind value.
    Custom(String),
}

/// Deterministic activity classification for a session at an injected time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionActivityStatus {
    /// Recent byte or declared activity is within the caller-provided threshold.
    Active,
    /// No activity exists or the latest activity is older than the threshold.
    Idle,
}

/// Byte and declared-activity accounting for a session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SessionActivity {
    /// Unix timestamp of the last input byte observed by core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_input_at: Option<u64>,
    /// Total input bytes observed by core.
    #[serde(default)]
    pub input_bytes: u64,
    /// Unix timestamp of the last output byte observed by core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_output_at: Option<u64>,
    /// Total output bytes observed by core.
    #[serde(default)]
    pub output_bytes: u64,
    /// Unix timestamp of the last non-byte activity signal declared by a host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_declared_activity_at: Option<u64>,
}

impl SessionActivity {
    /// Return the newest input, output, or declared activity timestamp.
    #[must_use]
    pub fn latest_activity_at(&self) -> Option<u64> {
        [
            self.last_input_at,
            self.last_output_at,
            self.last_declared_activity_at,
        ]
        .into_iter()
        .flatten()
        .max()
    }
}

/// Portable core session state used by embedders and host runtimes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSession {
    /// Stable session identifier.
    pub session_id: SessionId,
    /// Transport-neutral session kind.
    pub kind: SessionKind,
    /// Lifecycle summary shared with actor contracts.
    pub lifecycle: SessionLifecycleState,
    /// Portable activity accounting.
    #[serde(default)]
    pub activity: SessionActivity,
}

impl CoreSession {
    /// Build a core session with empty activity accounting.
    #[must_use]
    pub fn new(session_id: SessionId, kind: SessionKind, lifecycle: SessionLifecycleState) -> Self {
        Self {
            session_id,
            kind,
            lifecycle,
            activity: SessionActivity::default(),
        }
    }
}

/// Pure activity and lifecycle events accepted by the core reducer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionActivityEvent {
    /// PTY or transport input bytes were observed.
    InputBytes {
        /// Unix timestamp for the observed bytes.
        at: u64,
        /// Number of bytes observed. Zero-byte events do not refresh activity.
        bytes: u64,
    },
    /// PTY or transport output bytes were observed.
    OutputBytes {
        /// Unix timestamp for the observed bytes.
        at: u64,
        /// Number of bytes observed. Zero-byte events do not refresh activity.
        bytes: u64,
    },
    /// Non-byte activity was declared by a host using its own policy boundary.
    DeclaredActivity {
        /// Unix timestamp for the declared activity.
        at: u64,
    },
    /// Lifecycle changed without implying byte activity.
    Lifecycle {
        /// New lifecycle state.
        state: SessionLifecycleState,
    },
}
