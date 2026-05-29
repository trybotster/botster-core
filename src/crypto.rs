//! Narrow crypto and identity operation contracts.

use aes_gcm::aead::{rand_core::RngCore, Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::ZeroizeOnDrop;

const AES_GCM_KEY_LEN: usize = 32;
const AES_GCM_NONCE_LEN: usize = 12;

/// Serializable AES-GCM envelope shared across Botster hosts and clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AesGcmEnvelope {
    /// Standard base64 encoded 96-bit nonce.
    pub nonce: String,
    /// Standard base64 encoded authenticated ciphertext.
    pub ciphertext: String,
    /// Caller-owned envelope version preserved across serialization.
    pub version: u8,
}

/// AES-256-GCM key bytes.
#[derive(Clone, ZeroizeOnDrop)]
pub struct AesGcmKey([u8; AES_GCM_KEY_LEN]);

impl AesGcmKey {
    /// Build a key from exactly 32 bytes.
    pub fn new(bytes: [u8; AES_GCM_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Build a key from a byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let key: [u8; AES_GCM_KEY_LEN] =
            bytes
                .try_into()
                .map_err(|_| CryptoError::InvalidKeyLength {
                    expected: AES_GCM_KEY_LEN,
                    actual: bytes.len(),
                })?;

        Ok(Self(key))
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new_from_slice(&self.0).expect("AesGcmKey always stores exactly 32 bytes")
    }
}

/// Crypto utility errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// AES-256-GCM keys must be exactly 32 bytes.
    #[error("invalid AES-GCM key length: expected {expected} bytes, got {actual}")]
    InvalidKeyLength {
        /// Required key length.
        expected: usize,
        /// Supplied key length.
        actual: usize,
    },
    /// AES-GCM nonces must decode to exactly 12 bytes.
    #[error("invalid AES-GCM nonce length: expected {expected} bytes, got {actual}")]
    InvalidNonceLength {
        /// Required nonce length.
        expected: usize,
        /// Supplied nonce length.
        actual: usize,
    },
    /// Envelope base64 decoding failed.
    #[error("failed to decode {field} base64")]
    DecodeFailed {
        /// Envelope field that failed to decode.
        field: &'static str,
    },
    /// Encryption failed.
    #[error("AES-GCM encryption failed")]
    EncryptFailed,
    /// Authenticated decryption failed.
    #[error("AES-GCM decryption failed")]
    DecryptFailed,
}

/// Encrypt plaintext into an AES-GCM envelope with a fresh internal nonce.
///
/// AES-GCM nonces must never be reused with the same key. This function keeps
/// nonce generation inside core so callers cannot accidentally supply one.
pub fn encrypt_aes_gcm(
    key: &AesGcmKey,
    plaintext: &[u8],
    version: u8,
) -> Result<AesGcmEnvelope, CryptoError> {
    let mut nonce_bytes = [0_u8; AES_GCM_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let ciphertext = key
        .cipher()
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| CryptoError::EncryptFailed)?;

    Ok(AesGcmEnvelope {
        nonce: STANDARD.encode(nonce_bytes),
        ciphertext: STANDARD.encode(ciphertext),
        version,
    })
}

/// Decrypt an AES-GCM envelope and authenticate its ciphertext.
pub fn decrypt_aes_gcm(key: &AesGcmKey, envelope: &AesGcmEnvelope) -> Result<Vec<u8>, CryptoError> {
    let nonce = STANDARD
        .decode(&envelope.nonce)
        .map_err(|_| CryptoError::DecodeFailed { field: "nonce" })?;

    if nonce.len() != AES_GCM_NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength {
            expected: AES_GCM_NONCE_LEN,
            actual: nonce.len(),
        });
    }

    let ciphertext =
        STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|_| CryptoError::DecodeFailed {
                field: "ciphertext",
            })?;

    key.cipher()
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptFailed)
}

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
