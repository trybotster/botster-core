//! Explicit provenance for the real `botster-session-worker` under test.
//!
//! Tests never build the worker. A prebuild step (`script/prebuild-worker`
//! in this repository, or the Hub release-artifact script) writes the
//! executable and a manifest; the test process receives both through the
//! environment and refuses to run when they disagree.

use std::collections::BTreeMap;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::diagnostics::StepFailure;

/// Environment variable naming the worker executable.
pub const WORKER_BIN_ENV: &str = "BOTSTER_SESSION_WORKER_BIN";
/// Environment variable naming the candidate manifest.
pub const MANIFEST_ENV: &str = "BOTSTER_CANDIDATE_MANIFEST";
/// Artifact name of the worker inside the manifest.
pub const WORKER_ARTIFACT_NAME: &str = "botster-session-worker";

/// A verified worker executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerBinary {
    /// Executable path from the environment.
    pub path: PathBuf,
    /// Hex SHA-256 of the executable, equal to the manifest entry.
    pub sha256: String,
    /// Size in bytes, equal to the manifest entry.
    pub size: u64,
    /// Source revisions recorded by the prebuild step, by repository key.
    pub source_revisions: BTreeMap<String, String>,
}

impl WorkerBinary {
    /// Read `BOTSTER_SESSION_WORKER_BIN` and `BOTSTER_CANDIDATE_MANIFEST`,
    /// hash the executable, and verify it against the manifest's
    /// `botster-session-worker` artifact entry.
    #[expect(
        clippy::result_large_err,
        reason = "Provenance failures deliberately retain rich harness diagnostics by value"
    )]
    pub fn from_env() -> Result<Self, StepFailure> {
        let path = PathBuf::from(env_var(WORKER_BIN_ENV)?);
        let manifest_path = PathBuf::from(env_var(MANIFEST_ENV)?);
        let manifest_text = std::fs::read_to_string(&manifest_path).map_err(|error| {
            provenance(format!(
                "cannot read manifest {}: {error}",
                manifest_path.display()
            ))
        })?;
        let manifest: Manifest = serde_json::from_str(&manifest_text).map_err(|error| {
            provenance(format!(
                "manifest {} is not the expected shape: {error}",
                manifest_path.display()
            ))
        })?;
        let mut matching_entries = manifest
            .artifacts
            .iter()
            .filter(|artifact| artifact.name == WORKER_ARTIFACT_NAME);
        let entry = matching_entries.next().ok_or_else(|| {
            provenance(format!(
                "manifest {} has no artifact named {WORKER_ARTIFACT_NAME}",
                manifest_path.display()
            ))
        })?;
        if matching_entries.next().is_some() {
            return Err(provenance(format!(
                "manifest {} has more than one artifact named {WORKER_ARTIFACT_NAME}",
                manifest_path.display()
            )));
        }
        if manifest
            .source_revisions
            .get("botster_core")
            .is_none_or(|revision| revision.trim().is_empty())
        {
            return Err(provenance(format!(
                "manifest {} has no non-empty botster_core source revision",
                manifest_path.display()
            )));
        }
        let bytes = std::fs::read(&path).map_err(|error| {
            provenance(format!("cannot read worker {}: {error}", path.display()))
        })?;
        let size = bytes.len() as u64;
        if size != entry.size {
            return Err(provenance(format!(
                "worker size {size} differs from manifest size {}",
                entry.size
            )));
        }
        let digest = Sha256::digest(&bytes);
        let sha256 = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        if !sha256.eq_ignore_ascii_case(&entry.sha256) {
            return Err(provenance(format!(
                "worker sha256 {sha256} differs from manifest sha256 {}",
                entry.sha256
            )));
        }
        Ok(Self {
            path,
            sha256,
            size,
            source_revisions: manifest.source_revisions,
        })
    }
}

#[expect(
    clippy::result_large_err,
    reason = "Provenance failures deliberately retain rich harness diagnostics by value"
)]
fn env_var(name: &str) -> Result<String, StepFailure> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            provenance(format!(
                "{name} is not set; run script/prebuild-worker and export its variables"
            ))
        })
}

fn provenance(cause: String) -> StepFailure {
    StepFailure::new("core", "worker_provenance", cause)
}

#[derive(serde::Deserialize)]
struct Manifest {
    source_revisions: BTreeMap<String, String>,
    artifacts: Vec<Artifact>,
}

#[derive(serde::Deserialize)]
struct Artifact {
    name: String,
    size: u64,
    sha256: String,
}
