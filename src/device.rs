//! Public device identity metadata and fingerprint helpers.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

const FINGERPRINT_BYTES: usize = 8;

/// Public signing or verifying key bytes for a local device identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicSigningKeyBytes(pub Vec<u8>);

/// Stable public device fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceFingerprint(pub String);

impl fmt::Display for DeviceFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Public device metadata safe to serialize and send across process boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePublicMetadata {
    /// Operator-facing device name or label.
    pub name: Option<String>,
    /// Public signing key bytes used for verification.
    pub verifying_key: PublicSigningKeyBytes,
    /// Stable fingerprint derived only from public key material.
    pub fingerprint: DeviceFingerprint,
}

impl DevicePublicMetadata {
    /// Build public metadata from display name and public verifying key bytes.
    pub fn new(name: Option<String>, verifying_key: PublicSigningKeyBytes) -> Self {
        let fingerprint = device_fingerprint(&verifying_key.0);

        Self {
            name,
            verifying_key,
            fingerprint,
        }
    }
}

/// Derive the stable device fingerprint from public key bytes.
pub fn device_fingerprint(public_key: &[u8]) -> DeviceFingerprint {
    let digest = Sha256::digest(public_key);
    let parts = digest[..FINGERPRINT_BYTES]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>();

    DeviceFingerprint(parts.join(":"))
}

/// Verify that public key bytes match the expected stable fingerprint.
pub fn verify_device_fingerprint(public_key: &[u8], expected: &DeviceFingerprint) -> bool {
    device_fingerprint(public_key) == *expected
}
