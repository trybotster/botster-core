//! Package configuration schema and value contracts.

use serde::{Deserialize, Serialize};
use serde_json::Number;

/// Transport-neutral configuration schema carried by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigurationSchema {
    /// Optional display groups for related fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<PackageConfigurationGroup>,
    /// Configuration fields clients and hubs can inspect without running plugin code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<PackageConfigurationField>,
}

/// Display grouping metadata for configuration fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigurationGroup {
    /// Stable group identifier referenced by fields.
    pub id: String,
    /// Human-readable group label.
    pub label: String,
    /// Optional group help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Host-owned ordering hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
}

/// Configuration field metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigurationField {
    /// Stable field key within the package configuration object.
    pub key: String,
    /// Field value type.
    #[serde(rename = "type")]
    pub field_type: PackageConfigurationFieldType,
    /// Human-readable field label.
    pub label: String,
    /// Optional field help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the hub should require a value before enabling the package.
    #[serde(default)]
    pub required: bool,
    /// Optional metadata default. Secret defaults can only use redacted/write-only states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<PackageConfigurationValue>,
    /// Optional validation metadata. Core does not execute these hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<PackageConfigurationValidationHints>,
    /// Optional group id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Host-owned ordering hint within the field list or group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    /// Enumerated choices for select fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<PackageConfigurationOption>,
}

/// Supported package configuration field types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageConfigurationFieldType {
    /// Single-line string input.
    String,
    /// JSON number input.
    Number,
    /// Integer input.
    Integer,
    /// Boolean input.
    Boolean,
    /// Enumerated select input.
    Select,
    /// Filesystem path string.
    Path,
    /// URL string.
    Url,
    /// Multiline text input.
    MultilineText,
    /// Secret input whose serialized values are redacted or write-only.
    Secret,
}

/// Select-field option metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigurationOption {
    /// Stable option value.
    pub value: String,
    /// Human-readable option label.
    pub label: String,
    /// Optional option help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Validation metadata for package configuration fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageConfigurationValidationHints {
    /// Minimum string length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    /// Maximum string length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    /// Pattern string a host may interpret.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Minimum numeric value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Number>,
    /// Maximum numeric value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Number>,
    /// Allowed file extensions for path-like values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_extensions: Vec<String>,
}

/// Transport-neutral configuration value shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageConfigurationValue {
    /// String value.
    String {
        /// Serialized value.
        value: String,
    },
    /// JSON number value.
    Number {
        /// Serialized value.
        value: Number,
    },
    /// Integer value.
    Integer {
        /// Serialized value.
        value: i64,
    },
    /// Boolean value.
    Boolean {
        /// Serialized value.
        value: bool,
    },
    /// Select option value.
    Select {
        /// Selected option value.
        value: String,
    },
    /// Path string value.
    Path {
        /// Path value as declared by a host-owned policy.
        value: String,
    },
    /// URL string value.
    Url {
        /// URL value as declared by a host-owned policy.
        value: String,
    },
    /// Multiline text value.
    MultilineText {
        /// Serialized value.
        value: String,
    },
    /// Secret value marker. This shape never carries raw secret material.
    Secret {
        /// Secret value state.
        state: PackageConfigurationSecretValue,
    },
}

/// Secret configuration value marker states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageConfigurationSecretValue {
    /// A value exists but is intentionally redacted from this payload.
    Redacted,
    /// A caller supplied a new value through a write-only path.
    WriteOnly,
    /// No secret value is present.
    Unset,
}
