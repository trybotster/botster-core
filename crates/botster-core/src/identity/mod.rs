//! Crypto, device identity, and credential boundary contracts.

pub mod crypto;
pub mod device;
pub mod keyring;

pub use crypto::{
    decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError, CryptoOperation,
    IdentityOperation,
};
pub use device::{
    device_fingerprint, verify_device_fingerprint, DeviceFingerprint, DevicePublicMetadata,
    PublicSigningKeyBytes,
};
pub use keyring::{
    CredentialRecord, CredentialStore, CredentialStoreError, NonExportableSigner, SignatureBytes,
    SigningError, SigningKeyHandle,
};
