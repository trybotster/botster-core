//! Renderer-neutral UI node, binding, viewport, and action contracts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::session::RequestId;

/// Stable UI node identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiNodeId(pub String);

/// Stable UI action identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiActionId(pub String);

/// Stable UI surface identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UiSurfaceId(pub String);

/// UI action request identity reuses the core request correlation type.
pub type UiActionRequestId = RequestId;

/// Shared semantic UI node kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}

/// Semantic width class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiWidthClass {
    /// Single-column or narrow content area.
    Compact,
    /// Standard content area.
    Regular,
    /// Wide split-pane content area.
    Expanded,
}

/// Semantic height class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiHeightClass {
    /// Short cross-axis space.
    Short,
    /// Standard cross-axis space.
    Regular,
    /// Tall cross-axis space.
    Tall,
}

/// Semantic pointer precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiPointer {
    /// No pointer input.
    None,
    /// Coarse pointer input.
    Coarse,
    /// Fine pointer input.
    Fine,
}

/// Semantic screen orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiOrientation {
    /// Portrait orientation.
    Portrait,
    /// Landscape orientation.
    Landscape,
}

/// Renderer-neutral viewport context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiViewport {
    /// Content-area width class.
    pub width_class: UiWidthClass,
    /// Content-area height class.
    pub height_class: UiHeightClass,
    /// Pointer precision.
    pub pointer: UiPointer,
    /// Optional orientation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<UiOrientation>,
    /// Whether the software keyboard occludes the viewport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_occluded: Option<bool>,
}

/// Semantic spacing token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSpaceToken {
    /// No spacing.
    None,
    /// Extra-small spacing.
    Xs,
    /// Small spacing.
    Sm,
    /// Medium spacing.
    Md,
    /// Large spacing.
    Lg,
    /// Extra-large spacing.
    Xl,
}

/// Semantic color token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiColorToken {
    /// Default foreground/background color.
    Default,
    /// Muted content color.
    Muted,
    /// Accent color.
    Accent,
    /// Success color.
    Success,
    /// Warning color.
    Warning,
    /// Danger color.
    Danger,
}

/// Binding path sentinel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiBind {
    /// Absolute entity path or item-relative path.
    pub path: String,
}

impl Serialize for UiBind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = BTreeMap::new();
        map.insert("$bind", self.path.as_str());
        map.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UiBind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = BTreeMap::<String, String>::deserialize(deserializer)?;
        match map.get("$bind") {
            Some(path) if map.len() == 1 => Ok(Self { path: path.clone() }),
            _ => Err(serde::de::Error::custom(
                "expected exactly one $bind string field",
            )),
        }
    }
}

/// Responsive value keyed by semantic width and height classes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$kind", rename_all = "lowercase")]
pub enum UiResponsiveValue {
    /// Viewport-dependent values.
    Responsive {
        /// Width values by semantic width class.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<UiResponsiveWidth>,
        /// Height values by semantic height class.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<UiResponsiveHeight>,
    },
}

/// Width responsive map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UiResponsiveWidth {
    /// Compact width value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<Value>,
    /// Regular width value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regular: Option<Value>,
    /// Expanded width value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<Value>,
}

/// Height responsive map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UiResponsiveHeight {
    /// Short height value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short: Option<Value>,
    /// Regular height value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regular: Option<Value>,
    /// Tall height value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tall: Option<Value>,
}

/// Viewport predicate used by conditional wrappers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiCondition {
    /// Width-class predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<UiWidthClass>,
    /// Height-class predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<UiHeightClass>,
    /// Pointer predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer: Option<UiPointer>,
    /// Orientation predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<UiOrientation>,
    /// Keyboard occlusion predicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_occluded: Option<bool>,
}

/// Conditional child wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$kind", rename_all = "lowercase")]
pub enum UiConditional {
    /// Render the node only when the condition matches.
    When {
        /// Viewport predicate.
        condition: UiCondition,
        /// Wrapped node.
        node: Box<UiNode>,
    },
    /// Render the node only when the condition does not match.
    Hidden {
        /// Viewport predicate.
        condition: UiCondition,
        /// Wrapped node.
        node: Box<UiNode>,
    },
}

