//! Deterministic package dependency and feature-gate resolution contracts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::dependency::{PackageDependency, PackageFeatureGate, PackageRequirement};
use super::manifest::PackageManifest;
use crate::Capability;

/// Host-supplied state used to resolve package dependency contracts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PackageResolutionInput {
    /// Package records known to the host.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<PackageResolutionPackage>,
    /// Provider ids known to the host.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Capability grants known to the host outside a specific dependency package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    /// Auth handles known to the host.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<PackageAuthState>,
    /// Configuration keys known to the host.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub config: Vec<PackageConfigState>,
}

/// Host-supplied package state for dependency resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageResolutionPackage {
    /// Package name.
    pub name: String,
    /// Whether the host has enabled the package.
    pub enabled: bool,
    /// Provider ids this package makes available when enabled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<String>,
    /// Capabilities this package makes available when enabled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
}

/// Host-supplied auth state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageAuthState {
    /// Stable auth key.
    pub key: String,
    /// Whether that key is configured.
    pub status: PackageRequirementStatus,
}

/// Host-supplied configuration state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigState {
    /// Stable configuration key.
    pub key: String,
    /// Whether that key is configured.
    pub status: PackageRequirementStatus,
}

/// Caller-observed requirement status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageRequirementStatus {
    /// The host has configured the requirement.
    Configured,
    /// The host knows the requirement is missing.
    Missing,
}

/// Resolved dependency and feature-gate matrix for a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageResolutionMatrix {
    /// Dependency rows in manifest order.
    pub dependencies: Vec<PackageDependencyResolution>,
    /// Feature rows in manifest order.
    pub features: Vec<PackageFeatureResolution>,
}

/// Resolved dependency row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDependencyResolution {
    /// Dependency id from the manifest.
    pub id: String,
    /// Package name from the manifest.
    pub package: String,
    /// Availability state.
    pub state: PackageResolutionState,
    /// Structured blocked reasons in deterministic order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<PackageBlockedReason>,
}

/// Resolved feature row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFeatureResolution {
    /// Feature id from the manifest.
    pub id: String,
    /// Availability state.
    pub state: PackageResolutionState,
    /// Structured blocked reasons in deterministic order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_reasons: Vec<PackageBlockedReason>,
}

/// Resolved availability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageResolutionState {
    /// All declared requirements were satisfied by caller-supplied state.
    Available,
    /// At least one declared requirement was not satisfied.
    Blocked,
}

/// Structured reason explaining why a dependency or feature is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PackageBlockedReason {
    /// Required package was not present in caller-supplied state.
    MissingPackage {
        /// Package name.
        package: String,
    },
    /// Required package was present but disabled.
    DisabledPackage {
        /// Package name.
        package: String,
    },
    /// Required provider was not present in caller-supplied state.
    MissingProvider {
        /// Provider id.
        provider: String,
    },
    /// Required capability was not present in caller-supplied state.
    MissingCapability {
        /// Required package name, when the requirement belongs to a package dependency.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<String>,
        /// Required capability.
        capability: Capability,
    },
    /// Required auth handle is missing.
    MissingAuth {
        /// Auth key.
        key: String,
    },
    /// Required configuration key is missing.
    MissingConfig {
        /// Configuration key.
        key: String,
    },
}

/// Resolve a package manifest's dependency and feature-gate declarations using
/// only caller-supplied state.
pub fn resolve_package_dependencies(
    manifest: &PackageManifest,
    input: &PackageResolutionInput,
) -> PackageResolutionMatrix {
    let context = ResolutionContext::new(input);

    let dependencies = manifest
        .dependencies
        .iter()
        .map(|dependency| resolve_dependency(dependency, &context))
        .collect::<Vec<_>>();

    let dependency_reasons = dependencies
        .iter()
        .map(|resolution| (resolution.id.clone(), resolution.blocked_reasons.clone()))
        .collect::<BTreeMap<_, _>>();

    let features = manifest
        .features
        .iter()
        .map(|feature| resolve_feature(feature, &dependency_reasons, &context))
        .collect();

    PackageResolutionMatrix {
        dependencies,
        features,
    }
}

struct ResolutionContext {
    packages: BTreeMap<String, PackageResolutionPackage>,
    providers: BTreeSet<String>,
    capabilities: BTreeSet<Capability>,
    configured_auth: BTreeSet<String>,
    configured_config: BTreeSet<String>,
}

