//! Reusable Botster runtime contracts and transport-neutral primitives.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients. It
//! defines stable data shapes and low-level contracts, while `botster-hub`
//! owns Botster policy and orchestration.

pub mod boundary;
pub mod capability;
pub mod client;
pub mod crypto;
pub mod device;
pub mod entity;
pub mod extension;
pub mod keyring;
pub mod package;
pub mod session;
pub mod transport;
pub mod ui;

pub use boundary::{Layer, LayerResponsibility};
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use crypto::{
    decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError, CryptoOperation,
    IdentityOperation,
};
pub use device::{
    device_fingerprint, verify_device_fingerprint, DeviceFingerprint, DevicePublicMetadata,
    PublicSigningKeyBytes,
};
pub use entity::{EntityFrame, EntityId, EntityKind};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use keyring::{
    CredentialRecord, CredentialStore, CredentialStoreError, NonExportableSigner, SignatureBytes,
    SigningError, SigningKeyHandle,
};
pub use package::{PackageManifest, PackageSource};
pub use session::{RequestId, SessionId, SubscriptionId};
pub use transport::{TransportEgress, TransportIngress};
