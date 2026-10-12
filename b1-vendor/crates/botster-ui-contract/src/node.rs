//! The realized vocabulary: `UiNode`, `RealizedTree` and the props table.
//!
//! Clause: UI-RZ-2, UI-RZ-2a, UI-RZ-2b, UI-RZ-2e, UI-RZ-2f, UI-RZ-5, UI-SL-2, UI-RZ-2d, UI-NV-1, UI-NV-4.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::vocabulary::node_schema;
pub use crate::vocabulary::UiNodeKind;

/// The copy keys of UI-NV-6. A renderer maps each key to its own words; it shows an unknown key as a generic text and counts it.
/// `realize` emits only these keys (`tests/copy_keys.rs`).
pub const COPY_KEYS: &[&str] = &[
    "generic_empty",
    "generic_error",
    "nothing_selected",
    "submit",
    "cancel",
    "confirm",
    "optional_section",
    "form_unavailable",
    "view_unavailable",
];

/// The args of each copy key that takes args (UI-NV-6: `view_unavailable` with `{ view_id, reason }`, `reason` the diagnostic code
/// of UI-CF-1). A renderer ignores unknown args.
pub const COPY_KEY_ARGS: &[(&str, &[&str])] = &[("view_unavailable", &["view_id", "reason"])];

/// The values of the prop `absent_reason` (UI-RF-1; the closed list of UI-NV-2). `realize` emits only these
/// (`tests/codes.rs`).
pub const ABSENT_REASONS: &[&str] = &["not_found", "no_baseline", "remote_hub_unsupported"];

/// The local error codes (UI-FM-4; the closed list of UI-NV-2). `realize` stamps `option_unavailable` (UI-FM-4). A client sets
/// `required_missing` at submit (UI-FM-3 `required_when`, or a required property with no value), so `realize` never emits it.
pub const LOCAL_ERROR_CODES: &[&str] = &["option_unavailable", "required_missing"];

/// The diagnostic codes that this crate emits (UI-AD-2 `stale`, UI-NV-4). The UI-NV-4 list is non-exhaustive, so this list holds
/// exactly the codes that `realize`, admission and the vector runner emit (`tests/codes.rs`); it contains every code that UI-NV-4
/// names. A renderer shows an unknown code as a generic diagnostic.
pub const DIAGNOSTIC_CODES: &[&str] = &[
    "duplicate_node_id",
    "unknown_tone",
    "unknown_display",
    "open_without_screen",
    "masked_input_unavailable",
    "uneditable_required_property",
    "unsupported_view_version",
    "unsupported_view_feature",
    "unparsable_view",
    "row_without_id",
    "stale",
    "navigate_slot_absent",
    "url_refused",
    "unknown_widget",
];

/// A prop value that can show a default (UI-NV-6): `Text = string | { copy: "<key>", args?: { <name>: string } }`.
/// A string is always authored text, shown literally, even when it equals a key name. An object is always a copy key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Text {
    Authored(String),
    Copy(CopyKey),
}

/// The copy key object of [`Text`] (UI-NV-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CopyKey {
    pub copy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<std::collections::BTreeMap<String, String>>,
}

impl Text {
    /// The copy key `key` with no args.
    pub fn copy(key: &str) -> Self {
        Text::Copy(CopyKey {
            copy: key.to_string(),
            args: None,
        })
    }
}

/// The props that the clauses add to the stolen vocabulary, per kind (UI-RZ-2a, UI-RZ-2e, UI-RZ-2f, UI-NV-1).
/// UI-RZ-2a names: `label`, `value`, `selects`, `activate`, `confirm`, `action_ref`, `absent_reason`,
/// `disabled_reason`, `error`, `options`, `placeholder`, `help`, `disabled`, `required`, `tone`, `selected`.
/// UI-NV-6 names `label` as a `Text` prop of `form_section`. UI-NV-1 adds `state`, `detail`, `collapsed`, `read_only`, `masked`, `input`, `secret_set`, `indent`, `name` (embed)
/// and the `selected` and `activate` entries of `table.rows`.
pub const CLAUSE_PROPS: &[(UiNodeKind, &[&str])] = &[
    (UiNodeKind::Text, &["activate", "absent_reason", "href"]),
    (UiNodeKind::Badge, &["activate", "absent_reason"]),
    (UiNodeKind::Metric, &["activate", "absent_reason"]),
    (UiNodeKind::StatusBadge, &["absent_reason", "activate"]),
    (UiNodeKind::List, &["selects", "state", "detail"]),
    (UiNodeKind::ListItem, &["selects", "activate"]),
    (UiNodeKind::Table, &["selects", "state", "detail"]),
    (UiNodeKind::Panel, &["state", "detail"]),
    (UiNodeKind::Stack, &["indent"]),
    (UiNodeKind::FormSection, &["label", "collapsed"]),
    (
        UiNodeKind::Button,
        &["action_ref", "confirm", "disabled", "disabled_reason"],
    ),
    (
        UiNodeKind::MenuItem,
        &["action_ref", "confirm", "disabled", "disabled_reason"],
    ),
    (UiNodeKind::FormField, &["required", "help", "placeholder"]),
    (
        UiNodeKind::TextInput,
        &["help", "read_only", "masked", "input", "secret_set"],
    ),
    (UiNodeKind::Textarea, &["help", "read_only"]),
    (UiNodeKind::Checkbox, &["help", "read_only"]),
    (
        UiNodeKind::Select,
        &["help", "options", "placeholder", "read_only"],
    ),
    (
        UiNodeKind::Embed,
        &["title", "asset_id", "name", "channel", "state"],
    ),
];

