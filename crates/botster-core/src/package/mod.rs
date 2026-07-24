//! Package, extension, and capability contracts.

pub mod capability;
pub mod configuration;
pub mod dependency;
pub mod extension;
pub mod host_profile;
pub mod manifest;
pub mod navigation;
pub mod resolution;
pub mod runnable_entrypoint;
pub mod surface;

pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use configuration::{
    PackageConfigurationField, PackageConfigurationFieldType, PackageConfigurationGroup,
    PackageConfigurationOption, PackageConfigurationSchema, PackageConfigurationSecretValue,
    PackageConfigurationValidationHints, PackageConfigurationValue,
};
pub use dependency::{
    PackageDependency, PackageDependencyKind, PackageFeatureGate, PackageRequirement,
};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use host_profile::{
    admit_host_profile, AdmittedHostProfile, HostProfileAdmissionError,
    HostProfileCompatibilityField, HostProfileMetadata, HostProfilePolicySection,
};
pub use manifest::{PackageManifest, PackageSource};
pub use navigation::{PackageNavigationEntry, PackageNavigationTarget};
pub use resolution::{
    resolve_package_dependencies, PackageAuthState, PackageBlockedReason, PackageConfigState,
    PackageDependencyResolution, PackageFeatureResolution, PackageRequirementStatus,
    PackageResolutionInput, PackageResolutionMatrix, PackageResolutionPackage,
    PackageResolutionState,
};
pub use runnable_entrypoint::{
    validate_package_runnable_entrypoints, RunnableEntrypoint,
    RunnableEntrypointEnvironmentRequirement, RunnableEntrypointHubConnection,
    RunnableEntrypointHubConnectionTransport, RunnableEntrypointHubConnectionValidationError,
    RunnableEntrypointInjection, RunnableEntrypointInjectionKind,
    RunnableEntrypointInjectionTarget, RunnableEntrypointKind, RunnableEntrypointLaunchMode,
    RunnableEntrypointLaunchResult, RunnableEntrypointProcessState, RunnableEntrypointReadiness,
    RunnableEntrypointResultField, RunnableEntrypointValidationError,
    RunnableEntrypointWorkingDirectory,
};
pub use surface::{PackageSurfaceDescriptor, PackageSurfaceKind, PackageSurfaceOperation};
