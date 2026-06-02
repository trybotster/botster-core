//! Host-profile package metadata and admission contracts.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Capability, ExtensionKind, PackageManifest};

/// Typed host-profile metadata carried by a provider package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProfileMetadata {
    /// Stable profile identity within the package ecosystem.
    pub profile_id: String,
    /// Botster compatibility requirement for this profile.
    pub compatibility: String,
    /// Host-owned ordering hint used when multiple profiles are available.
    pub precedence: u32,
    /// Provider package names that the host must resolve before enabling this profile.
    #[serde(default)]
    pub required_providers: Vec<String>,
    /// Capabilities the package manifest must declare before this profile can be admitted.
    #[serde(default)]
    pub required_capabilities: Vec<Capability>,
    /// Typed policy sections this profile expects the host to interpret.
    #[serde(default)]
    pub policy_sections: Vec<HostProfilePolicySection>,
}

/// Typed host-profile policy section names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostProfilePolicySection {
    /// Startup composition and ordering policy.
    Startup,
    /// Runtime configuration layering policy.
    Config,
    /// Provider selection and enablement policy.
    Providers,
    /// Host-level capability grant policy.
    Capabilities,
    /// Client admission and trust policy.
    ClientAdmission,
    /// Durable storage ownership and retention policy.
    Persistence,
}

/// Successfully admitted host-profile package contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedHostProfile {
    /// Package name carrying the admitted profile.
    pub package_name: String,
    /// Package version carrying the admitted profile.
    pub package_version: String,
    /// Host-profile metadata admitted from the package manifest.
    pub metadata: HostProfileMetadata,
}

/// Host-profile admission failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HostProfileAdmissionError {
    /// Package did not declare host-profile metadata.
    #[error("package does not declare host-profile metadata")]
    MissingMetadata,
    /// Package declared host-profile metadata but is not a provider.
    #[error("host-profile metadata is only admitted for provider packages")]
    NotProvider,
    /// Host caller did not enable this package/profile.
    #[error("host-profile package is not enabled by the host")]
    Disabled,
    /// Package has no source provenance metadata.
    #[error("host-profile package has no source provenance metadata")]
    MissingSource,
    /// Package has no bootstrap entrypoint.
    #[error("host-profile package has no bootstrap entrypoint")]
    MissingBootstrapEntrypoint,
    /// Package manifest is missing a capability required by the host-profile metadata.
    #[error("host-profile package is missing required capability {0:?}")]
    MissingRequiredCapability(Capability),
}

/// Admit host-profile metadata from a package manifest when all core contract
/// preconditions are satisfied.
pub fn admit_host_profile(
    manifest: &PackageManifest,
    enabled: bool,
) -> Result<AdmittedHostProfile, HostProfileAdmissionError> {
    let metadata = manifest
        .host_profile
        .as_ref()
        .ok_or(HostProfileAdmissionError::MissingMetadata)?;

    if manifest.kind != ExtensionKind::Provider {
        return Err(HostProfileAdmissionError::NotProvider);
    }

    if !enabled {
        return Err(HostProfileAdmissionError::Disabled);
    }

    if manifest.source.is_none() {
        return Err(HostProfileAdmissionError::MissingSource);
    }

    if !manifest
        .entrypoints
        .iter()
        .any(|entrypoint| entrypoint.bootstrap)
    {
        return Err(HostProfileAdmissionError::MissingBootstrapEntrypoint);
    }

    if let Some(capability) = metadata
        .required_capabilities
        .iter()
        .find(|capability| !manifest.capabilities.contains(capability))
    {
        return Err(HostProfileAdmissionError::MissingRequiredCapability(
            capability.clone(),
        ));
    }

    Ok(AdmittedHostProfile {
        package_name: manifest.name.clone(),
        package_version: manifest.version.clone(),
        metadata: metadata.clone(),
    })
}
