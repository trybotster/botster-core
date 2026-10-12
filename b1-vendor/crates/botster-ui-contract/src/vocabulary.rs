//! The UiNode vocabulary: the node kinds and the allowed props and slots of each kind.
//!
//! Clause: UI-RZ-2, UI-RZ-2a (the mapping of ui-contract 1.0 section 12 names this vocabulary as the existing one).
//! Steward ruling R-2.3 adopts it by reference. UI-RZ-2e and UI-RZ-2f add props (see `node`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Shared semantic UI node kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeKind {
    /// Vertical or horizontal stack layout.
    Stack,
    /// Inline layout.
    Inline,
    /// Form container.
    Form,
    /// Form section grouping.
    FormSection,
    /// Schema-driven form field.
    FormField,
    /// Panel region.
    Panel,
    /// Application metric.
    Metric,
    /// Responsive metric collection.
    MetricGrid,
    /// Semantic command/filter/search/action container.
    Toolbar,
    /// Compact status display with state semantics.
    StatusBadge,
    /// Lightweight content grouping.
    Section,
    /// Scrollable region.
    ScrollArea,
    /// Text node.
    Text,
    /// Icon node.
    Icon,
    /// Badge node.
    Badge,
    /// Status dot node.
    StatusDot,
    /// Empty state node.
    EmptyState,
    /// List container.
    List,
    /// List item.
    ListItem,
    /// Tree container.
    Tree,
    /// Tree item.
    TreeItem,
    /// Table container.
    Table,
    /// Button/action node.
    Button,
    /// Icon-only button/action node.
    IconButton,
    /// Menu container.
    Menu,
    /// Menu item.
    MenuItem,
    /// Dialog node.
    Dialog,
    /// Text input node.
    TextInput,
    /// Textarea node.
    Textarea,
    /// Checkbox node.
    Checkbox,
    /// Select node.
    Select,
    /// Select option node.
    SelectOption,
    /// Terminal view placeholder.
    TerminalView,
    /// Connection-code view placeholder.
    ConnectionCodeView,
    /// Sandboxed iframe or webview placeholder for generated HTML surfaces.
    Iframe,
    /// Owner-namespaced custom component with a validated renderer fallback.
    Custom,
    /// The embedded plugin page (UI-RZ-2f, ui-contract 1.2). Added by the clause, not part of the stolen kinds.
    Embed,
}

