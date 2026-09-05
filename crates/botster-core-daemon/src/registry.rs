//! Filesystem-backed daemon session registry.

#[cfg(test)]
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use botster_core::{CoreSessionMetadata, ProcessIdentity, ResizePayload, SessionId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const RECORD_FORMAT: &str = "botster.session-registry.v1";
const RECORD_FILENAME_PREFIX: &str = "v1.";

#[derive(Serialize)]
struct StoredRecord<'a> {
    format: &'static str,
    #[serde(flatten)]
    record: &'a RegistryRecord,
}

#[derive(Deserialize)]
struct StoredRecordFormat {
    #[serde(default)]
    format: serde_json::Value,
}

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
    /// Opaque host-owned metadata retained for lifecycle projection and adoption.
    #[serde(default)]
    pub metadata: CoreSessionMetadata,
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
    /// Number of extra live worker candidates claiming this session identity.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub duplicate_worker_candidates: usize,
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
            metadata: CoreSessionMetadata::new(),
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
            duplicate_worker_candidates: 0,
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
    /// The record identity does not match the identity that selected its file.
    #[error("registry record identity does not match its filename")]
    IdentityMismatch,
    /// The file uses an unsupported registry format.
    #[error("registry record format is unsupported")]
    UnsupportedFormat,
}

/// Filesystem-backed registry with one JSON record per session.
#[derive(Debug, Clone)]
pub struct SessionRegistry {
    root: PathBuf,
    #[cfg(test)]
    load_all_calls: Cell<u64>,
}