/// Entity-backed list binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$kind", rename_all = "snake_case")]
pub enum UiBindList {
    /// Render a node template once per matching entity.
    BindList {
        /// Entity family path.
        source: String,
        /// Exact top-level field filters.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        #[serde(rename = "where")]
        r#where: BTreeMap<String, Value>,
        /// Template for each entity row.
        item_template: Box<UiNode>,
        /// Template for an empty result.
        #[serde(skip_serializing_if = "Option::is_none")]
        empty_template: Option<Box<UiNode>>,
    },
}

/// Conditional node binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "$kind", rename_all = "snake_case")]
pub enum UiBindIf {
    /// Render a node when the binding path is truthy.
    BindIf {
        /// Absolute entity path or item-relative path.
        path: String,
        /// Node to render.
        node: Box<UiNode>,
    },
}

/// Child node entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UiChild {
    /// Conditional wrapper child.
    Conditional(UiConditional),
    /// Static node child.
    Node(Box<UiNode>),
    /// Entity-backed list child.
    BindList(UiBindList),
    /// Conditional node child.
    BindIf(UiBindIf),
}

/// Shared UI node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiNode {
    /// Semantic primitive type.
    #[serde(rename = "type")]
    pub kind: UiNodeKind,
    /// Optional stable node id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<UiNodeId>,
    /// Semantic properties.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
    /// Positional child entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiChild>,
    /// Named slots for compound primitives.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Vec<UiChild>>,
}

impl UiNode {
    /// Validate the semantic UI contract recursively.
    pub fn validate(&self) -> Result<(), UiValidationError> {
        validate_ui_node(self)
    }
}

/// Validate one semantic UI node recursively.
pub fn validate_ui_node(node: &UiNode) -> Result<(), UiValidationError> {
    validate_node(node).map_err(|error| UiValidationError::Node {
        id: node.id.clone(),
        kind: node.kind,
        source: Box::new(error),
    })
}

/// Narrow v1 field kinds shared by form fields and input primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFieldKind {
    /// Single-line text input.
    Text,
    /// Multi-line text input.
    Textarea,
    /// Boolean checkbox input.
    Checkbox,
    /// Select input backed by renderer-neutral options.
    Select,
}

/// Renderer-neutral field schema for schema-driven form fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFieldSchema {
    /// Field primitive kind.
    pub kind: UiFieldKind,
    /// Submission/state name.
    pub name: String,
    /// User-facing label.
    pub label: String,
    /// Optional help or description text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional placeholder for text-like fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Whether renderers should present the field as required.
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    /// Default value used to initialize renderer-local state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// Validation hints for renderers and plugin authors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<UiFieldValidationHints>,
    /// Options for select fields.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<UiFieldOption>,
}

/// Renderer-neutral select option metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFieldOption {
    /// Submitted option value.
    pub value: Value,
    /// User-facing option label.
    pub label: String,
    /// Whether renderers should present the option as disabled.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
}

/// Field validation metadata. Core validates the shape only; renderers and
/// plugins decide how to present or enforce these hints.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFieldValidationHints {
    /// Minimum string length hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u64>,
    /// Maximum string length hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    /// Pattern hint string. Core does not compile or execute it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Minimum numeric value hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum numeric value hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Renderer-neutral allowed-value hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub one_of: Vec<Value>,
}

/// Semantic UI action descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiAction {
    /// Semantic action id.
    pub id: UiActionId,
    /// Optional action payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// Whether clients should present the action as disabled.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
}

/// Semantic UI action request kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiActionKind {
    /// Submit form values or commit an action.
    Submit,
    /// Reset local or owner-managed form state.
    Reset,
    /// Ask the owner to validate current values without committing them.
    Validate,
    /// Cancel a pending interaction.
    Cancel,
}