/// The allowed props and slots of a node kind (UI-RZ-2a).
pub fn node_schema(kind: UiNodeKind) -> UiNodeSchema {
    match kind {
        UiNodeKind::Stack => schema(
            &["direction", "gap", "align", "justify"],
            &["direction"],
            &[],
            &[],
        ),
        UiNodeKind::Inline => schema(&["gap", "align", "justify"], &[], &[], &[]),
        UiNodeKind::Form => schema(
            &["action", "submit_label", "disabled", "loading", "error"],
            &["action", "submit_label"],
            &[],
            &[],
        ),
        UiNodeKind::FormSection => schema(
            &["title", "description", "disabled", "loading", "error"],
            &["title"],
            &[],
            &[],
        ),
        UiNodeKind::FormField => schema(
            &[
                "schema", "value", "checked", "selected", "default", "disabled", "loading", "error",
            ],
            &["schema"],
            &[],
            &[],
        ),
        UiNodeKind::Panel => schema(
            &["title", "tone", "density", "variant"],
            &[],
            &["header", "toolbar", "body", "footer", "empty", "actions"],
            &[],
        ),
        UiNodeKind::Metric => schema(
            &[
                "label", "value", "caption", "tone", "status", "trend", "delta", "action", "ref",
            ],
            &["label", "value"],
            &[],
            &[],
        ),
        UiNodeKind::MetricGrid => schema(&["density", "variant", "compact"], &[], &[], &[]),
        UiNodeKind::Toolbar => schema(
            &["label", "density", "variant"],
            &[],
            &["commands", "filters", "search", "actions"],
            &[],
        ),
        UiNodeKind::StatusBadge => schema(
            &["label", "status", "tone", "hover_label", "action"],
            &["label"],
            &[],
            &[],
        ),
        UiNodeKind::Section => schema(
            &["title", "description", "density", "variant"],
            &[],
            &["header", "toolbar", "body", "footer", "empty", "actions"],
            &[],
        ),
        UiNodeKind::ScrollArea => schema(&["height"], &[], &[], &[]),
        UiNodeKind::Text => schema(
            &["text", "tone", "variant", "hover_label", "copy_value"],
            &["text"],
            &[],
            &[],
        ),
        UiNodeKind::Icon => schema(
            &["icon", "label", "tone", "hover_label"],
            &["icon"],
            &[],
            &[],
        ),
        UiNodeKind::Badge => schema(&["label", "tone", "hover_label"], &["label"], &[], &[]),
        UiNodeKind::StatusDot => schema(&["label", "tone", "hover_label"], &["label"], &[], &[]),
        UiNodeKind::EmptyState => schema(
            &[
                "title",
                "description",
                "icon",
                "action",
                "primary_action",
                "secondary_action",
            ],
            &["title"],
            &[],
            &[],
        ),
        UiNodeKind::List => schema(&["aria_label", "selection"], &[], &[], &[]),
        UiNodeKind::ListItem => schema(
            &[
                "value",
                "selected",
                "action",
                "activation",
                "hover_label",
                "context_menu",
            ],
            &[],
            &["title", "subtitle", "meta", "actions"],
            &["title"],
        ),
        UiNodeKind::Tree => schema(&["aria_label"], &[], &[], &[]),
        UiNodeKind::TreeItem => schema(
            &[
                "value",
                "expanded",
                "selected",
                "hover_label",
                "context_menu",
            ],
            &[],
            &["title", "children", "actions"],
            &["title"],
        ),
        UiNodeKind::Table => schema(
            &[
                "columns",
                "rows",
                "empty_state",
                "selection",
                "row_action",
                "activation",
            ],
            &["columns"],
            &[],
            &[],
        ),
        UiNodeKind::Button => schema(
            &[
                "label",
                "action",
                "tone",
                "variant",
                "shortcut",
                "hover_label",
                "context_menu",
                "toolbar_overflow",
            ],
            &[],
            &[],
            &[],
        ),
        UiNodeKind::IconButton => schema(
            &[
                "label",
                "icon",
                "action",
                "tone",
                "variant",
                "shortcut",
                "hover_label",
                "context_menu",
                "toolbar_overflow",
            ],
            &["icon"],
            &[],
            &[],
        ),
        UiNodeKind::Menu => schema(&["label"], &[], &["items"], &["items"]),
        UiNodeKind::MenuItem => schema(
            &[
                "label",
                "action",
                "icon",
                "shortcut",
                "hover_label",
                "context_menu",
                "toolbar_overflow",
            ],
            &[],
            &[],
            &[],
        ),
        UiNodeKind::Dialog => schema(
            &["title", "presentation"],
            &["title"],
            &["body", "actions"],
            &["body"],
        ),
        UiNodeKind::TextInput => schema(
            &[
                "name",
                "label",
                "description",
                "value",
                "default",
                "placeholder",
                "required",
                "disabled",
                "loading",
                "error",
                "validation",
            ],
            &["name", "label"],
            &[],
            &[],
        ),
        UiNodeKind::Textarea => schema(
            &[
                "name",
                "label",
                "description",
                "value",
                "default",
                "placeholder",
                "required",
                "disabled",
                "loading",
                "error",
                "validation",
            ],
            &["name", "label"],
            &[],
            &[],
        ),
        UiNodeKind::Checkbox => schema(
            &[
                "name",
                "label",
                "description",
                "checked",
                "default",
                "required",
                "disabled",
                "loading",
                "error",
                "validation",
            ],
            &["name", "label"],
            &[],
            &[],
        ),
        UiNodeKind::Select => schema(
            &[
                "name",
                "label",
                "description",
                "value",
                "selected",
                "default",
                "required",
                "disabled",
                "loading",
                "error",
                "validation",
                "options_source",
            ],
            &["name", "label"],
            &["options"],
            // options xor options_source enforced in validate_prop_combinations
            &[],
        ),
        UiNodeKind::SelectOption => schema(
            &["value", "label", "disabled"],
            &["value", "label"],
            &[],
            &[],
        ),
        UiNodeKind::TerminalView => schema(&["session_id", "title"], &["session_id"], &[], &[]),
        UiNodeKind::ConnectionCodeView => {
            schema(&["code", "label", "copy_value"], &["code"], &[], &[])
        }
        UiNodeKind::Iframe => schema(
            &["src", "title", "sandbox", "allow", "bridge"],
            &["src", "title"],
            &[],
            &[],
        ),
        // UI-RZ-2f: props `title`, `asset_id`, `channel`, `state`.
        UiNodeKind::Embed => schema(
            &["title", "asset_id", "channel", "state"],
            &["title"],
            &[],
            &[],
        ),
        UiNodeKind::Custom => schema(
            &["namespace", "component", "reason"],
            &["namespace", "component", "reason"],
            &["fallback"],
            &["fallback"],
        ),
    }
}

/// Builds one table row.
fn schema(
    allowed_props: &[&'static str],
    required_props: &[&'static str],
    allowed_slots: &[&'static str],
    required_slots: &[&'static str],
) -> UiNodeSchema {
    UiNodeSchema {
        allowed_props: allowed_props.iter().copied().collect(),
        required_props: required_props.to_vec(),
        allowed_slots: allowed_slots.iter().copied().collect(),
        required_slots: required_slots.to_vec(),
    }
}

/// The props and slots that one node kind allows (the `schema(...)` table).
#[derive(Debug, Clone)]
pub struct UiNodeSchema {
    pub allowed_props: BTreeSet<&'static str>,
    pub required_props: Vec<&'static str>,
    pub allowed_slots: BTreeSet<&'static str>,
    pub required_slots: Vec<&'static str>,
}
