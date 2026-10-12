//! The `realize` inputs: kind snapshots, local state, viewport and capabilities.
//!
//! Clause: UI-RZ-1, UI-RZ-3, UI-PT-3, UI-CF-1, UI-CF-2, UI-CF-6, UI-CP-1, UI-RF-1, UI-AC-5, UI-RZ-2b, UI-RZ-2d, UI-NV-5.

use crate::doc::{ViewDocument, ViewportAxis};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The error of a watch (UI-NV-5 `kind_snapshots`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A watched kind at one cut (UI-NV-5, section 12 of ui-contract 1.0, Hub to client WT-4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct KindSnapshot {
    /// False before the first baseline arrives (UI-RF-2, UI-CP-1).
    #[serde(default)]
    pub baseline: bool,
    /// The hub that these rows come from. A ref key with another `hub_id` is absent (UI-RF-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hub_id: Option<String>,
    #[serde(default)]
    pub rows: Vec<Value>,
    /// Set when the watch failed or ended (UI-CP-1, UI-NV-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SnapshotError>,
}

/// One draft (UI-NV-5): a value, no value that hides the default, or a cleared secret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Draft {
    /// The edited value (UI-ID-5).
    Value(Value),
    /// The field has no value and does not show its default (UI-NV-4, UI-FM-4).
    Unset(bool),
    /// Allowed only for a `secret: true` property (UI-FM-8).
    Clear(bool),
}

/// One open overlay (UI-NV-5): the form or the confirmation of a button, keyed by the button node id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Overlay {
    /// An open action form (UI-AC-4).
    Form(OverlayButton),
    /// A pending confirmation (UI-AC-3).
    Confirm(OverlayButton),
}

/// `{ button }` of an overlay entry (UI-NV-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OverlayButton {
    pub button: String,
}

impl Overlay {
    pub fn button(&self) -> &str {
        match self {
            Overlay::Form(b) | Overlay::Confirm(b) => &b.button,
        }
    }
}

/// An error code with an optional message (UI-NV-2 `error`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorValue {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// The errors that the renderer and `realize` stamp (UI-NV-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct LocalErrors {
    /// Draft key to error.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, ErrorValue>,
    /// Form node id to message.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub forms: BTreeMap<String, String>,
    /// Button node id to message.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, String>,
}

/// The client-local state (P5, UI-RZ-2d, UI-NV-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct LocalState {
    /// Slot id to row key (UI-SL-1).
    #[serde(default)]
    pub selections: BTreeMap<String, String>,
    /// Draft key to draft (UI-ID-5).
    #[serde(default)]
    pub drafts: BTreeMap<String, Draft>,
    /// Open overlays, in opening order (UI-NV-4).
    #[serde(default)]
    pub overlays: Vec<Overlay>,
    #[serde(default)]
    pub errors: LocalErrors,
    /// The node id with row focus. It never sets a selection (UI-SL-4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    /// Node id to an opaque renderer value (UI-NV-5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scroll: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WidthClass {
    Compact,
    Regular,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeightClass {
    Short,
    Regular,
    Tall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PointerClass {
    Fine,
    Coarse,
    None,
}

/// The viewport classes (UI-PT-3). There are 27 combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Viewport {
    pub width: WidthClass,
    pub height: HeightClass,
    pub pointer: PointerClass,
}

impl Viewport {
    /// All 27 classes (UI-PT-3).
    pub fn all() -> Vec<Viewport> {
        let mut out = Vec::with_capacity(27);
        for width in [
            WidthClass::Compact,
            WidthClass::Regular,
            WidthClass::Expanded,
        ] {
            for height in [HeightClass::Short, HeightClass::Regular, HeightClass::Tall] {
                for pointer in [PointerClass::Fine, PointerClass::Coarse, PointerClass::None] {
                    out.push(Viewport {
                        width,
                        height,
                        pointer,
                    });
                }
            }
        }
        out
    }

    /// The class text of one axis, as `eq` and `ne` compare it (UI-PT-3).
    pub fn class(&self, axis: ViewportAxis) -> &'static str {
        match axis {
            ViewportAxis::Width => match self.width {
                WidthClass::Compact => "compact",
                WidthClass::Regular => "regular",
                WidthClass::Expanded => "expanded",
            },
            ViewportAxis::Height => match self.height {
                HeightClass::Short => "short",
                HeightClass::Regular => "regular",
                HeightClass::Tall => "tall",
            },
            ViewportAxis::Pointer => match self.pointer {
                PointerClass::Fine => "fine",
                PointerClass::Coarse => "coarse",
                PointerClass::None => "none",
            },
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            width: WidthClass::Regular,
            height: HeightClass::Regular,
            pointer: PointerClass::Fine,
        }
    }
}

/// What a renderer declares (UI-CF-6, section 11 of ui-contract 1.0). The declaration is diagnostic and local (UI-CF-1).
/// `realize` reads it to choose fallbacks (UI-CF-2, UI-CF-5, UI-EM-6).
///
/// An absent `parts` or `widgets` list means all. An absent `features` or `screens` list means none (UI-CF-6).
/// An empty list means none, in every list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Capabilities {
    /// Supported `"M.m"` values of `ui_contract_view`. Required (UI-CF-6).
    pub ui_contract_view: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widgets: Option<Vec<String>>,
    /// Each entry is `"<kind>@<MAJOR>"` (UI-CF-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screens: Option<Vec<String>>,
}

impl Capabilities {
    /// UI-CF-6: an absent `parts` list means every part.
    pub fn has_part(&self, name: &str) -> bool {
        self.parts
            .as_ref()
            .is_none_or(|l| l.iter().any(|p| p == name))
    }
    /// UI-CF-6: an absent `features` list means no feature.
    pub fn has_feature(&self, name: &str) -> bool {
        self.features
            .as_ref()
            .is_some_and(|l| l.iter().any(|p| p == name))
    }
    /// UI-CF-6: an absent `widgets` list means every widget.
    pub fn has_widget(&self, name: &str) -> bool {
        self.widgets
            .as_ref()
            .is_none_or(|l| l.iter().any(|p| p == name))
    }
    /// UI-CF-6: an absent `screens` list means no screen. An entry is `"<kind>@<MAJOR>"` (UI-SL-3, DS-4).
    pub fn has_screen(&self, kind: &str, major: &str) -> bool {
        let want = format!("{kind}@{major}");
        self.screens.as_ref().is_some_and(|l| l.contains(&want))
    }
}

/// All inputs of `realize` (section 12 of ui-contract 1.0). The vector files use this shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RealizeInput {
    pub document: ViewDocument,
    #[serde(default)]
    pub kind_snapshots: BTreeMap<String, KindSnapshot>,
    #[serde(default)]
    pub local_state: LocalState,
    #[serde(default)]
    pub viewport: Viewport,
    /// The renderer's declaration (UI-CF-6). It is an input of `realize` (section 12).
    pub capabilities: Capabilities,
}
