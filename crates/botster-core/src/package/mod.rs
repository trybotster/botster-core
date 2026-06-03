//! Package, extension, and capability contracts.

pub mod capability;
pub mod extension;
pub mod host_profile;
pub mod manifest;

pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use host_profile::{
    admit_host_profile, AdmittedHostProfile, HostProfileAdmissionError,
    HostProfileCompatibilityField, HostProfileMetadata, HostProfilePolicySection,
};
pub use manifest::{PackageManifest, PackageSource};