/// Transport-neutral form values keyed by field id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UiFormValues(pub Map<String, Value>);

/// Transport-neutral action request emitted by a UI client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiActionRequest {
    /// Request correlation id.
    pub request_id: UiActionRequestId,
    /// Surface that owns or routed the action.
    pub surface_id: UiSurfaceId,
    /// Semantic action id.
    pub action_id: UiActionId,
    /// Optional node that emitted the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<UiNodeId>,
    /// Semantic action request kind.
    pub kind: UiActionKind,
    /// Optional form values sent with submit or validate requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<UiFormValues>,
    /// Optional non-form action metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}

/// UI action result state authored by the action owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiActionResultState {
    /// The owner accepted and applied the action.
    Accepted,
    /// The owner rejected the action, commonly with validation details.
    Rejected,
    /// The owner deferred completion and will resolve it asynchronously.
    Deferred,
    /// The owner failed to process the action.
    Error,
}

/// Optional owner-authored UI tree update reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiTreeUpdateRef {
    /// Reference to a patch that clients may fetch or apply through their transport.
    Patch {
        /// Opaque patch reference id.
        ref_id: String,
    },
    /// Reference to a replacement tree that clients may fetch or apply through their transport.
    Replacement {
        /// Opaque replacement reference id.
        ref_id: String,
    },
}

/// Field-level validation messages keyed by field id.
pub type UiFieldErrors = BTreeMap<String, Vec<String>>;

/// Action result identity, outcome, and owner-authored validation details.
///
/// Validation results are authoritative only when returned by the action owner,
/// host, or plugin. Clients may use hints for preflight presentation, but must
/// not treat normalized values or validation messages as client-side authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiActionResult {
    /// Request correlation id.
    pub request_id: UiActionRequestId,
    /// Surface that owns or routed the action.
    pub surface_id: UiSurfaceId,
    /// Semantic action id.
    pub action_id: UiActionId,
    /// Optional node that emitted the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<UiNodeId>,
    /// Owner-authored action result state.
    pub state: UiActionResultState,
    /// Owner-authored field validation errors keyed by field id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_errors: UiFieldErrors,
    /// Owner-authored form-level validation errors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub form_errors: Vec<String>,
    /// Owner-authored warnings that do not reject the action.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Owner-authored normalized values returned after validation or submit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_values: Option<UiFormValues>,
    /// Optional reference to an owner-authored UI tree update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_update: Option<UiTreeUpdateRef>,
    /// Optional successful or deferred action payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    /// Optional owner-authored error detail for rejected or failed actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// UI contract validation error.
#[derive(Debug, Error, PartialEq)]
pub enum UiValidationError {
    /// Unknown primitive kind.
    #[error("unknown UI node kind `{kind}`")]
    UnknownKind {
        /// Unknown kind name.
        kind: String,
    },
    /// Required prop is missing.
    #[error("{kind:?} missing required prop `{prop}`")]
    MissingProp {
        /// Node kind.
        kind: UiNodeKind,
        /// Prop name.
        prop: &'static str,
    },
    /// Unknown prop is present.
    #[error("{kind:?} has unknown prop `{prop}`")]
    UnknownProp {
        /// Node kind.
        kind: UiNodeKind,
        /// Prop name.
        prop: String,
    },
    /// Prop value is invalid.
    #[error("{kind:?} has invalid prop `{prop}`: {reason}")]
    InvalidProp {
        /// Node kind.
        kind: UiNodeKind,
        /// Prop name.
        prop: String,
        /// Validation reason.
        reason: String,
    },
    /// Required slot is missing.
    #[error("{kind:?} missing required slot `{slot}`")]
    MissingSlot {
        /// Node kind.
        kind: UiNodeKind,
        /// Slot name.
        slot: &'static str,
    },
    /// Unknown slot is present.
    #[error("{kind:?} has unknown slot `{slot}`")]
    UnknownSlot {
        /// Node kind.
        kind: UiNodeKind,
        /// Slot name.
        slot: String,
    },
    /// Required action is missing.
    #[error("{kind:?} missing required action")]
    MissingAction {
        /// Node kind.
        kind: UiNodeKind,
    },
    /// Required accessible label is missing.
    #[error("{kind:?} missing required label")]
    MissingLabel {
        /// Node kind.
        kind: UiNodeKind,
    },
    /// Stable node id is required.
    #[error("{kind:?} missing required stable node id: {reason}")]
    MissingId {
        /// Node kind.
        kind: UiNodeKind,
        /// Requirement reason.
        reason: &'static str,
    },
    /// Binding path is invalid.
    #[error("invalid bind path `{path}`: {reason}")]
    InvalidBindPath {
        /// Invalid path.
        path: String,
        /// Validation reason.
        reason: String,
    },
    /// Recursive node context.
    #[error("invalid node {id:?} ({kind:?}): {source}")]
    Node {
        /// Node id.
        id: Option<UiNodeId>,
        /// Node kind.
        kind: UiNodeKind,
        /// Nested error.
        source: Box<UiValidationError>,
    },
}

