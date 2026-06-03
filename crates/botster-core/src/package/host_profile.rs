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

/// Compatibility requirement field checked during host-profile admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostProfileCompatibilityField {
    /// `PackageManifest.botster`.
    Package,
    /// `HostProfileMetadata.compatibility`.
    Profile,
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
    /// Package has a bootstrap entrypoint with a blank path.
    #[error("host-profile package has a blank bootstrap entrypoint path")]
    BlankBootstrapEntrypoint,
    /// Host-profile metadata has a blank profile id.
    #[error("host-profile metadata has a blank profile id")]
    BlankProfileId,
    /// Host-profile metadata has a blank compatibility requirement.
    #[error("host-profile metadata has a blank compatibility requirement")]
    BlankProfileCompatibility,
    /// Host-profile metadata has a blank required provider name.
    #[error("host-profile metadata has a blank required provider name")]
    BlankRequiredProvider,
    /// Package manifest is missing a capability required by the host-profile metadata.
    #[error("host-profile package is missing required capability {0:?}")]
    MissingRequiredCapability(Capability),
    /// Caller supplied an invalid host Botster point version.
    #[error("host Botster version is invalid: {0}")]
    InvalidHostBotsterVersion(String),
    /// Compatibility requirement uses unsupported syntax.
    #[error("unsupported compatibility requirement for {field:?}: {requirement}")]
    UnsupportedCompatibilityRequirement {
        /// Field carrying the unsupported requirement.
        field: HostProfileCompatibilityField,
        /// Unsupported requirement string.
        requirement: String,
    },
    /// Compatibility requirement version is malformed.
    #[error("malformed compatibility requirement for {field:?}: {requirement}")]
    MalformedCompatibilityRequirement {
        /// Field carrying the malformed requirement.
        field: HostProfileCompatibilityField,
        /// Malformed requirement string.
        requirement: String,
    },
    /// Compatibility requirement does not admit the host Botster version.
    #[error(
        "host Botster version {host_version} does not satisfy {field:?} requirement {requirement}"
    )]
    IncompatibleBotsterVersion {
        /// Field carrying the incompatible requirement.
        field: HostProfileCompatibilityField,
        /// Incompatible requirement string.
        requirement: String,
        /// Host Botster version supplied by the caller.
        host_version: String,
    },
}

/// Admit host-profile metadata from a package manifest when all core contract
/// preconditions are satisfied.
pub fn admit_host_profile(
    manifest: &PackageManifest,
    enabled: bool,
    host_botster_version: &str,
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

    if metadata.profile_id.trim().is_empty() {
        return Err(HostProfileAdmissionError::BlankProfileId);
    }

    if metadata.compatibility.trim().is_empty() {
        return Err(HostProfileAdmissionError::BlankProfileCompatibility);
    }

    if metadata
        .required_providers
        .iter()
        .any(|provider| provider.trim().is_empty())
    {
        return Err(HostProfileAdmissionError::BlankRequiredProvider);
    }

    let bootstrap_entrypoint = manifest
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.bootstrap)
        .ok_or(HostProfileAdmissionError::MissingBootstrapEntrypoint)?;

    if bootstrap_entrypoint.path.trim().is_empty() {
        return Err(HostProfileAdmissionError::BlankBootstrapEntrypoint);
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

    let host_version = Version::parse_host(host_botster_version)?;
    require_compatible(
        HostProfileCompatibilityField::Package,
        &manifest.botster,
        host_botster_version,
        host_version,
    )?;
    require_compatible(
        HostProfileCompatibilityField::Profile,
        &metadata.compatibility,
        host_botster_version,
        host_version,
    )?;

    Ok(AdmittedHostProfile {
        package_name: manifest.name.clone(),
        package_version: manifest.version.clone(),
        metadata: metadata.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse_host(version: &str) -> Result<Self, HostProfileAdmissionError> {
        Self::parse(version).ok_or_else(|| {
            HostProfileAdmissionError::InvalidHostBotsterVersion(version.to_string())
        })
    }

    fn parse(version: &str) -> Option<Self> {
        let mut components = version.split('.');
        let major = parse_version_component(components.next()?)?;
        let minor = parse_version_component(components.next()?)?;
        let patch = parse_version_component(components.next()?)?;

        if components.next().is_some() {
            return None;
        }

        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

fn parse_version_component(component: &str) -> Option<u64> {
    if component.is_empty() || !component.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    component.parse().ok()
}

fn require_compatible(
    field: HostProfileCompatibilityField,
    requirement: &str,
    host_version_string: &str,
    host_version: Version,
) -> Result<(), HostProfileAdmissionError> {
    let requirement = requirement.trim();

    if let Some(version) = requirement.strip_prefix(">=") {
        let minimum = Version::parse(version).ok_or_else(|| {
            HostProfileAdmissionError::MalformedCompatibilityRequirement {
                field,
                requirement: requirement.to_string(),
            }
        })?;

        if host_version >= minimum {
            return Ok(());
        }

        return Err(HostProfileAdmissionError::IncompatibleBotsterVersion {
            field,
            requirement: requirement.to_string(),
            host_version: host_version_string.to_string(),
        });
    }

    if requirement
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        let exact = Version::parse(requirement).ok_or_else(|| {
            HostProfileAdmissionError::MalformedCompatibilityRequirement {
                field,
                requirement: requirement.to_string(),
            }
        })?;

        if host_version == exact {
            return Ok(());
        }

        return Err(HostProfileAdmissionError::IncompatibleBotsterVersion {
            field,
            requirement: requirement.to_string(),
            host_version: host_version_string.to_string(),
        });
    }

    Err(
        HostProfileAdmissionError::UnsupportedCompatibilityRequirement {
            field,
            requirement: requirement.to_string(),
        },
    )
}