/// Allows `prop` on `kind`: a prop of the stolen vocabulary, a prop that a clause adds, or a prop that awaits a ruling.
pub fn prop_allowed(kind: UiNodeKind, prop: &str) -> bool {
    node_schema(kind).allowed_props.contains(prop)
        || CLAUSE_PROPS
            .iter()
            .any(|(k, l)| *k == kind && l.contains(&prop))
}

/// Allows `slot` on `kind` (the stolen vocabulary).
pub fn slot_allowed(kind: UiNodeKind, slot: &str) -> bool {
    node_schema(kind).allowed_slots.contains(slot)
}

/// A realized node (UI-RZ-2). `id` is an opaque string (UI-ID-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UiNode {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: UiNodeKind,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub slots: std::collections::BTreeMap<String, Vec<UiNode>>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiNode>,
}

impl UiNode {
    pub fn new(id: impl Into<String>, kind: UiNodeKind) -> Self {
        UiNode {
            id: id.into(),
            kind,
            props: Map::new(),
            children: Vec::new(),
            slots: std::collections::BTreeMap::new(),
        }
    }

    /// Sets a prop. A `None` value is omitted (UI-RZ-2a).
    pub fn prop(mut self, name: &str, value: impl Into<Option<Value>>) -> Self {
        if let Some(v) = value.into() {
            self.props.insert(name.to_string(), v);
        }
        self
    }

    pub fn slot(mut self, name: &str, nodes: Vec<UiNode>) -> Self {
        self.slots.insert(name.to_string(), nodes);
        self
    }

    pub fn with_children(mut self, children: Vec<UiNode>) -> Self {
        self.children = children;
        self
    }

    /// Calls `f` on this node and on every descendant, in document order.
    pub fn walk<'a>(&'a self, f: &mut dyn FnMut(&'a UiNode)) {
        f(self);
        for c in self.children.iter().chain(self.slots.values().flatten()) {
            c.walk(f);
        }
    }

    pub fn find(&self, id: &str) -> Option<&UiNode> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter()
            .chain(self.slots.values().flatten())
            .find_map(|c| c.find(id))
    }
}

/// A diagnostic of a realization (UI-NV-4). A diagnostic never fails the realization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// The node id that the diagnostic names, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Diagnostic {
    /// A diagnostic with a code that this crate chooses, one of [`DIAGNOSTIC_CODES`]. `diagnostic_tree` takes an open code instead.
    pub(crate) fn new(node_id: Option<&str>, code: &str, detail: &str) -> Diagnostic {
        debug_assert!(
            DIAGNOSTIC_CODES.contains(&code),
            "{code} is not a diagnostic code (UI-NV-4)"
        );
        Diagnostic {
            node_id: node_id.map(str::to_string),
            code: code.to_string(),
            detail: Some(detail.to_string()),
        }
    }
}

/// An error that `realize` itself found (UI-NV-4 `stamped_errors`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StampedError {
    pub draft_key: String,
    pub code: String,
}

/// What `realize` changed in the local state; the renderer applies it before the next draw (UI-NV-4, UI-RZ-2d).
/// An empty list is omitted.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct LocalChanges {
    /// Slots whose row left its kind, or that the document no longer declares (UI-SL-2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cleared_selections: Vec<String>,
    /// Draft keys whose shown value is no longer in its options (UI-FM-4). The renderer sets each to `{ unset: true }`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unset_drafts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stamped_errors: Vec<StampedError>,
    /// Node ids of `focus` or `scroll` entries whose node left the tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dropped_view_state: Vec<String>,
}

/// The result of `realize` (UI-NV-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RealizedTree {
    pub root: UiNode,
    /// The dialogs to show (UI-RZ-2b, UI-NV-4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<UiNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "is_default_changes")]
    pub local_changes: LocalChanges,
}

fn is_default_changes(c: &LocalChanges) -> bool {
    *c == LocalChanges::default()
}

impl RealizedTree {
    /// Every node of the tree, overlays included, in document order.
    pub fn nodes(&self) -> Vec<&UiNode> {
        let mut out = Vec::new();
        self.root.walk(&mut |n| out.push(n));
        for o in &self.overlays {
            o.walk(&mut |n| out.push(n));
        }
        out
    }

