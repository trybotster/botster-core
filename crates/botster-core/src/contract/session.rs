//! Session identifiers, host metadata, and portable activity state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::actor::SessionLifecycleState;

/// Maximum encoded JSON size for core session host metadata.
///
/// This mirrors the handshake metadata cap posture: hosts can attach small,
/// durable classification values, not arbitrary runtime state blobs.
pub const MAX_CORE_SESSION_METADATA_LEN: usize = 64 * 1024;

/// Stable identifier for a Botster session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

/// Stable identifier for a client subscription to a session or stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(pub String);

/// Opaque request correlation identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub String);

/// Host-owned session metadata that core serializes but does not interpret.
///
/// Values are string-only and should be small classification facts such as a
/// namespaced `session_type`. Hosts are responsible for excluding PII such as
/// cwd, title, username, prompt text, or terminal content. Plugin-owned runtime
/// state belongs in plugin entities or plugin state, not in this core metadata
/// surface.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CoreSessionMetadata {
    /// Host-owned classification entries.
    #[serde(default)]
    pub entries: BTreeMap<String, String>,
}

impl CoreSessionMetadata {
    /// Build empty host metadata.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Build host metadata from entries.
    #[must_use]
    pub fn from_entries(entries: BTreeMap<String, String>) -> Self {
        Self { entries }
    }

    /// Whether the encoded metadata is within core's public cap.
    #[must_use]
    pub fn is_within_encoded_len_limit(&self) -> bool {
        serde_json::to_vec(self)
            .map(|encoded| encoded.len() <= MAX_CORE_SESSION_METADATA_LEN)
            .unwrap_or(false)
    }
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
    /// Unix seconds of the last input byte observed by core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_input_at: Option<u64>,
    /// Unix seconds of the last output byte observed by core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_output_at: Option<u64>,
    /// Unix seconds of the last non-byte activity signal declared by a host.
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
    /// Host-owned metadata serialized by core but not interpreted by core.
    #[serde(default)]
    pub metadata: CoreSessionMetadata,
    /// Lifecycle summary shared with actor contracts.
    pub lifecycle: SessionLifecycleState,
    /// Portable activity accounting.
    #[serde(default)]
    pub activity: SessionActivity,
}

impl CoreSession {
    /// Build a core session with empty activity accounting.
    #[must_use]
    pub fn new(session_id: SessionId, lifecycle: SessionLifecycleState) -> Self {
        Self::with_metadata(session_id, lifecycle, CoreSessionMetadata::default())
    }

    /// Build a core session with host-owned metadata and empty activity accounting.
    #[must_use]
    pub fn with_metadata(
        session_id: SessionId,
        lifecycle: SessionLifecycleState,
        metadata: CoreSessionMetadata,
    ) -> Self {
        Self {
            session_id,
            metadata,
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
        /// Unix seconds for the observed bytes.
        at: u64,
        /// Number of bytes observed. Zero-byte events do not refresh activity.
        bytes: u64,
    },
    /// PTY or transport output bytes were observed.
    OutputBytes {
        /// Unix seconds for the observed bytes.
        at: u64,
        /// Number of bytes observed. Zero-byte events do not refresh activity.
        bytes: u64,
    },
    /// Non-byte activity was declared by a host using its own policy boundary.
    DeclaredActivity {
        /// Unix seconds for the declared activity.
        at: u64,
    },
    /// Lifecycle changed without implying byte activity.
    Lifecycle {
        /// New lifecycle state.
        state: SessionLifecycleState,
    },
}
