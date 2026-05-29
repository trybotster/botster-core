//! Narrow crypto and identity operation contracts.

use serde::{Deserialize, Serialize};

/// Crypto operation a capability holder may request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CryptoOperation {
    /// Generate random bytes.
    RandomBytes,
    /// Hash bytes.
    Hash,
    /// Sign bytes with a non-exportable device key.
    SignWithDeviceKey,
    /// Verify a signature.
    Verify,
    /// Encrypt an envelope for a client.
    EncryptForClient,
    /// Decrypt an envelope from a client.
    DecryptFromClient,
    /// Seal a local secret.
    SealSecret,
    /// Open a local secret.
    OpenSecret,
}

/// Identity operation a capability holder may request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityOperation {
    /// Read the public local device identity.
    ReadPublicDeviceIdentity,
    /// Sign a challenge with the local device identity.
    SignChallenge,
}
