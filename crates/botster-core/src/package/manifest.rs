//! Package manifest contracts.

use serde::{Deserialize, Serialize};

use super::configuration::PackageConfigurationSchema;
use super::dependency::{PackageDependency, PackageFeatureGate};
use super::host_profile::HostProfileMetadata;
use super::navigation::PackageNavigationEntry;
use super::runnable_entrypoint::RunnableEntrypoint;
use super::surface::PackageSurfaceDescriptor;
use crate::capability::Capability;
use crate::extension::{ExtensionEntrypoint, ExtensionKind};

/// Source location for an installable package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PackageSource {
    /// Git repository source.
    Git {
        /// Repository URL.
        repo: String,
        /// Branch, tag, or revision.
        reference: String,
    },
    /// Local filesystem source.
    Path {
        /// Local package path.
        path: String,
    },
}

/// Installable Botster package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Extension kind.
    pub kind: ExtensionKind,
    /// Compatible Botster version requirement.
    pub botster: String,
    /// Package source.
    pub source: Option<PackageSource>,
    /// Requested capabilities.
    pub capabilities: Vec<Capability>,
    /// Entrypoints supplied by this package.
    pub entrypoints: Vec<ExtensionEntrypoint>,
    /// Package dependency declarations inspected by hosts without running plugin code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<PackageDependency>,
    /// Named feature gates resolved from dependency and requirement state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<PackageFeatureGate>,
    /// Host-profile metadata for privileged provider packages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_profile: Option<HostProfileMetadata>,
    /// Configuration metadata clients and hubs can inspect without running plugin code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration: Option<PackageConfigurationSchema>,
    /// Runnable app/process descriptors clients and hubs can inspect without running plugin code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runnable_entrypoints: Vec<RunnableEntrypoint>,
    /// UI surface descriptors clients and hubs can inspect without running plugin code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<PackageSurfaceDescriptor>,
    /// Optional package-authored navigation intent. Hosts own admission and presentation policy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub navigation: Vec<PackageNavigationEntry>,
}
