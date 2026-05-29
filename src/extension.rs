//! Extension package metadata shared by plugin runtimes and providers.

use serde::{Deserialize, Serialize};

/// Extension execution kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    /// Ordinary runtime plugin.
    Plugin,
    /// Privileged plugin that provides bootstrap or authority surfaces.
    Provider,
}

/// Runtime used to execute extension code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntime {
    /// In-process Lua runtime.
    Lua,
    /// Supervised out-of-process provider/plugin.
    Process,
    /// Future WebAssembly runtime.
    Wasm,
}

/// Extension entrypoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionEntrypoint {
    /// Runtime used by this entrypoint.
    pub runtime: ExtensionRuntime,
    /// Entrypoint path relative to package root.
    pub path: String,
    /// Whether this entrypoint participates in hub bootstrap.
    pub bootstrap: bool,
}