    /// Every id that names a realized node: node ids, slot nodes, table row entries and cell nodes (UI-RZ-5, UI-ID-2).
    pub fn identity_ids(&self) -> Vec<String> {
        fn cell_ids(v: &Value, out: &mut Vec<String>) {
            if let Value::Object(m) = v {
                if let Some(Value::String(id)) = m.get("id") {
                    out.push(id.clone());
                }
            }
        }
        let mut out = Vec::new();
        for n in self.nodes() {
            out.push(n.id.clone());
            if n.kind == UiNodeKind::Table {
                for row in n
                    .props
                    .get("rows")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(Value::String(id)) = row.get("id") {
                        out.push(id.clone());
                    }
                    for cell in row
                        .get("cells")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flat_map(|c| c.values())
                    {
                        cell_ids(cell, &mut out);
                    }
                }
            }
        }
        out
    }

    pub fn find(&self, id: &str) -> Option<&UiNode> {
        self.root
            .find(id)
            .or_else(|| self.overlays.iter().find_map(|o| o.find(id)))
    }

    /// True when every node kind and prop is in the vocabulary (UI-RZ-2c). Returns the first violation.
    pub fn check_vocabulary(&self) -> Result<(), String> {
        for n in self.nodes() {
            for slot in n.slots.keys() {
                if !slot_allowed(n.kind, slot) {
                    return Err(format!(
                        "node '{}' of kind {:?} has the unsupported slot '{slot}'",
                        n.id, n.kind
                    ));
                }
            }
            for name in n.props.keys() {
                if !prop_allowed(n.kind, name) {
                    return Err(format!(
                        "node '{}' of kind {:?} has the unsupported prop '{name}'",
                        n.id, n.kind
                    ));
                }
            }
        }
        Ok(())
    }
}

/// One row of the part to node mapping (UI-RZ-2, UI-RZ-2b, steward ruling R-2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingRow {
    /// The part, field display or overlay.
    pub from: &'static str,
    /// The node kinds that it realizes into, outermost first.
    pub to: &'static [UiNodeKind],
    /// True when the outer node takes the role segment `container` (R-2.2).
    pub container: bool,
}

/// The normative part to node table (UI-RZ-2, UI-RZ-2b). Table rows are entries of the `rows` prop of one `table` node.
pub const MAPPING: &[MappingRow] = &[
    MappingRow {
        from: "list",
        to: &[UiNodeKind::List, UiNodeKind::ListItem],
        container: true,
    },
    MappingRow {
        from: "table",
        to: &[UiNodeKind::Table],
        container: false,
    },
    MappingRow {
        from: "detail",
        to: &[UiNodeKind::Panel, UiNodeKind::Inline],
        container: true,
    },
    MappingRow {
        from: "form",
        to: &[
            UiNodeKind::Form,
            UiNodeKind::FormSection,
            UiNodeKind::FormField,
            UiNodeKind::TextInput,
            UiNodeKind::Textarea,
            UiNodeKind::Checkbox,
            UiNodeKind::Select,
        ],
        container: true,
    },
    MappingRow {
        from: "actions",
        to: &[UiNodeKind::Toolbar, UiNodeKind::Button, UiNodeKind::Menu],
        container: true,
    },
    MappingRow {
        from: "toolbar",
        to: &[UiNodeKind::Toolbar, UiNodeKind::Button, UiNodeKind::Menu],
        container: true,
    },
    MappingRow {
        from: "status_badge",
        to: &[UiNodeKind::StatusBadge],
        container: false,
    },
    MappingRow {
        from: "stack",
        to: &[UiNodeKind::Stack],
        container: false,
    },
    MappingRow {
        from: "inline",
        to: &[UiNodeKind::Inline],
        container: false,
    },
    MappingRow {
        from: "panel",
        to: &[UiNodeKind::Panel],
        container: false,
    },
    MappingRow {
        from: "section",
        to: &[UiNodeKind::Section],
        container: false,
    },
    MappingRow {
        from: "text",
        to: &[UiNodeKind::Text],
        container: false,
    },
    MappingRow {
        from: "empty_state",
        to: &[UiNodeKind::EmptyState],
        container: false,
    },
    MappingRow {
        from: "embed",
        to: &[UiNodeKind::Embed],
        container: false,
    },
    MappingRow {
        from: "field:metric",
        to: &[UiNodeKind::Metric],
        container: false,
    },
    MappingRow {
        from: "field:badge",
        to: &[UiNodeKind::Badge],
        container: false,
    },
    MappingRow {
        from: "field:text",
        to: &[UiNodeKind::Inline, UiNodeKind::Text],
        container: true,
    },
    MappingRow {
        from: "overlay:form",
        to: &[UiNodeKind::Dialog, UiNodeKind::Form],
        container: true,
    },
    MappingRow {
        from: "overlay:confirm",
        to: &[UiNodeKind::Dialog, UiNodeKind::Button],
        container: true,
    },
];