impl SessionRegistry {
    /// Build a registry under a caller-provided data directory.
    #[must_use]
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: data_dir.into().join("sessions"),
            #[cfg(test)]
            load_all_calls: Cell::new(0),
        }
    }

    /// Return the registry root.
    #[must_use]
    pub const fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Save one record after verifying any existing record's format and identity.
    pub fn save(&self, record: &RegistryRecord) -> Result<(), SessionRegistryError> {
        self.load(&record.session_id)?;
        fs::create_dir_all(&self.root)?;
        let path = self.record_path(&record.session_id);
        let temp_path = path.with_extension("json.tmp");
        match read_record(&temp_path) {
            Ok(Some(existing)) => verify_identity(&existing, &record.session_id)?,
            // A crash can leave incomplete temporary JSON. The primary record stays strictly checked.
            Ok(None) | Err(SessionRegistryError::Json(_)) => {}
            Err(error) => return Err(error),
        }
        let data = serde_json::to_vec_pretty(&StoredRecord {
            format: RECORD_FORMAT,
            record,
        })?;
        fs::write(&temp_path, data)?;
        fs::rename(temp_path, path)?;
        Ok(())
    }

    /// Load one record after verifying its format and exact session identity.
    ///
    /// Legacy files cause an error. This method does not scan or migrate files.
    pub fn load(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<RegistryRecord>, SessionRegistryError> {
        let Some(record) = read_record(&self.record_path(session_id))? else {
            self.reject_legacy_record(session_id)?;
            return Ok(None);
        };
        verify_identity(&record, session_id)?;
        Ok(Some(record))
    }

    /// Load one record, skipping malformed JSON like [`Self::load_all`].
    ///
    /// Missing and malformed files return `Ok(None)`. Identity, format, and I/O errors remain errors.
    pub fn load_skip_malformed(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<RegistryRecord>, SessionRegistryError> {
        match self.load(session_id) {
            Err(SessionRegistryError::Json(_)) => Ok(None),
            result => result,
        }
    }

    /// Count of [`Self::load_all`] calls. Exact-session tests use this to
    /// fail if a query scans the registry collection.
    #[cfg(test)]
    #[must_use]
    pub fn test_load_all_calls(&self) -> u64 {
        self.load_all_calls.get()
    }

    /// Load supported records whose stored identities match their filenames.
    ///
    /// Skip malformed JSON and temporary files. Reject foreign or unsupported records without changing files.
    pub fn load_all(&self) -> Result<Vec<RegistryRecord>, SessionRegistryError> {
        #[cfg(test)]
        self.load_all_calls
            .set(self.load_all_calls.get().saturating_add(1));
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            if let Some(record) = self.load_entry(&entry?)? {
                records.push(record);
            }
        }
        records.sort_by(|left: &RegistryRecord, right| left.session_id.0.cmp(&right.session_id.0));
        Ok(records)
    }

    // The paged daemon scan uses this reader without learning the filename encoding.
    pub(crate) fn load_entry(
        &self,
        entry: &fs::DirEntry,
    ) -> Result<Option<RegistryRecord>, SessionRegistryError> {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".json") {
            return Err(SessionRegistryError::UnsupportedFormat);
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            return Ok(None);
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(SessionRegistryError::InvalidRecordPath)?;
        if !is_current_record_filename(filename) {
            return Err(SessionRegistryError::UnsupportedFormat);
        }
        match read_record(&path) {
            Ok(Some(record)) => {
                if filename != record_filename(&record.session_id) {
                    return Err(SessionRegistryError::IdentityMismatch);
                }
                Ok(Some(record))
            }
            Ok(None) | Err(SessionRegistryError::Json(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Remove one record after verifying its format and exact session identity.
    pub fn remove(&self, session_id: &SessionId) -> Result<(), SessionRegistryError> {
        if self.load(session_id)?.is_some() {
            fs::remove_file(self.record_path(session_id))?;
        }
        Ok(())
    }

    fn reject_legacy_record(&self, session_id: &SessionId) -> Result<(), SessionRegistryError> {
        match fs::symlink_metadata(self.root.join(legacy_record_filename(session_id))) {
            Ok(_) => Err(SessionRegistryError::UnsupportedFormat),
            // An overlong legacy filename cannot exist. The new filename has a fixed length.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidFilename
                ) =>
            {
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    fn record_path(&self, session_id: &SessionId) -> PathBuf {
        self.root.join(record_filename(session_id))
    }
}

fn record_filename(session_id: &SessionId) -> String {
    // The digest selects a candidate. Stored identity still decides whether the record matches.
    let digest = Sha256::digest(session_id.0.as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{RECORD_FILENAME_PREFIX}{hex}.json")
}

fn is_current_record_filename(filename: &str) -> bool {
    filename
        .strip_prefix(RECORD_FILENAME_PREFIX)
        .and_then(|stem| stem.strip_suffix(".json"))
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn read_record(path: &Path) -> Result<Option<RegistryRecord>, SessionRegistryError> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let stored: StoredRecordFormat = serde_json::from_slice(&data)?;
    if stored.format.as_str() != Some(RECORD_FORMAT) {
        return Err(SessionRegistryError::UnsupportedFormat);
    }
    Ok(Some(serde_json::from_slice(&data)?))
}

fn verify_identity(
    record: &RegistryRecord,
    session_id: &SessionId,
) -> Result<(), SessionRegistryError> {
    if record.session_id != *session_id {
        return Err(SessionRegistryError::IdentityMismatch);
    }
    Ok(())
}

// This filename is used only to reject an exact legacy file. Never read or write a record through it.
fn legacy_record_filename(session_id: &SessionId) -> String {
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

fn is_zero(value: &usize) -> bool {
    *value == 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture {
        registry: SessionRegistry,
        data_dir: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let data_dir = std::env::temp_dir().join(format!(
                "botster-registry-identity-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock after epoch")
                    .as_nanos()
            ));
            let registry = SessionRegistry::new(&data_dir);
            fs::create_dir_all(registry.root()).expect("create registry fixture");
            Self { registry, data_dir }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.data_dir);
        }
    }

    fn record(id: &str) -> RegistryRecord {
        RegistryRecord::running(
            SessionId(id.to_string()),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "sh".to_string(),
            1,
        )
    }

    #[test]
    fn colliding_sanitizer_ids_keep_distinct_records() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let mut first = record("audit:a");
        let second = record("audit_a");
        registry.save(&first).expect("save punctuation id");
        registry.save(&second).expect("save underscore id");
        assert_ne!(
            registry.record_path(&first.session_id),
            registry.record_path(&second.session_id)
        );
        assert_eq!(
            registry.load(&first.session_id).expect("load first"),
            Some(first.clone())
        );
        assert_eq!(
            registry.load(&second.session_id).expect("load second"),
            Some(second.clone())
        );
        assert_eq!(
            registry.load_all().expect("list both"),
            vec![first.clone(), second.clone()]
        );
        first.rows = 40;
        registry.save(&first).expect("update only first");
        assert_eq!(
            registry.load(&first.session_id).expect("load update"),
            Some(first.clone())
        );
        assert_eq!(
            registry.load(&second.session_id).expect("load sibling"),
            Some(second.clone())
        );
        registry.remove(&first.session_id).expect("remove first");
        assert!(registry
            .load(&first.session_id)
            .expect("first is absent")
            .is_none());
        assert_eq!(
            registry.load(&second.session_id).expect("preserve sibling"),
            Some(second)
        );
    }

    #[test]
    fn private_filename_uses_full_sha256_and_a_distinct_versioned_namespace() {
        assert_eq!(
            record_filename(&SessionId("abc".to_string())),
            "v1.ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.json"
        );
        let fixture = Fixture::new();
        let expected = record("abc");
        fixture
            .registry
            .save(&expected)
            .expect("save formatted record");
        let bytes =
            fs::read(fixture.registry.record_path(&expected.session_id)).expect("stored bytes");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("stored JSON");
        assert_eq!(value["format"], "botster.session-registry.v1");
        assert_eq!(value["session_id"], "abc");
        assert_eq!(
            serde_json::from_slice::<RegistryRecord>(&bytes).expect("public record"),
            expected
        );
        assert!(serde_json::to_value(&expected)
            .expect("public JSON")
            .get("format")
            .is_none());
    }

    #[test]
    fn unicode_punctuation_and_long_ids_round_trip_without_restrictions() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        for id in [
            "".to_string(),
            "../a/b\\c:!? .".to_string(),
            "会話:é🦀".to_string(),
            "x".repeat(1_000),
            "é".repeat(1_000),
        ] {
            let expected = record(&id);
            registry.save(&expected).expect("save arbitrary id");
            assert_eq!(record_filename(&expected.session_id).len(), 3 + 64 + 5);
            assert_eq!(
                registry
                    .load(&expected.session_id)
                    .expect("load arbitrary id"),
                Some(expected.clone())
            );
            assert_eq!(
                registry
                    .load_skip_malformed(&expected.session_id)
                    .expect("exact tolerant read"),
                Some(expected.clone())
            );
            registry
                .remove(&expected.session_id)
                .expect("remove arbitrary id");
            assert!(registry
                .load(&expected.session_id)
                .expect("removed id")
                .is_none());
        }
        assert!(registry.load_all().expect("empty registry").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn overlong_legacy_probe_does_not_reject_a_valid_new_identity() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let expected = record(&"x".repeat(1_000));
        let legacy = registry
            .root()
            .join(legacy_record_filename(&expected.session_id));
        assert_eq!(
            fs::symlink_metadata(legacy)
                .expect_err("legacy filename exceeds the filesystem limit")
                .kind(),
            io::ErrorKind::InvalidFilename
        );
        assert!(registry
            .load(&expected.session_id)
            .expect("missing digest file")
            .is_none());
        registry
            .save(&expected)
            .expect("save with bounded filename");
        assert_eq!(
            registry
                .load(&expected.session_id)
                .expect("load exact identity"),
            Some(expected.clone())
        );
        registry
            .remove(&expected.session_id)
            .expect("remove exact identity");
        assert_eq!(registry.test_load_all_calls(), 0);
    }

    #[test]
    fn foreign_identity_is_rejected_before_return_overwrite_or_remove() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let requested = record("audit:a");
        let foreign = record("private/foreign-session");
        registry.save(&foreign).expect("save foreign source");
        let bytes = fs::read(registry.record_path(&foreign.session_id)).expect("foreign bytes");
        let path = registry.record_path(&requested.session_id);
        fs::write(&path, &bytes).expect("place foreign record at requested path");
        assert!(matches!(
            registry.load(&requested.session_id),
            Err(SessionRegistryError::IdentityMismatch)
        ));
        assert!(matches!(
            registry.load_skip_malformed(&requested.session_id),
            Err(SessionRegistryError::IdentityMismatch)
        ));
        assert!(matches!(
            registry.save(&requested),
            Err(SessionRegistryError::IdentityMismatch)
        ));
        assert_eq!(fs::read(&path).expect("bytes after refused save"), bytes);
        assert!(matches!(
            registry.remove(&requested.session_id),
            Err(SessionRegistryError::IdentityMismatch)
        ));
        assert_eq!(fs::read(&path).expect("bytes after refused removal"), bytes);
        assert!(matches!(
            registry.load_all(),
            Err(SessionRegistryError::IdentityMismatch)
        ));
        assert_eq!(fs::read(&path).expect("bytes after refused scan"), bytes);
        assert_eq!(
            SessionRegistryError::IdentityMismatch.to_string(),
            "registry record identity does not match its filename"
        );
    }

    #[test]
    fn unsupported_formats_are_rejected_before_record_shape_and_preserve_bytes() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let requested = record("unsupported");
        let path = registry.record_path(&requested.session_id);
        for bytes in [
            serde_json::to_vec(&requested).expect("unversioned public record"),
            br#"{"format":"botster.session-registry.v2"}"#.to_vec(),
            br#"{"format":1}"#.to_vec(),
        ] {
            fs::write(&path, &bytes).expect("unsupported fixture");
            assert!(matches!(
                registry.load(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.load_skip_malformed(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.save(&requested),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert_eq!(fs::read(&path).expect("bytes after refused save"), bytes);
            assert!(matches!(
                registry.remove(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.load_all(),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert_eq!(fs::read(&path).expect("unsupported bytes remain"), bytes);
        }
    }

    #[test]
    fn legacy_paths_are_rejected_without_migration_or_scans() {
        for id in [String::new(), "audit:a".to_string(), "a".repeat(64)] {
            let fixture = Fixture::new();
            let registry = &fixture.registry;
            let requested = record(&id);
            let bytes = serde_json::to_vec(&requested).expect("legacy JSON");
            let path = registry
                .root()
                .join(legacy_record_filename(&requested.session_id));
            fs::write(&path, &bytes).expect("legacy fixture");
            assert!(matches!(
                registry.load(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.load_skip_malformed(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.save(&requested),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert!(matches!(
                registry.remove(&requested.session_id),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert_eq!(registry.test_load_all_calls(), 0);
            assert!(!registry.record_path(&requested.session_id).exists());
            assert!(matches!(
                registry.load_all(),
                Err(SessionRegistryError::UnsupportedFormat)
            ));
            assert_eq!(fs::read(path).expect("legacy bytes remain"), bytes);
        }
    }

    #[test]
    fn malformed_current_files_are_skipped_only_by_tolerant_reads() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let good = record("good");
        let bad = record("bad");
        registry.save(&good).expect("save good record");
        let path = registry.record_path(&bad.session_id);
        let bytes = b"not JSON";
        fs::write(&path, bytes).expect("malformed fixture");
        assert!(matches!(
            registry.load(&bad.session_id),
            Err(SessionRegistryError::Json(_))
        ));
        assert!(registry
            .load_skip_malformed(&bad.session_id)
            .expect("skip malformed")
            .is_none());
        assert!(matches!(
            registry.save(&bad),
            Err(SessionRegistryError::Json(_))
        ));
        assert!(matches!(
            registry.remove(&bad.session_id),
            Err(SessionRegistryError::Json(_))
        ));
        assert_eq!(
            registry.load_all().expect("skip malformed during scan"),
            vec![good]
        );
        assert_eq!(
            fs::read(path).expect("malformed bytes remain").as_slice(),
            bytes
        );
    }

    #[test]
    fn save_recovers_from_malformed_temporary_json() {
        for primary_exists in [false, true] {
            let fixture = Fixture::new();
            let registry = &fixture.registry;
            let mut expected = record("crash-recovery");
            if primary_exists {
                registry
                    .save(&expected)
                    .expect("save initial primary record");
            }
            let path = registry.record_path(&expected.session_id);
            let temp_path = path.with_extension("json.tmp");
            fs::write(
                &temp_path,
                br#"{"format":"botster.session-registry.v1","session_id":"#,
            )
            .expect("write incomplete temporary JSON");
            assert!(matches!(
                read_record(&temp_path),
                Err(SessionRegistryError::Json(_))
            ));
            expected.rows = 40;
            registry
                .save(&expected)
                .expect("replace incomplete temporary JSON");
            assert_eq!(
                registry
                    .load(&expected.session_id)
                    .expect("load recovered primary"),
                Some(expected)
            );
            assert!(
                !temp_path.exists(),
                "rename must consume the recovered temporary file"
            );
        }
    }

    #[test]
    fn scans_ignore_temporary_files_and_saves_preserve_foreign_temporary_data() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        let expected = record("temp");
        let foreign = record("foreign-temp");
        registry.save(&expected).expect("save record");
        let path = registry
            .record_path(&expected.session_id)
            .with_extension("json.tmp");
        let cases = [
            (
                serde_json::to_vec(&StoredRecord {
                    format: RECORD_FORMAT,
                    record: &foreign,
                })
                .expect("valid foreign temporary record"),
                true,
            ),
            (
                serde_json::to_vec(&StoredRecord {
                    format: "botster.session-registry.v2",
                    record: &expected,
                })
                .expect("unsupported temporary format"),
                false,
            ),
            (
                serde_json::to_vec(&foreign).expect("unversioned temporary record"),
                false,
            ),
        ];
        for (bytes, foreign_identity) in cases {
            fs::write(&path, &bytes).expect("temporary fixture");
            assert_eq!(
                registry.load_all().expect("ignore temporary file"),
                vec![expected.clone()]
            );
            let error = registry
                .save(&expected)
                .expect_err("refuse foreign or unsupported temporary record");
            match error {
                SessionRegistryError::IdentityMismatch if foreign_identity => {}
                SessionRegistryError::UnsupportedFormat if !foreign_identity => {}
                other => panic!("unexpected temporary record error: {other:?}"),
            }
            assert_eq!(fs::read(&path).expect("temporary bytes remain"), bytes);
            assert_eq!(
                registry
                    .load(&expected.session_id)
                    .expect("committed record remains"),
                Some(expected.clone())
            );
        }
    }

    #[test]
    fn exact_operations_do_not_scan_unrelated_files() {
        let fixture = Fixture::new();
        let registry = &fixture.registry;
        fs::write(registry.root().join("unsupported.json"), b"{}")
            .expect("unrelated unsupported file");
        let expected = record("exact");
        registry
            .save(&expected)
            .expect("exact save ignores unrelated files");
        assert_eq!(
            registry.load(&expected.session_id).expect("exact load"),
            Some(expected.clone())
        );
        assert_eq!(
            registry
                .load_skip_malformed(&expected.session_id)
                .expect("exact tolerant load"),
            Some(expected.clone())
        );
        registry.remove(&expected.session_id).expect("exact remove");
        registry
            .remove(&expected.session_id)
            .expect("missing remove");
        assert!(registry
            .load(&expected.session_id)
            .expect("missing load")
            .is_none());
        assert_eq!(registry.test_load_all_calls(), 0);
        assert!(matches!(
            registry.load_all(),
            Err(SessionRegistryError::UnsupportedFormat)
        ));
        assert_eq!(registry.test_load_all_calls(), 1);
    }
}
