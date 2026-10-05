//! The prebuilt binaries of the real-process tier and their provenance (plan 4.2 and section 5).
//!
//! `cargo xtask prebuild-worker` builds `botster-worker`, `botster-conformance-probe` and `botster-test-anchor` into
//! `target/candidate/` and writes
//! `manifest.json` last, with the sha256 of each binary. A test never builds a binary (testing rule 7, which also avoids the
//! first-launch stall of macOS inside a test). [`Candidate::locate`] reads the manifest, hashes each binary and refuses to
//! return one that is missing, unlisted or different from its manifest entry. The idea is the old `real_worker.rs` provenance
//! check (prior art at `72b2e335`); the code is new for the manifest of this workspace.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};

/// The worker binary (HC RT-3 names it `botster-worker`).
pub const WORKER: &str = "botster-worker";
/// The program of the real-process tier, from botster-contracts.
pub const PROBE: &str = "botster-conformance-probe";
/// The guarded launch wrapper of the real-process tier (`crate::anchor`).
pub const ANCHOR: &str = "botster-test-anchor";

/// Why the candidate directory cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateError {
    /// The manifest is not there or not readable: the prebuild step did not run.
    NoManifest { path: PathBuf, why: String },
    /// The manifest is not the shape that xtask writes.
    BadManifest { why: String },
    /// A binary has no entry, or more than one.
    Entry { name: String, entries: usize },
    /// The binary cannot be read.
    Unreadable { name: String, why: String },
    /// The binary is not the one that the manifest lists.
    Mismatch {
        name: String,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for CandidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hint = "run `cargo xtask prebuild-worker`";
        match self {
            CandidateError::NoManifest { path, why } => {
                write!(f, "no manifest at {} ({why}); {hint}", path.display())
            }
            CandidateError::BadManifest { why } => {
                write!(f, "the manifest is not valid: {why}; {hint}")
            }
            CandidateError::Entry { name, entries } => {
                write!(
                    f,
                    "the manifest has {entries} entries named {name}, not one; {hint}"
                )
            }
            CandidateError::Unreadable { name, why } => {
                write!(f, "cannot read {name}: {why}; {hint}")
            }
            CandidateError::Mismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "{name} has sha256 {actual}, the manifest says {expected}; {hint}"
            ),
        }
    }
}

impl std::error::Error for CandidateError {}