fn validate_node(node: &UiNode) -> Result<(), UiValidationError> {
    let schema = schema_for(node.kind);

    for required in schema.required_props {
        if !node.props.contains_key(required) {
            return Err(UiValidationError::MissingProp {
                kind: node.kind,
                prop: required,
            });
        }
    }

    for (prop, value) in &node.props {
        if !schema.allowed_props.contains(prop.as_str()) {
            return Err(UiValidationError::UnknownProp {
                kind: node.kind,
                prop: prop.clone(),
            });
        }
        validate_prop_value(node.kind, prop, value)?;
    }

    validate_prop_combinations(node)?;
    validate_stable_id(node)?;
    validate_required_action(node)?;
    validate_required_label(node)?;

    for required in schema.required_slots {
        if !node.slots.contains_key(required) {
            return Err(UiValidationError::MissingSlot {
                kind: node.kind,
                slot: required,
            });
        }
    }

    for (slot, children) in &node.slots {
        if !schema.allowed_slots.contains(slot.as_str()) {
            return Err(UiValidationError::UnknownSlot {
                kind: node.kind,
                slot: slot.clone(),
            });
        }
        for child in children {
            validate_child(child)?;
        }
    }

    for child in &node.children {
        validate_child(child)?;
    }

    Ok(())
}

fn validate_child(child: &UiChild) -> Result<(), UiValidationError> {
    match child {
        UiChild::Conditional(conditional) => validate_conditional(conditional),
        UiChild::Node(node) => node.validate(),
        UiChild::BindList(bind_list) => validate_bind_list(bind_list),
        UiChild::BindIf(bind_if) => validate_bind_if(bind_if),
    }
}

fn validate_conditional(conditional: &UiConditional) -> Result<(), UiValidationError> {
    match conditional {
        UiConditional::When { condition: _, node }
        | UiConditional::Hidden { condition: _, node } => node.validate(),
    }
}

fn validate_bind_list(bind_list: &UiBindList) -> Result<(), UiValidationError> {
    match bind_list {
        UiBindList::BindList {
            source,
            item_template,
            empty_template,
            ..
        } => {
            validate_bind_path(source)?;
            item_template.validate()?;
            if let Some(template) = empty_template {
                template.validate()?;
            }
            Ok(())
        }
    }
}

fn validate_bind_if(bind_if: &UiBindIf) -> Result<(), UiValidationError> {
    match bind_if {
        UiBindIf::BindIf { path, node } => {
            validate_bind_path(path)?;
            node.validate()
        }
    }
}

