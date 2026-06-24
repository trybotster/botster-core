//! Package, extension, and capability contracts.

pub mod capability;
pub mod configuration;
pub mod extension;
pub mod host_profile;
pub mod manifest;
pub mod surface;

pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use configuration::{
    PackageConfigurationField, PackageConfigurationFieldType, PackageConfigurationGroup,
    PackageConfigurationOption, PackageConfigurationSchema, PackageConfigurationSecretValue,
    PackageConfigurationValidationHints, PackageConfigurationValue,
};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use host_profile::{
    admit_host_profile, AdmittedHostProfile, HostProfileAdmissionError,
    HostProfileCompatibilityField, HostProfileMetadata, HostProfilePolicySection,
};
pub use manifest::{PackageManifest, PackageSource};
pub use surface::{PackageSurfaceDescriptor, PackageSurfaceKind, PackageSurfaceOperation};
