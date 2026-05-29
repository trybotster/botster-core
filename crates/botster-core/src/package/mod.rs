//! Package, extension, and capability contracts.

pub mod capability;
pub mod extension;
pub mod manifest;

pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use manifest::{PackageManifest, PackageSource};