fn validate_prop_value(
    kind: UiNodeKind,
    prop: &str,
    value: &Value,
) -> Result<(), UiValidationError> {
    if let Some(path) = value.get("$bind").and_then(Value::as_str) {
        validate_bind_path(path)?;
    }

    if value.get("$bind").is_some() {
        let object = value
            .as_object()
            .ok_or_else(|| UiValidationError::InvalidProp {
                kind,
                prop: prop.to_string(),
                reason: "bind value must be an object".to_string(),
            })?;
        if object.len() != 1 {
            return Err(UiValidationError::InvalidProp {
                kind,
                prop: prop.to_string(),
                reason: "bind value may only contain $bind".to_string(),
            });
        }
        if !object.get("$bind").is_some_and(Value::is_string) {
            return Err(UiValidationError::InvalidProp {
                kind,
                prop: prop.to_string(),
                reason: "$bind value must be a string".to_string(),
            });
        }
    }

    if let Some(dynamic_kind) = value.get("$kind").and_then(Value::as_str) {
        match dynamic_kind {
            "responsive" => {
                serde_json::from_value::<UiResponsiveValue>(value.clone()).map_err(|error| {
                    UiValidationError::InvalidProp {
                        kind,
                        prop: prop.to_string(),
                        reason: error.to_string(),
                    }
                })?;
                validate_token_value(kind, prop, value)?;
            }
            other => {
                return Err(UiValidationError::InvalidProp {
                    kind,
                    prop: prop.to_string(),
                    reason: format!("unknown dynamic value kind `{other}`"),
                });
            }
        }
    } else {
        validate_token_value(kind, prop, value)?;
    }

    match (kind, prop) {
        (UiNodeKind::FormField, "schema") => {
            let schema = deserialize_prop::<UiFieldSchema>(kind, prop, value)?;
            validate_field_schema(kind, prop, &schema)?;
        }
        (
            UiNodeKind::TextInput
            | UiNodeKind::Textarea
            | UiNodeKind::Checkbox
            | UiNodeKind::Select,
            "validation",
        ) => {
            deserialize_prop::<UiFieldValidationHints>(kind, prop, value)?;
        }
        (_, "disabled" | "loading") => {
            if !value.is_boolean() && value.get("$bind").is_none() {
                return Err(UiValidationError::InvalidProp {
                    kind,
                    prop: prop.to_string(),
                    reason: "value must be a boolean".to_string(),
                });
            }
        }
        (_, "error") => validate_error_prop(kind, prop, value)?,
        _ => {}
    }

    Ok(())
}

fn deserialize_prop<T>(kind: UiNodeKind, prop: &str, value: &Value) -> Result<T, UiValidationError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value::<T>(value.clone()).map_err(|error| UiValidationError::InvalidProp {
        kind,
        prop: prop.to_string(),
        reason: error.to_string(),
    })
}

fn validate_field_schema(
    kind: UiNodeKind,
    prop: &str,
    schema: &UiFieldSchema,
) -> Result<(), UiValidationError> {
    if schema.name.trim().is_empty() {
        return Err(UiValidationError::InvalidProp {
            kind,
            prop: prop.to_string(),
            reason: "schema name cannot be empty".to_string(),
        });
    }

    if schema.label.trim().is_empty() {
        return Err(UiValidationError::InvalidProp {
            kind,
            prop: prop.to_string(),
            reason: "schema label cannot be empty".to_string(),
        });
    }

    if schema.kind == UiFieldKind::Select && schema.options.is_empty() {
        return Err(UiValidationError::InvalidProp {
            kind,
            prop: prop.to_string(),
            reason: "select schema requires options".to_string(),
        });
    }

    if schema.kind != UiFieldKind::Select && !schema.options.is_empty() {
        return Err(UiValidationError::InvalidProp {
            kind,
            prop: prop.to_string(),
            reason: "only select schema may define options".to_string(),
        });
    }

    Ok(())
}

fn validate_error_prop(
    kind: UiNodeKind,
    prop: &str,
    value: &Value,
) -> Result<(), UiValidationError> {
    if value.get("$bind").is_some() || value.is_string() || value.is_null() {
        return Ok(());
    }

    if value
        .as_object()
        .and_then(|object| object.get("message"))
        .is_some_and(Value::is_string)
    {
        return Ok(());
    }

    Err(UiValidationError::InvalidProp {
        kind,
        prop: prop.to_string(),
        reason: "error must be a string or object with a string message".to_string(),
    })
}