impl ResolutionContext {
    fn new(input: &PackageResolutionInput) -> Self {
        let packages = input
            .packages
            .iter()
            .map(|package| (package.name.clone(), package.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut providers = input.providers.iter().cloned().collect::<BTreeSet<_>>();
        for package in input.packages.iter().filter(|package| package.enabled) {
            providers.extend(package.providers.iter().cloned());
        }
        let mut capabilities = input.capabilities.iter().cloned().collect::<BTreeSet<_>>();
        for package in input.packages.iter().filter(|package| package.enabled) {
            capabilities.extend(package.capabilities.iter().cloned());
        }
        let configured_auth = input
            .auth
            .iter()
            .filter(|auth| auth.status == PackageRequirementStatus::Configured)
            .map(|auth| auth.key.clone())
            .collect();
        let configured_config = input
            .config
            .iter()
            .filter(|config| config.status == PackageRequirementStatus::Configured)
            .map(|config| config.key.clone())
            .collect();

        Self {
            packages,
            providers,
            capabilities,
            configured_auth,
            configured_config,
        }
    }
}

fn resolve_dependency(
    dependency: &PackageDependency,
    context: &ResolutionContext,
) -> PackageDependencyResolution {
    let mut blocked_reasons = Vec::new();
    let package = context.packages.get(&dependency.package);

    match package {
        None => blocked_reasons.push(PackageBlockedReason::MissingPackage {
            package: dependency.package.clone(),
        }),
        Some(package) if !package.enabled => {
            blocked_reasons.push(PackageBlockedReason::DisabledPackage {
                package: dependency.package.clone(),
            });
        }
        Some(package) => append_requirement_reasons(
            &dependency.requirements,
            Some(package),
            Some(&dependency.package),
            context,
            &mut blocked_reasons,
        ),
    }

    PackageDependencyResolution {
        id: dependency.id.clone(),
        package: dependency.package.clone(),
        state: resolution_state(&blocked_reasons),
        blocked_reasons,
    }
}

fn resolve_feature(
    feature: &PackageFeatureGate,
    dependency_reasons: &BTreeMap<String, Vec<PackageBlockedReason>>,
    context: &ResolutionContext,
) -> PackageFeatureResolution {
    let mut blocked_reasons = Vec::new();

    for dependency_id in &feature.dependencies {
        if let Some(reasons) = dependency_reasons.get(dependency_id) {
            blocked_reasons.extend(reasons.iter().cloned());
        }
    }

    append_requirement_reasons(
        &feature.requirements,
        None,
        None,
        context,
        &mut blocked_reasons,
    );

    PackageFeatureResolution {
        id: feature.id.clone(),
        state: resolution_state(&blocked_reasons),
        blocked_reasons,
    }
}

fn append_requirement_reasons(
    requirements: &[PackageRequirement],
    package: Option<&PackageResolutionPackage>,
    package_name: Option<&String>,
    context: &ResolutionContext,
    blocked_reasons: &mut Vec<PackageBlockedReason>,
) {
    for requirement in requirements {
        match requirement {
            PackageRequirement::Provider { provider } => {
                let provided_by_package = package
                    .map(|package| package.providers.contains(provider))
                    .unwrap_or(false);
                if !provided_by_package && !context.providers.contains(provider) {
                    blocked_reasons.push(PackageBlockedReason::MissingProvider {
                        provider: provider.clone(),
                    });
                }
            }
            PackageRequirement::Capability { capability } => {
                let provided_by_package = package
                    .map(|package| package.capabilities.contains(capability))
                    .unwrap_or(false);
                if !provided_by_package && !context.capabilities.contains(capability) {
                    blocked_reasons.push(PackageBlockedReason::MissingCapability {
                        package: package_name.cloned(),
                        capability: capability.clone(),
                    });
                }
            }
            PackageRequirement::Auth { key } => {
                if !context.configured_auth.contains(key) {
                    blocked_reasons.push(PackageBlockedReason::MissingAuth { key: key.clone() });
                }
            }
            PackageRequirement::Config { key } => {
                if !context.configured_config.contains(key) {
                    blocked_reasons.push(PackageBlockedReason::MissingConfig { key: key.clone() });
                }
            }
        }
    }
}

fn resolution_state(blocked_reasons: &[PackageBlockedReason]) -> PackageResolutionState {
    if blocked_reasons.is_empty() {
        PackageResolutionState::Available
    } else {
        PackageResolutionState::Blocked
    }
}
