//! Filesystem-backed daemon session registry.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use botster_core::{ProcessIdentity, ResizePayload, SessionId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Durable session lifecycle state recorded by the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrySessionState {
    /// Session is known to be running.
    Running,
    /// Session is stopping.
    Stopping,
    /// Session exited cleanly.
    Exited,
    /// Session record is stale or adoption failed.
    Stale,
}

/// Durable non-PII session metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryRecord {
    /// Session id.
    pub session_id: SessionId,
    /// Durable state.
    pub state: RegistrySessionState,
    /// Process identity when known.
    pub process: Option<ProcessIdentity>,
    /// Current terminal rows.
    pub rows: u16,
    /// Current terminal columns.
    pub cols: u16,
    /// Spawn executable basename or caller-supplied non-PII label.
    pub command_label: String,
    /// Logical creation timestamp.
    pub created_at: u64,
    /// Logical update timestamp.
    pub updated_at: u64,
    /// Protocol version observed for the session worker.
    pub protocol_version: u8,
    /// Whether the HELLO/WELCOME restart-contract handshake has been observed.
    pub handshake_verified: bool,
    /// Whether ping/pong liveness is available for this worker.
    pub ping_pong_supported: bool,
    /// Optional recovery identity from the session-worker protocol.
    pub recovery_identity: Option<serde_json::Value>,
}

impl RegistryRecord {
    /// Build a running record from a spawn result.
    #[must_use]
    pub fn running(
        session_id: SessionId,
        process: Option<ProcessIdentity>,
        size: ResizePayload,
        command_label: String,
        now_seconds: u64,
    ) -> Self {
        Self {
            session_id,
            state: RegistrySessionState::Running,
            process,
            rows: size.rows,
            cols: size.cols,
            command_label,
            created_at: now_seconds,
            updated_at: now_seconds,
            protocol_version: botster_core::PROTOCOL_VERSION,
            handshake_verified: false,
            ping_pong_supported: false,
            recovery_identity: None,
        }
    }

    /// Record restart-contract evidence observed from the session-worker protocol.
    ///
    /// Callers should only use this after the daemon has observed the
    /// HELLO/WELCOME handshake, FRAME_PING/PONG liveness, and recovery identity
    /// from [`botster_core::SessionMetadata`].
    pub fn observe_restart_contract(
        &mut self,
        recovery_identity: serde_json::Value,
        now_seconds: u64,
    ) {
        self.protocol_version = botster_core::PROTOCOL_VERSION;
        self.handshake_verified = true;
        self.ping_pong_supported = true;
        self.recovery_identity = Some(recovery_identity);
        self.updated_at = now_seconds;
    }

    /// Update state and timestamp.
    pub fn mark(&mut self, state: RegistrySessionState, now_seconds: u64) {
        self.state = state;
        self.updated_at = now_seconds;
    }
}

/// Registry persistence error.
#[derive(Debug, Error)]
pub enum SessionRegistryError {
    /// Filesystem error.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Serialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Record filename was not valid UTF-8.
    #[error("registry record filename is not valid UTF-8")]
    InvalidRecordPath,
}

/// Filesystem-backed registry with one JSON record per session.
#[derive(Debug, Clone)]
pub struct SessionRegistry {
    root: PathBuf,
}

impl SessionRegistry {
    /// Build a registry under a caller-provided data directory.
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: data_dir.into().join("sessions"),
        }
    }

    /// Return the registry root.
    #[must_use]
    pub const fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Save one record atomically enough for local daemon metadata.
    pub fn save(&self, record: &RegistryRecord) -> Result<(), SessionRegistryError> {
        fs::create_dir_all(&self.root)?;
        let path = self.record_path(&record.session_id);
        let temp_path = path.with_extension("json.tmp");
        let data = serde_json::to_vec_pretty(record)?;
        fs::write(&temp_path, data)?;
        fs::rename(temp_path, path)?;
        Ok(())
    }

    /// Load one record if it exists.
    pub fn load(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<RegistryRecord>, SessionRegistryError> {
        let path = self.record_path(session_id);
        if !path.exists() {
            return Ok(None);
        }
        let data = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&data)?))
    }

    /// Load every registry record.
    pub fn load_all(&self) -> Result<Vec<RegistryRecord>, SessionRegistryError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let data = fs::read(path)?;
            if let Ok(record) = serde_json::from_slice(&data) {
                records.push(record);
            }
        }
        records.sort_by(|left: &RegistryRecord, right| left.session_id.0.cmp(&right.session_id.0));
        Ok(records)
    }

    /// Remove one record.
    pub fn remove(&self, session_id: &SessionId) -> Result<(), SessionRegistryError> {
        let path = self.record_path(session_id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn record_path(&self, session_id: &SessionId) -> PathBuf {
        self.root.join(record_filename(session_id))
    }
}

fn record_filename(session_id: &SessionId) -> String {
    let safe: String = session_id
        .0
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.json")
}

/// Return the non-sensitive basename of an executable path.
#[must_use]
pub fn command_label(executable: &str) -> String {
    Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("command")
        .to_string()
}
