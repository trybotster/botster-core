//! Minimal shared UI contract scaffolding.

use serde::{Deserialize, Serialize};

/// Stable UI node identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiNodeId(pub String);

/// Shared UI node kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeKind {
    /// Text node.
    Text,
    /// Button/action node.
    Button,
    /// Form node.
    Form,
    /// List node.
    List,
    /// Custom primitive rendered by a client adapter.
    Primitive(String),
}
