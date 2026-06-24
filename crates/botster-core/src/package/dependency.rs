//! Package dependency and feature-gate declaration contracts.

use serde::{Deserialize, Serialize};

use crate::Capability;

/// A package dependency declared by an installable package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDependency {
    /// Stable dependency id within this manifest.
    pub id: String,
    /// Package name the host must resolve.
    pub package: String,
    /// Whether this dependency is mandatory for the package or optional for a feature.
    pub kind: PackageDependencyKind,
    /// Optional feature id this dependency participates in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// Provider, capability, auth, or config requirements tied to this dependency.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<PackageRequirement>,
}

/// Dependency strength declared by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageDependencyKind {
    /// Required before the package can be considered available.
    Required,
    /// Optional dependency that may block only a feature or integration.
    Optional,
}

/// A named feature gate declared by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFeatureGate {
    /// Stable feature id within this manifest.
    pub id: String,
    /// Human-readable feature label for host/client presentation.
    pub label: String,
    /// Optional feature help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Dependency ids this feature requires.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// Provider, capability, auth, or config requirements tied directly to this feature.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<PackageRequirement>,
}

/// Policy-free requirement shape supplied by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PackageRequirement {
    /// A named provider must be available to the host.
    Provider {
        /// Provider id interpreted by the host.
        provider: String,
    },
    /// A capability must be available on the dependency package or host.
    Capability {
        /// Required capability.
        capability: Capability,
    },
    /// An auth handle must be configured by the host.
    Auth {
        /// Stable auth key. Raw credentials are never carried here.
        key: String,
    },
    /// A configuration key must be configured by the host.
    Config {
        /// Stable configuration key. Raw values are never carried here.
        key: String,
    },
}
