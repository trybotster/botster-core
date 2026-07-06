//! Package navigation intent contracts.

use serde::{Deserialize, Serialize};

/// Optional package-authored navigation intent inspected by hosts without
/// running plugin code.
///
/// Navigation entries are not ordering, pinning, hiding, shell-placement, or
/// admission authority. Hosts decide whether and where admitted entries render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageNavigationEntry {
    /// Stable navigation item identifier within the package.
    pub id: String,
    /// User-facing label for the navigation item.
    pub label: String,
    /// Optional renderer-neutral icon token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Optional descriptive help text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Host-resolved target for the navigation item.
    pub target: PackageNavigationTarget,
}

/// Host-resolved target for a package navigation entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageNavigationTarget {
    /// Target one package surface by stable surface id.
    Surface {
        /// Stable surface identifier within the same package.
        surface_id: String,
    },
}