fn validate_prop_combinations(node: &UiNode) -> Result<(), UiValidationError> {
    if node.props.contains_key("default") {
        for controlled_prop in ["value", "checked", "selected"] {
            if node.props.contains_key(controlled_prop) {
                return Err(UiValidationError::InvalidProp {
                    kind: node.kind,
                    prop: "default".to_string(),
                    reason: format!("default cannot be used with `{controlled_prop}`"),
                });
            }
        }
    }

    if node.kind == UiNodeKind::FormField {
        let schema = node
            .props
            .get("schema")
            .map(|value| deserialize_prop::<UiFieldSchema>(node.kind, "schema", value))
            .transpose()?;
        if let (Some(schema), Some(default)) = (schema, node.props.get("default")) {
            match &schema.default {
                Some(schema_default) if schema_default == default => {}
                Some(_) => {
                    return Err(UiValidationError::InvalidProp {
                        kind: node.kind,
                        prop: "default".to_string(),
                        reason: "default must match schema default".to_string(),
                    });
                }
                None => {
                    return Err(UiValidationError::InvalidProp {
                        kind: node.kind,
                        prop: "default".to_string(),
                        reason: "form_field default must be declared in schema".to_string(),
                    });
                }
            }
        }
    }

    Ok(())
}

fn validate_stable_id(node: &UiNode) -> Result<(), UiValidationError> {
    let reason = if matches!(
        node.kind,
        UiNodeKind::Form | UiNodeKind::FormSection | UiNodeKind::FormField
    ) {
        Some("forms and form fields require stable identity")
    } else if matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::IconButton | UiNodeKind::MenuItem
    ) {
        Some("action feedback requires stable identity")
    } else if matches!(
        node.kind,
        UiNodeKind::TextInput | UiNodeKind::Textarea | UiNodeKind::Checkbox | UiNodeKind::Select
    ) && node
        .props
        .keys()
        .any(|prop| matches!(prop.as_str(), "value" | "checked" | "selected" | "default"))
    {
        Some("field state requires stable identity")
    } else {
        None
    };

    if let Some(reason) = reason {
        if node.id.as_ref().is_none_or(|id| id.0.trim().is_empty()) {
            return Err(UiValidationError::MissingId {
                kind: node.kind,
                reason,
            });
        }
    }

    Ok(())
}

fn validate_token_value(
    kind: UiNodeKind,
    prop: &str,
    value: &Value,
) -> Result<(), UiValidationError> {
    match prop {
        "gap" => validate_token_values::<UiSpaceToken>(kind, prop, value),
        "tone" => validate_token_values::<UiColorToken>(kind, prop, value),
        _ => Ok(()),
    }
}

fn validate_token_values<T>(
    kind: UiNodeKind,
    prop: &str,
    value: &Value,
) -> Result<(), UiValidationError>
where
    T: for<'de> Deserialize<'de>,
{
    if value.get("$bind").is_some() {
        return Ok(());
    }

    if value.get("$kind").and_then(Value::as_str) == Some("responsive") {
        if let Some(width) = value.get("width").and_then(Value::as_object) {
            for token in width.values() {
                validate_one_token::<T>(kind, prop, token)?;
            }
        }
        if let Some(height) = value.get("height").and_then(Value::as_object) {
            for token in height.values() {
                validate_one_token::<T>(kind, prop, token)?;
            }
        }
        return Ok(());
    }

    validate_one_token::<T>(kind, prop, value)
}

fn validate_one_token<T>(
    kind: UiNodeKind,
    prop: &str,
    value: &Value,
) -> Result<(), UiValidationError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value::<T>(value.clone())
        .map(|_| ())
        .map_err(|error| UiValidationError::InvalidProp {
            kind,
            prop: prop.to_string(),
            reason: error.to_string(),
        })
}

fn validate_required_action(node: &UiNode) -> Result<(), UiValidationError> {
    if matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::IconButton | UiNodeKind::MenuItem
    ) && !node.props.contains_key("action")
    {
        return Err(UiValidationError::MissingAction { kind: node.kind });
    }
    Ok(())
}