/// The verified binaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub worker: PathBuf,
    pub probe: PathBuf,
    pub anchor: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Candidate {
    /// `target/candidate`, found from the running test binary (`target/<profile>/deps/<test>`), so it holds for any target
    /// directory.
    pub fn beside_test_binary() -> io::Result<PathBuf> {
        let exe = std::env::current_exe()?;
        exe.ancestors()
            .nth(3)
            .map(|target| target.join("candidate"))
            .ok_or_else(|| io::Error::other(format!("{} is not in target/<profile>/deps", exe.display())))
    }

    /// Verifies the binaries of `dir` (`target/candidate`) against its `manifest.json`.
    pub fn locate(dir: &Path) -> Result<Candidate, CandidateError> {
        let path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&path).map_err(|e| CandidateError::NoManifest {
            path: path.clone(),
            why: e.to_string(),
        })?;
        let manifest: Value = serde_json::from_str(&text)
            .map_err(|e| CandidateError::BadManifest { why: e.to_string() })?;
        let binaries =
            manifest["binaries"]
                .as_array()
                .ok_or_else(|| CandidateError::BadManifest {
                    why: "no `binaries` list".into(),
                })?;
        let verify = |name: &str| -> Result<PathBuf, CandidateError> {
            let entries: Vec<&Value> = binaries.iter().filter(|b| b["name"] == name).collect();
            let [entry] = entries[..] else {
                return Err(CandidateError::Entry {
                    name: name.to_string(),
                    entries: entries.len(),
                });
            };
            let expected = entry["sha256"]
                .as_str()
                .ok_or_else(|| CandidateError::BadManifest {
                    why: format!("{name} has no sha256"),
                })?;
            let file = dir.join(entry["path"].as_str().unwrap_or(name));
            let bytes = std::fs::read(&file).map_err(|e| CandidateError::Unreadable {
                name: name.to_string(),
                why: e.to_string(),
            })?;
            let actual = sha256_hex(&bytes);
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(CandidateError::Mismatch {
                    name: name.to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
            Ok(file)
        };
        Ok(Candidate {
            worker: verify(WORKER)?,
            probe: verify(PROBE)?,
            anchor: verify(ANCHOR)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn candidate_dir(manifest: impl FnOnce(&str, &str) -> Value) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(WORKER), b"worker bytes").unwrap();
        std::fs::write(dir.path().join(PROBE), b"probe bytes").unwrap();
        std::fs::write(dir.path().join(ANCHOR), b"anchor bytes").unwrap();
        let manifest = manifest(&sha256_hex(b"worker bytes"), &sha256_hex(b"probe bytes"));
        std::fs::write(dir.path().join("manifest.json"), manifest.to_string()).unwrap();
        dir
    }

    fn anchor_entry() -> Value {
        json!({"name": ANCHOR, "path": ANCHOR, "sha256": sha256_hex(b"anchor bytes")})
    }

    fn good(worker: &str, probe: &str) -> Value {
        json!({"binaries": [
            {"name": WORKER, "path": WORKER, "sha256": worker},
            {"name": PROBE, "path": PROBE, "sha256": probe},
            anchor_entry(),
        ]})
    }

    #[test]
    fn verified_binaries_are_returned() {
        let dir = candidate_dir(good);
        let found = Candidate::locate(dir.path()).unwrap();
        assert_eq!(found.worker, dir.path().join(WORKER));
        assert_eq!(found.probe, dir.path().join(PROBE));
        assert_eq!(found.anchor, dir.path().join(ANCHOR));
    }

    #[test]
    fn no_manifest_means_the_prebuild_did_not_run() {
        let dir = tempfile::tempdir().unwrap();
        let error = Candidate::locate(dir.path()).unwrap_err();
        assert!(matches!(error, CandidateError::NoManifest { .. }));
        assert!(error.to_string().contains("cargo xtask prebuild-worker"));
    }

    #[test]
    fn a_binary_that_differs_from_its_entry_is_refused() {
        for binary in [WORKER, PROBE, ANCHOR] {
            let dir = candidate_dir(good);
            std::fs::write(dir.path().join(binary), b"rebuilt later").unwrap();
            let error = Candidate::locate(dir.path()).unwrap_err();
            assert!(
                matches!(&error, CandidateError::Mismatch { name, .. } if name == binary),
                "{error}"
            );
        }
        // The comparison ignores the case of the hex digits.
        let upper = candidate_dir(|w, p| good(&w.to_uppercase(), &p.to_uppercase()));
        assert!(Candidate::locate(upper.path()).is_ok());
    }

    #[test]
    fn a_missing_duplicate_or_unreadable_entry_is_refused() {
        let missing = candidate_dir(
            |_, probe| json!({"binaries": [{"name": PROBE, "path": PROBE, "sha256": probe}, anchor_entry()]}),
        );
        assert_eq!(
            Candidate::locate(missing.path()).unwrap_err(),
            CandidateError::Entry {
                name: WORKER.into(),
                entries: 0
            }
        );
        let twice = candidate_dir(|w, p| {
            json!({"binaries": [
                {"name": WORKER, "path": WORKER, "sha256": w},
                {"name": WORKER, "path": WORKER, "sha256": w},
                {"name": PROBE, "path": PROBE, "sha256": p},
                anchor_entry(),
            ]})
        });
        assert_eq!(
            Candidate::locate(twice.path()).unwrap_err(),
            CandidateError::Entry {
                name: WORKER.into(),
                entries: 2
            }
        );
        let gone = candidate_dir(good);
        std::fs::remove_file(gone.path().join(PROBE)).unwrap();
        assert!(matches!(
            Candidate::locate(gone.path()),
            Err(CandidateError::Unreadable { .. })
        ));
    }

    #[test]
    fn a_manifest_of_another_shape_is_refused() {
        for manifest in [
            json!({}),
            json!({"binaries": "x"}),
            json!({"binaries": [{"name": WORKER, "path": WORKER}]}),
        ] {
            let dir = candidate_dir(|_, _| manifest.clone());
            assert!(
                matches!(
                    Candidate::locate(dir.path()),
                    Err(CandidateError::BadManifest { .. })
                ),
                "{manifest}"
            );
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("manifest.json"), "not json").unwrap();
        assert!(matches!(
            Candidate::locate(dir.path()),
            Err(CandidateError::BadManifest { .. })
        ));
    }
}
