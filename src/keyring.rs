//! Credential-store and non-exportable signing boundaries.

use thiserror::Error;
use zeroize::ZeroizeOnDrop;

/// Opaque credential record stored by an implementation outside core.
#[derive(Clone, PartialEq, Eq, ZeroizeOnDrop)]
pub struct CredentialRecord {
    /// Opaque credential bytes.
    pub value: Vec<u8>,
}

impl CredentialRecord {
    /// Build an opaque credential record.
    pub fn new(value: Vec<u8>) -> Self {
        Self { value }
    }

    /// Borrow the opaque credential bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }
}

/// Boundary trait for credential persistence.
pub trait CredentialStore {
    /// Return an opaque credential by key, or `None` when it is absent.
    fn get(&self, key: &str) -> Result<Option<CredentialRecord>, CredentialStoreError>;

    /// Persist an opaque credential by key.
    fn set(&mut self, key: &str, record: CredentialRecord) -> Result<(), CredentialStoreError>;

    /// Delete an opaque credential by key.
    fn delete(&mut self, key: &str) -> Result<(), CredentialStoreError>;
}

/// Credential-store boundary errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CredentialStoreError {
    /// The requested credential was not found.
    #[error("credential not found")]
    NotFound,
    /// The backing store rejected the operation.
    #[error("credential store rejected operation: {0}")]
    Rejected(String),
}

/// Opaque handle for a signing key that must not export private material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningKeyHandle {
    /// Stable handle identifier.
    pub id: String,
}

impl SigningKeyHandle {
    /// Build a signing key handle.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Signature bytes produced by a non-exportable signer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBytes(pub Vec<u8>);

/// Boundary trait for signing without exporting private key bytes.
pub trait NonExportableSigner {
    /// Sign a message using an opaque key handle.
    fn sign(
        &self,
        handle: &SigningKeyHandle,
        message: &[u8],
    ) -> Result<SignatureBytes, SigningError>;
}

/// Signing boundary errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SigningError {
    /// The requested signing key handle was unknown to the implementation.
    #[error("unknown signing key handle")]
    UnknownHandle,
    /// The signer rejected the operation.
    #[error("signing operation rejected: {0}")]
    Rejected(String),
}