fn validate_required_label(node: &UiNode) -> Result<(), UiValidationError> {
    if !matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::IconButton | UiNodeKind::MenuItem
    ) {
        return Ok(());
    }

    match node.props.get("label").and_then(Value::as_str) {
        Some(label) if !label.trim().is_empty() => Ok(()),
        _ => Err(UiValidationError::MissingLabel { kind: node.kind }),
    }
}

fn validate_bind_path(path: &str) -> Result<(), UiValidationError> {
    if path.is_empty() {
        return Err(UiValidationError::InvalidBindPath {
            path: path.to_string(),
            reason: "path cannot be empty".to_string(),
        });
    }

    if path.starts_with('/') || path.starts_with("@/") {
        return Ok(());
    }

    Err(UiValidationError::InvalidBindPath {
        path: path.to_string(),
        reason: "path must start with `/` or `@/`".to_string(),
    })
}

fn schema_for(kind: UiNodeKind) -> UiNodeSchema {
    match kind {
        UiNodeKind::Stack => schema(
            &["direction", "gap", "align", "justify"],
            &["direction"],
            &[],
            &[],
        ),
        UiNodeKind::Inline => schema(&["gap", "align", "justify"], &[], &[], &[]),
        UiNodeKind::Form => schema(&["action", "disabled", "loading", "error"], &[], &[], &[]),
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
        UiNodeKind::Panel => schema(&["title", "tone"], &[], &[], &[]),
        UiNodeKind::ScrollArea => schema(&["height"], &[], &[], &[]),
        UiNodeKind::Text => schema(&["text", "tone", "variant"], &["text"], &[], &[]),
        UiNodeKind::Icon => schema(&["icon", "label", "tone"], &["icon"], &[], &[]),
        UiNodeKind::Badge => schema(&["label", "tone"], &["label"], &[], &[]),
        UiNodeKind::StatusDot => schema(&["label", "tone"], &["label"], &[], &[]),
        UiNodeKind::EmptyState => schema(
            &["title", "description", "icon", "action"],
            &["title"],
            &[],
            &[],
        ),
        UiNodeKind::List => schema(&["aria_label"], &[], &[], &[]),
        UiNodeKind::ListItem => schema(
            &["value", "selected"],
            &[],
            &["title", "subtitle", "meta", "actions"],
            &["title"],
        ),
        UiNodeKind::Tree => schema(&["aria_label"], &[], &[], &[]),
        UiNodeKind::TreeItem => schema(
            &["value", "expanded", "selected"],
            &[],
            &["title", "children", "actions"],
            &["title"],
        ),
        UiNodeKind::Table => schema(&["columns"], &["columns"], &[], &[]),
        UiNodeKind::Button => schema(&["label", "action", "tone", "variant"], &[], &[], &[]),
        UiNodeKind::IconButton => schema(
            &["label", "icon", "action", "tone", "variant"],
            &["icon"],
            &[],
            &[],
        ),
        UiNodeKind::Menu => schema(&["label"], &[], &["items"], &["items"]),
        UiNodeKind::MenuItem => schema(&["label", "action", "icon"], &[], &[], &[]),
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
            ],
            &["name", "label"],
            &["options"],
            &["options"],
        ),
        UiNodeKind::SelectOption => schema(
            &["value", "label", "disabled"],
            &["value", "label"],
            &[],
            &[],
        ),
        UiNodeKind::TerminalView => schema(&["session_id", "title"], &["session_id"], &[], &[]),
        UiNodeKind::ConnectionCodeView => schema(&["code", "label"], &["code"], &[], &[]),
    }
}

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

struct UiNodeSchema {
    allowed_props: BTreeSet<&'static str>,
    required_props: Vec<&'static str>,
    allowed_slots: BTreeSet<&'static str>,
    required_slots: Vec<&'static str>,
}

fn is_false(value: &bool) -> bool {
    !*value
}
