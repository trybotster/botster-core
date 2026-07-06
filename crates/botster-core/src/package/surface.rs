//! Package UI surface descriptor contracts.

use serde::{Deserialize, Serialize};

/// Transport-neutral UI surface metadata carried by a package manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSurfaceDescriptor {
    /// Stable surface identifier within the package.
    pub id: String,
    /// Semantic surface kind.
    pub kind: PackageSurfaceKind,
    /// Human-readable surface title.
    pub title: String,
    /// Optional surface help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional renderer-neutral icon or token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Legacy non-authoritative ordering hint kept for manifest compatibility.
    /// Hosts, users, and clients own actual navigation ordering policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    /// Optional host-readable category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Supported surface operations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supports: Vec<PackageSurfaceOperation>,
}

/// Semantic UI surface kinds a package can declare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageSurfaceKind {
    /// Main application surface.
    App,
    /// Settings or preferences surface.
    Settings,
    /// Dashboard widget surface.
    DashboardWidget,
    /// Diagnostic or troubleshooting surface.
    Diagnostics,
}

/// Operations a client can perform for a declared package surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageSurfaceOperation {
    /// Client can render the surface.
    Render,
    /// Client can invoke actions for the surface.
    Action,
}
