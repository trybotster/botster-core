//! Compatibility descriptors for the independent terminal protocol plane.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    CONFORMANCE_FIXTURE_REVISION, DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING,
    FEATURE_TRANSPORT_DUPLEX_BINARY, PROTOCOL, PROTOCOL_VERSION,
};

/// Advertised terminal-plane support.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCompatibility {
    /// Protocol name. Must equal [`PROTOCOL`].
    pub protocol: String,
    /// Exact protocol version.
    pub protocol_version: u16,
    /// Advertised feature tokens.
    pub features: Vec<String>,
    /// Conformance fixture revision this descriptor claims.
    pub conformance_fixture_revision: u16,
}

impl TerminalCompatibility {
    /// Advertised support for the current producer.
    ///
    /// Includes the default required tokens plus optional
    /// `snapshot_delivery=ready_then_history`.
    #[must_use]
    pub fn current() -> Self {
        Self {
            protocol: PROTOCOL.to_string(),
            protocol_version: PROTOCOL_VERSION,
            features: current_feature_list()
                .into_iter()
                .map(str::to_string)
                .collect(),
            conformance_fixture_revision: CONFORMANCE_FIXTURE_REVISION,
        }
    }

    /// Return whether `feature` is advertised.
    #[must_use]
    pub fn supports_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|supported| supported == feature)
    }
}

/// Client requirement for the terminal protocol plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCompatibilityRequirement {
    /// Required protocol name.
    pub protocol: String,
    /// Required exact protocol version.
    pub protocol_version: u16,
    /// Feature tokens the client requires.
    pub required_features: Vec<String>,
    /// Lowest accepted conformance fixture revision.
    pub minimum_conformance_fixture_revision: u16,
    /// Client name used in mismatch diagnostics.
    pub client_name: String,
}

impl TerminalCompatibilityRequirement {
    /// Default requirement for ordinary terminal operations.
    ///
    /// Requires `terminal_streaming`, `resize`, and
    /// `transport=duplex_binary`. Additive snapshot delivery does not raise
    /// this floor.
    #[must_use]
    pub fn current() -> Self {
        Self {
            protocol: PROTOCOL.to_string(),
            protocol_version: PROTOCOL_VERSION,
            required_features: default_required_feature_list()
                .into_iter()
                .map(str::to_string)
                .collect(),
            minimum_conformance_fixture_revision: DEFAULT_MINIMUM_CONFORMANCE_FIXTURE_REVISION,
            client_name: "botster-terminal-protocol".to_string(),
        }
    }

    /// Requirement for READY-then-history snapshot attach.
    #[must_use]
    pub fn for_ready_then_history_attach() -> Self {
        let mut requirement = Self::current();
        requirement
            .required_features
            .push(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY.to_string());
        requirement.minimum_conformance_fixture_revision = CONFORMANCE_FIXTURE_REVISION;
        requirement
    }
}

impl Default for TerminalCompatibilityRequirement {
    fn default() -> Self {
        Self::current()
    }
}

/// Compatibility mismatch for the terminal protocol plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCompatibilityError {
    /// Human-readable diagnostic.
    pub diagnostic: String,
}

impl fmt::Display for TerminalCompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl Error for TerminalCompatibilityError {}

/// Check that `compatibility` satisfies `requirement`.
///
/// Protocol name and version use exact equality. Conformance revision uses a
/// floor comparison. Every required feature must be advertised.
pub fn ensure_compatible(
    requirement: &TerminalCompatibilityRequirement,
    compatibility: &TerminalCompatibility,
) -> Result<(), TerminalCompatibilityError> {
    if compatibility.protocol != requirement.protocol {
        return Err(compatibility_error(
            requirement,
            compatibility,
            format!(
                "unsupported protocol {}; expected {}",
                compatibility.protocol, requirement.protocol
            ),
        ));
    }

    if compatibility.protocol_version != requirement.protocol_version {
        return Err(compatibility_error(
            requirement,
            compatibility,
            format!(
                "unsupported protocol version {}; client requires {}",
                compatibility.protocol_version, requirement.protocol_version
            ),
        ));
    }

    if compatibility.conformance_fixture_revision < requirement.minimum_conformance_fixture_revision
    {
        return Err(compatibility_error(
            requirement,
            compatibility,
            format!(
                "unsupported conformance fixture revision {}; requires at least {}",
                compatibility.conformance_fixture_revision,
                requirement.minimum_conformance_fixture_revision
            ),
        ));
    }

    let missing: Vec<&str> = requirement
        .required_features
        .iter()
        .map(String::as_str)
        .filter(|feature| !compatibility.supports_feature(feature))
        .collect();
    if !missing.is_empty() {
        return Err(compatibility_error(
            requirement,
            compatibility,
            format!("missing required feature(s): {}", missing.join(", ")),
        ));
    }

    Ok(())
}

fn compatibility_error(
    requirement: &TerminalCompatibilityRequirement,
    compatibility: &TerminalCompatibility,
    reason: String,
) -> TerminalCompatibilityError {
    TerminalCompatibilityError {
        diagnostic: format!(
            "{} is incompatible with the terminal protocol: {}; required protocol={} min_version={} required_features=[{}] min_conformance_fixture_revision={}; running protocol={} version={} features=[{}] conformance_fixture_revision={}",
            requirement.client_name,
            reason,
            requirement.protocol,
            requirement.protocol_version,
            requirement.required_features.join(","),
            requirement.minimum_conformance_fixture_revision,
            compatibility.protocol,
            compatibility.protocol_version,
            compatibility.features.join(","),
            compatibility.conformance_fixture_revision
        ),
    }
}

fn current_feature_list() -> Vec<&'static str> {
    let mut features = default_required_feature_list();
    features.push(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY);
    features
}

fn default_required_feature_list() -> Vec<&'static str> {
    vec![
        FEATURE_TERMINAL_STREAMING,
        FEATURE_RESIZE,
        FEATURE_TRANSPORT_DUPLEX_BINARY,
    ]
}
