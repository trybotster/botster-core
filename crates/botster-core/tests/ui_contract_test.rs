//! UI contract serialization and validation tests.

use std::collections::{BTreeMap, BTreeSet};

use botster_core::ui::{
    validate_ui_node, validate_ui_node_with_capabilities, UiActionId, UiActionKind,
    UiActionRequest, UiActionResult, UiActionResultState, UiBind, UiBindIf, UiBindList,
    UiCapabilityFallback, UiCapabilitySet, UiChild, UiCondition, UiConditional,
    UiDialogPresentation, UiFieldErrors, UiFieldKind, UiFieldOption, UiFieldSchema,
    UiFieldValidationHints, UiFormValues, UiHeightClass, UiKeyboardCapability, UiNode, UiNodeId,
    UiNodeKind, UiPointer, UiResponsiveHeight, UiResponsiveValue, UiResponsiveWidth, UiSurfaceId,
    UiTreeUpdateRef, UiValidationError, UiWidthClass,
};
use botster_core::{RequestId, UiAction};
use serde_json::{json, Map, Value};

fn node(kind: UiNodeKind, props: Value) -> UiNode {
    UiNode {
        kind,
        id: Some(UiNodeId(format!("{kind:?}").to_lowercase())),
        props: props.as_object().cloned().unwrap_or_default(),
        children: Vec::new(),
        slots: BTreeMap::new(),
    }
}

fn text_node(value: &str) -> UiNode {
    node(UiNodeKind::Text, json!({ "text": value }))
}

fn text(value: &str) -> UiChild {
    UiChild::Node(Box::new(text_node(value)))
}

fn idless_node(kind: UiNodeKind, props: Value) -> UiNode {
    UiNode {
        kind,
        id: None,
        props: props.as_object().cloned().unwrap_or_default(),
        children: Vec::new(),
        slots: BTreeMap::new(),
    }
}

fn assert_error_contains(node: UiNode, expected: &str) {
    let message = node
        .validate()
        .expect_err("node should fail validation")
        .to_string();
    assert!(
        message.contains(expected),
        "expected `{message}` to contain `{expected}`"
    );
}

fn rich_capabilities() -> UiCapabilitySet {
    UiCapabilitySet {
        width_classes: BTreeMap::from([
            (UiWidthClass::Compact, ()),
            (UiWidthClass::Regular, ()),
            (UiWidthClass::Expanded, ()),
        ])
        .into_keys()
        .collect(),
        height_classes: BTreeMap::from([
            (UiHeightClass::Short, ()),
            (UiHeightClass::Regular, ()),
            (UiHeightClass::Tall, ()),
        ])
        .into_keys()
        .collect(),
        pointer: UiPointer::Fine,
        keyboard: UiKeyboardCapability {
            text_entry: true,
            shortcuts: true,
            focus_traversal: true,
        },
        hover: true,
        clipboard: true,
        context_menu: true,
        dialog_presentations: BTreeMap::from([
            (UiDialogPresentation::Inline, ()),
            (UiDialogPresentation::Overlay, ()),
            (UiDialogPresentation::Sheet, ()),
            (UiDialogPresentation::Fullscreen, ()),
        ])
        .into_keys()
        .collect(),
        table: true,
        terminal_selection: true,
        qr_code: true,
        rich_color: true,
        fallbacks: BTreeSet::new(),
    }
}

#[test]
fn ui_node_serializes_minimal_and_populated_wire_shape() {
    let minimal = UiNode {
        kind: UiNodeKind::Stack,
        id: None,
        props: Map::new(),
        children: Vec::new(),
        slots: BTreeMap::new(),
    };
    assert_eq!(
        serde_json::to_value(&minimal).expect("serialize minimal node"),
        json!({ "type": "stack" })
    );

    let mut slots = BTreeMap::new();
    slots.insert("title".to_string(), vec![text("Row title")]);

    let node = UiNode {
        kind: UiNodeKind::ListItem,
        id: Some(UiNodeId("ticket-row".to_string())),
        props: Map::from_iter([("value".to_string(), json!("ticket_123"))]),
        children: vec![text("Child")],
        slots,
    };

    let value = serde_json::to_value(&node).expect("serialize populated node");
    assert_eq!(
        value,
        json!({
            "type": "list_item",
            "id": "ticket-row",
            "props": { "value": "ticket_123" },
            "children": [{
                "type": "text",
                "id": "text",
                "props": { "text": "Child" }
            }],
            "slots": {
                "title": [{
                    "type": "text",
                    "id": "text",
                    "props": { "text": "Row title" }
                }]
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<UiNode>(value).expect("deserialize populated node"),
        node
    );
    node.validate().expect("populated node should validate");
}

#[test]
fn required_props_fail_clearly() {
    assert_error_contains(node(UiNodeKind::Stack, json!({})), "direction");
    assert_error_contains(node(UiNodeKind::Text, json!({})), "text");
    assert_error_contains(
        node(UiNodeKind::Button, json!({ "label": "Run" })),
        "action",
    );
    assert_error_contains(
        node(UiNodeKind::SelectOption, json!({ "label": "Open" })),
        "value",
    );
    assert_error_contains(
        node(UiNodeKind::SelectOption, json!({ "value": "open" })),
        "label",
    );
}

#[test]
fn required_slots_fail_clearly() {
    assert_error_contains(node(UiNodeKind::ListItem, json!({})), "title");
    assert_error_contains(node(UiNodeKind::TreeItem, json!({})), "title");
    assert_error_contains(node(UiNodeKind::Menu, json!({})), "items");
    assert_error_contains(
        node(UiNodeKind::Dialog, json!({ "title": "Confirm" })),
        "body",
    );
}

#[test]
fn renderer_specific_props_are_rejected() {
    for (kind, props, expected) in [
        (
            UiNodeKind::Stack,
            json!({ "direction": "vertical", "className": "flex" }),
            "className",
        ),
        (UiNodeKind::Panel, json!({ "padding": "lg" }), "padding"),
        (UiNodeKind::Panel, json!({ "radius": "xl" }), "radius"),
        (
            UiNodeKind::Button,
            json!({ "label": "Run", "action": { "id": "run" }, "leadingIcon": "play" }),
            "leadingIcon",
        ),
        (
            UiNodeKind::Button,
            json!({ "label": "Run", "action": { "id": "run" }, "leading_icon": "play" }),
            "leading_icon",
        ),
        (
            UiNodeKind::Button,
            json!({ "label": "Run", "action": { "id": "run" }, "disabled": true }),
            "disabled",
        ),
        (UiNodeKind::Tree, json!({ "density": "compact" }), "density"),
        (
            UiNodeKind::Text,
            json!({ "text": "Hi", "foo": true }),
            "foo",
        ),
        (
            UiNodeKind::Text,
            json!({ "text": "Hi", "when": { "$kind": "viewport", "viewport": "regular" } }),
            "when",
        ),
    ] {
        assert_error_contains(node(kind, props), expected);
    }
}

#[test]
fn ui_node_v1_primitive_inventory_is_explicit() {
    let primitives = [
        UiNodeKind::Stack,
        UiNodeKind::Inline,
        UiNodeKind::Panel,
        UiNodeKind::ScrollArea,
        UiNodeKind::Text,
        UiNodeKind::Icon,
        UiNodeKind::Badge,
        UiNodeKind::StatusDot,
        UiNodeKind::EmptyState,
        UiNodeKind::List,
        UiNodeKind::ListItem,
        UiNodeKind::Tree,
        UiNodeKind::TreeItem,
        UiNodeKind::Table,
        UiNodeKind::Button,
        UiNodeKind::IconButton,
        UiNodeKind::Menu,
        UiNodeKind::MenuItem,
        UiNodeKind::Dialog,
        UiNodeKind::Form,
        UiNodeKind::FormSection,
        UiNodeKind::FormField,
        UiNodeKind::TextInput,
        UiNodeKind::Textarea,
        UiNodeKind::Checkbox,
        UiNodeKind::Select,
        UiNodeKind::SelectOption,
        UiNodeKind::TerminalView,
        UiNodeKind::ConnectionCodeView,
    ];

    let wire_names: Vec<_> = primitives
        .into_iter()
        .map(|kind| serde_json::to_value(kind).expect("serialize kind"))
        .collect();

    assert_eq!(
        wire_names,
        vec![
            json!("stack"),
            json!("inline"),
            json!("panel"),
            json!("scroll_area"),
            json!("text"),
            json!("icon"),
            json!("badge"),
            json!("status_dot"),
            json!("empty_state"),
            json!("list"),
            json!("list_item"),
            json!("tree"),
            json!("tree_item"),
            json!("table"),
            json!("button"),
            json!("icon_button"),
            json!("menu"),
            json!("menu_item"),
            json!("dialog"),
            json!("form"),
            json!("form_section"),
            json!("form_field"),
            json!("text_input"),
            json!("textarea"),
            json!("checkbox"),
            json!("select"),
            json!("select_option"),
            json!("terminal_view"),
            json!("connection_code_view"),
        ]
    );
}

#[test]
fn form_and_form_section_round_trip_wire_shape() {
    let mut form = node(
        UiNodeKind::Form,
        json!({
            "action": { "id": "project-pipelines.ticket.save" },
            "disabled": false,
            "loading": true,
            "error": { "message": "Save failed" }
        }),
    );
    form.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::FormSection,
        json!({
            "title": "Ticket",
            "description": "Visible in every renderer",
            "disabled": false,
            "loading": false,
            "error": "Section unavailable"
        }),
    ))));

    let value = serde_json::to_value(&form).expect("serialize form");
    assert_eq!(value["type"], "form");
    assert_eq!(value["children"][0]["type"], "form_section");
    assert_eq!(
        serde_json::from_value::<UiNode>(value).expect("deserialize form"),
        form
    );
    form.validate().expect("form tree should validate");

    assert_error_contains(idless_node(UiNodeKind::Form, json!({})), "stable node id");
    assert_error_contains(node(UiNodeKind::FormSection, json!({})), "title");
}

#[test]
fn form_field_schema_round_trips_for_v1_field_kinds() {
    let schemas = [
        UiFieldSchema {
            kind: UiFieldKind::Text,
            name: "title".to_string(),
            label: "Title".to_string(),
            description: Some("Short summary".to_string()),
            placeholder: Some("Ticket title".to_string()),
            required: true,
            default: Some(json!("Draft")),
            validation: Some(UiFieldValidationHints {
                min_length: Some(3),
                max_length: Some(120),
                pattern: Some("^[[:print:]]+$".to_string()),
                ..Default::default()
            }),
            options: Vec::new(),
        },
        UiFieldSchema {
            kind: UiFieldKind::Textarea,
            name: "body".to_string(),
            label: "Body".to_string(),
            description: None,
            placeholder: Some("Details".to_string()),
            required: false,
            default: Some(json!("")),
            validation: None,
            options: Vec::new(),
        },
        UiFieldSchema {
            kind: UiFieldKind::Checkbox,
            name: "notify".to_string(),
            label: "Notify watchers".to_string(),
            description: None,
            placeholder: None,
            required: false,
            default: Some(json!(true)),
            validation: None,
            options: Vec::new(),
        },
        UiFieldSchema {
            kind: UiFieldKind::Select,
            name: "status".to_string(),
            label: "Status".to_string(),
            description: Some("Workflow state".to_string()),
            placeholder: None,
            required: true,
            default: Some(json!("open")),
            validation: Some(UiFieldValidationHints {
                one_of: vec![json!("open"), json!("closed")],
                ..Default::default()
            }),
            options: vec![
                UiFieldOption {
                    value: json!("open"),
                    label: "Open".to_string(),
                    disabled: false,
                },
                UiFieldOption {
                    value: json!("closed"),
                    label: "Closed".to_string(),
                    disabled: true,
                },
            ],
        },
    ];

    for schema in schemas {
        let field = node(
            UiNodeKind::FormField,
            json!({
                "schema": schema,
                "default": schema.default,
                "disabled": false,
                "loading": false,
                "error": null
            }),
        );
        field.validate().expect("form field should validate");

        let value = serde_json::to_value(&field).expect("serialize field");
        assert_eq!(
            serde_json::from_value::<UiNode>(value).expect("deserialize field"),
            field
        );
    }
}

#[test]
fn form_field_schema_rejects_invalid_v1_field_shapes() {
    for (schema, expected) in [
        (
            json!({
                "kind": "text",
                "name": "   ",
                "label": "Title"
            }),
            "schema name cannot be empty",
        ),
        (
            json!({
                "kind": "text",
                "name": "title",
                "label": "   "
            }),
            "schema label cannot be empty",
        ),
        (
            json!({
                "kind": "select",
                "name": "status",
                "label": "Status"
            }),
            "select schema requires options",
        ),
        (
            json!({
                "kind": "text",
                "name": "title",
                "label": "Title",
                "options": [{ "value": "draft", "label": "Draft" }]
            }),
            "only select schema may define options",
        ),
    ] {
        assert_error_contains(
            node(UiNodeKind::FormField, json!({ "schema": schema })),
            expected,
        );
    }
}

#[test]
fn error_prop_rejects_non_renderer_neutral_shapes() {
    for props in [
        json!({ "error": 42 }),
        json!({ "error": { "code": "failed" } }),
        json!({ "error": { "message": false } }),
    ] {
        assert_error_contains(
            node(UiNodeKind::Form, props),
            "error must be a string or object with a string message",
        );
    }
}

#[test]
fn field_schema_accepts_metadata_without_renderer_props() {
    for (kind, props) in [
        (
            UiNodeKind::TextInput,
            json!({
                "name": "title",
                "label": "Title",
                "description": "Visible help",
                "placeholder": "Ticket title",
                "required": true,
                "default": "Draft",
                "disabled": false,
                "loading": false,
                "error": { "message": "Required" },
                "validation": { "minLength": 3, "maxLength": 120 }
            }),
        ),
        (
            UiNodeKind::Textarea,
            json!({
                "name": "body",
                "label": "Body",
                "description": "Markdown allowed",
                "placeholder": "Details",
                "required": false,
                "default": "",
                "disabled": false,
                "loading": false,
                "error": "Too long",
                "validation": { "maxLength": 1000 }
            }),
        ),
        (
            UiNodeKind::Checkbox,
            json!({
                "name": "notify",
                "label": "Notify watchers",
                "description": "Sends a generic notification",
                "required": false,
                "default": true,
                "disabled": false,
                "loading": false,
                "error": null,
                "validation": {}
            }),
        ),
    ] {
        node(kind, props)
            .validate()
            .expect("input metadata should validate");
    }

    let mut select = node(
        UiNodeKind::Select,
        json!({
            "name": "status",
            "label": "Status",
            "description": "Workflow state",
            "required": true,
            "default": "open",
            "disabled": false,
            "loading": false,
            "error": null,
            "validation": { "oneOf": ["open", "closed"] }
        }),
    );
    select.slots.insert(
        "options".to_string(),
        vec![UiChild::Node(Box::new(node(
            UiNodeKind::SelectOption,
            json!({ "value": "open", "label": "Open", "disabled": false }),
        )))],
    );
    select
        .validate()
        .expect("select metadata and option slot should validate");
}

#[test]
fn field_schema_validation_hints_are_metadata_not_policy() {
    let hints = UiFieldValidationHints {
        min_length: Some(10),
        max_length: Some(3),
        pattern: Some("[".to_string()),
        min: Some(10.0),
        max: Some(1.0),
        one_of: vec![json!("a"), json!({ "structured": true })],
    };

    node(
        UiNodeKind::TextInput,
        json!({
            "name": "code",
            "label": "Code",
            "default": "x",
            "validation": hints
        }),
    )
    .validate()
    .expect("core validates hint shape but not business policy");

    assert_error_contains(
        node(
            UiNodeKind::TextInput,
            json!({
                "name": "code",
                "label": "Code",
                "validation": { "minLength": "long" }
            }),
        ),
        "validation",
    );
}

#[test]
fn field_defaults_are_representable_for_each_v1_field_kind() {
    for (kind, props, controlled_prop) in [
        (
            UiNodeKind::TextInput,
            json!({ "name": "title", "label": "Title", "default": "Draft" }),
            "value",
        ),
        (
            UiNodeKind::Textarea,
            json!({ "name": "body", "label": "Body", "default": "Details" }),
            "value",
        ),
        (
            UiNodeKind::Checkbox,
            json!({ "name": "notify", "label": "Notify", "default": true }),
            "checked",
        ),
        (
            UiNodeKind::Select,
            json!({ "name": "status", "label": "Status", "default": "open" }),
            "selected",
        ),
    ] {
        let mut field = node(kind, props);
        if kind == UiNodeKind::Select {
            field.slots.insert(
                "options".to_string(),
                vec![UiChild::Node(Box::new(node(
                    UiNodeKind::SelectOption,
                    json!({ "value": "open", "label": "Open" }),
                )))],
            );
        }
        field.validate().expect("default should validate");

        field
            .props
            .insert(controlled_prop.to_string(), json!("controlled"));
        assert_error_contains(field, "default cannot be used");
    }

    let schema = UiFieldSchema {
        kind: UiFieldKind::Text,
        name: "title".to_string(),
        label: "Title".to_string(),
        description: None,
        placeholder: None,
        required: false,
        default: Some(json!("Draft")),
        validation: None,
        options: Vec::new(),
    };

    node(
        UiNodeKind::FormField,
        json!({ "schema": schema, "default": "Draft" }),
    )
    .validate()
    .expect("form_field node default may mirror schema default");

    assert_error_contains(
        node(
            UiNodeKind::FormField,
            json!({ "schema": schema, "default": "Different" }),
        ),
        "default must match schema default",
    );
}

#[test]
fn explicit_value_checked_or_selected_marks_field_controlled() {
    for (kind, props) in [
        (
            UiNodeKind::TextInput,
            json!({ "name": "title", "label": "Title", "value": "Plugin owned" }),
        ),
        (
            UiNodeKind::Textarea,
            json!({ "name": "body", "label": "Body", "value": "Plugin owned" }),
        ),
        (
            UiNodeKind::Checkbox,
            json!({ "name": "notify", "label": "Notify", "checked": true }),
        ),
    ] {
        node(kind, props)
            .validate()
            .expect("controlled field should validate with stable id");
    }

    let mut select = node(
        UiNodeKind::Select,
        json!({ "name": "status", "label": "Status", "selected": "open" }),
    );
    select.slots.insert(
        "options".to_string(),
        vec![UiChild::Node(Box::new(node(
            UiNodeKind::SelectOption,
            json!({ "value": "open", "label": "Open" }),
        )))],
    );
    select.validate().expect("selected alias should validate");
}

#[test]
fn renderer_local_fields_require_stable_node_ids() {
    assert_error_contains(
        idless_node(
            UiNodeKind::TextInput,
            json!({ "name": "title", "label": "Title", "default": "Draft" }),
        ),
        "stable node id",
    );
    assert_error_contains(
        idless_node(
            UiNodeKind::Checkbox,
            json!({ "name": "notify", "label": "Notify", "checked": false }),
        ),
        "stable node id",
    );

    idless_node(
        UiNodeKind::TextInput,
        json!({ "name": "title", "label": "Title" }),
    )
    .validate()
    .expect("static field metadata without state may omit id");
}

#[test]
fn form_field_and_action_state_props_validate() {
    let schema = UiFieldSchema {
        kind: UiFieldKind::Text,
        name: "title".to_string(),
        label: "Title".to_string(),
        description: None,
        placeholder: None,
        required: true,
        default: None,
        validation: None,
        options: Vec::new(),
    };

    node(
        UiNodeKind::FormField,
        json!({
            "schema": schema,
            "disabled": true,
            "loading": true,
            "error": { "message": "Unavailable" }
        }),
    )
    .validate()
    .expect("node-level state props should validate");

    let action = UiAction {
        id: UiActionId("save".to_string()),
        payload: None,
        disabled: true,
    };
    assert_eq!(
        serde_json::to_value(&action).expect("serialize action disabled")["disabled"],
        true
    );

    assert_error_contains(
        node(
            UiNodeKind::FormField,
            json!({
                "schema": schema,
                "disabled": "yes"
            }),
        ),
        "disabled",
    );
}

#[test]
fn action_emitters_require_stable_node_ids_for_pending_feedback() {
    assert_error_contains(
        idless_node(
            UiNodeKind::Button,
            json!({ "label": "Save", "action": { "id": "save" } }),
        ),
        "stable node id",
    );

    node(
        UiNodeKind::Button,
        json!({ "label": "Save", "action": { "id": "save" } }),
    )
    .validate()
    .expect("action emitter with id should validate");
}

#[test]
fn unknown_ui_node_kind_is_rejected() {
    let err = serde_json::from_value::<UiNode>(json!({
        "type": "overlay",
        "props": {}
    }));
    assert!(err.is_err());
}

#[test]
fn renderer_specific_form_props_are_rejected() {
    for (kind, props, expected) in [
        (
            UiNodeKind::Form,
            json!({ "method": "post", "action": { "id": "save" } }),
            "method",
        ),
        (
            UiNodeKind::FormSection,
            json!({ "title": "Profile", "className": "gap-2" }),
            "className",
        ),
        (
            UiNodeKind::FormField,
            json!({
                "schema": {
                    "kind": "text",
                    "name": "title",
                    "label": "Title"
                },
                "component": "IonInput"
            }),
            "component",
        ),
    ] {
        assert_error_contains(node(kind, props), expected);
    }
}

#[test]
fn icon_button_requires_accessible_label() {
    assert_error_contains(
        node(
            UiNodeKind::IconButton,
            json!({ "icon": "play", "action": { "id": "run" } }),
        ),
        "label",
    );
    assert_error_contains(
        node(
            UiNodeKind::IconButton,
            json!({ "label": "", "icon": "play", "action": { "id": "run" } }),
        ),
        "label",
    );

    node(
        UiNodeKind::IconButton,
        json!({ "label": "Run", "icon": "play", "action": { "id": "run" } }),
    )
    .validate()
    .expect("labeled icon button should validate");
}

#[test]
fn binding_paths_serialize_exactly() {
    for path in ["/project-pipelines.ticket/ticket_123/title", "@/title"] {
        let bind = UiBind {
            path: path.to_string(),
        };
        let value = serde_json::to_value(&bind).expect("serialize bind");
        assert_eq!(value, json!({ "$bind": path }));
        assert_eq!(
            serde_json::from_value::<UiBind>(value).expect("deserialize bind"),
            bind
        );
    }

    let err = node(UiNodeKind::Text, json!({ "text": { "$bind": "title" } }))
        .validate()
        .expect_err("relative bind without @/ should fail");
    assert!(matches!(
        err,
        UiValidationError::Node {
            source,
            ..
        } if matches!(*source, UiValidationError::InvalidBindPath { .. })
    ));

    assert_error_contains(
        node(UiNodeKind::Text, json!({ "text": { "$bind": 123 } })),
        "$bind value must be a string",
    );
}

#[test]
fn bind_list_and_bind_if_wire_shapes_round_trip() {
    let bind_list = UiBindList::BindList {
        source: "/project-pipelines.ticket".to_string(),
        r#where: BTreeMap::from([("status".to_string(), json!("open"))]),
        item_template: Box::new(node(
            UiNodeKind::Text,
            json!({ "text": { "$bind": "@/title" } }),
        )),
        empty_template: Some(Box::new(node(
            UiNodeKind::EmptyState,
            json!({ "title": "No tickets" }),
        ))),
    };
    let value = serde_json::to_value(&bind_list).expect("serialize bind_list");
    assert_eq!(
        value,
        json!({
            "$kind": "bind_list",
            "source": "/project-pipelines.ticket",
            "where": { "status": "open" },
            "item_template": {
                "type": "text",
                "id": "text",
                "props": { "text": { "$bind": "@/title" } }
            },
            "empty_template": {
                "type": "empty_state",
                "id": "emptystate",
                "props": { "title": "No tickets" }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<UiBindList>(value).expect("deserialize bind_list"),
        bind_list
    );

    let bind_if = UiBindIf::BindIf {
        path: "@/active".to_string(),
        node: Box::new(node(UiNodeKind::Text, json!({ "text": "Active" }))),
    };
    let value = serde_json::to_value(&bind_if).expect("serialize bind_if");
    assert_eq!(value["$kind"], "bind_if");
    assert_eq!(value["path"], "@/active");
    assert_eq!(
        serde_json::from_value::<UiBindIf>(value).expect("deserialize bind_if"),
        bind_if
    );
}

#[test]
fn bind_list_filters_are_exact_top_level_fields() {
    let empty_field = UiBindList::BindList {
        source: "/project-pipelines.ticket".to_string(),
        r#where: BTreeMap::from([("".to_string(), json!("open"))]),
        item_template: Box::new(node(
            UiNodeKind::Text,
            json!({ "text": { "$bind": "@/title" } }),
        )),
        empty_template: None,
    };
    let mut parent = node(UiNodeKind::Stack, json!({ "direction": "vertical" }));
    parent.children.push(UiChild::BindList(empty_field));
    assert_error_contains(parent, "field cannot be empty");

    for field in ["ticket.status", "ticket/status"] {
        let bind_list = UiBindList::BindList {
            source: "/project-pipelines.ticket".to_string(),
            r#where: BTreeMap::from([(field.to_string(), json!("open"))]),
            item_template: Box::new(node(
                UiNodeKind::Text,
                json!({ "text": { "$bind": "@/title" } }),
            )),
            empty_template: None,
        };
        let mut parent = node(UiNodeKind::Stack, json!({ "direction": "vertical" }));
        parent.children.push(UiChild::BindList(bind_list));

        assert_error_contains(parent, "top-level");
    }

    let bind_list = UiBindList::BindList {
        source: "@/children".to_string(),
        r#where: BTreeMap::new(),
        item_template: Box::new(text_node("Child")),
        empty_template: None,
    };
    let mut parent = node(UiNodeKind::Stack, json!({ "direction": "vertical" }));
    parent.children.push(UiChild::BindList(bind_list));

    assert_error_contains(parent, "absolute entity family path");
}

#[test]
fn responsive_and_conditionals_wire_shapes_round_trip() {
    let responsive = UiResponsiveValue::Responsive {
        width: Some(UiResponsiveWidth {
            compact: Some(json!("vertical")),
            expanded: Some(json!("horizontal")),
            ..Default::default()
        }),
        height: Some(UiResponsiveHeight {
            short: Some(json!("sm")),
            tall: Some(json!("md")),
            ..Default::default()
        }),
    };
    let value = serde_json::to_value(&responsive).expect("serialize responsive");
    assert_eq!(
        value,
        json!({
            "$kind": "responsive",
            "width": {
                "compact": "vertical",
                "expanded": "horizontal"
            },
            "height": {
                "short": "sm",
                "tall": "md"
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<UiResponsiveValue>(value).expect("deserialize responsive"),
        responsive
    );

    let condition = UiCondition {
        width: Some(UiWidthClass::Compact),
        pointer: Some(UiPointer::Coarse),
        keyboard_occluded: Some(true),
        ..Default::default()
    };
    let conditional = UiConditional::Hidden {
        condition,
        node: Box::new(text_node("Metadata")),
    };
    let value = serde_json::to_value(&conditional).expect("serialize conditional");
    assert_eq!(
        value,
        json!({
            "$kind": "hidden",
            "condition": {
                "width": "compact",
                "pointer": "coarse",
                "keyboardOccluded": true
            },
            "node": {
                "type": "text",
                "id": "text",
                "props": { "text": "Metadata" }
            }
        })
    );
    assert_eq!(
        serde_json::from_value::<UiConditional>(value).expect("deserialize conditional"),
        conditional
    );

    let mut parent = node(
        UiNodeKind::Stack,
        json!({ "direction": { "$kind": "responsive", "width": { "compact": "vertical", "expanded": "horizontal" } } }),
    );
    parent
        .children
        .push(UiChild::Conditional(UiConditional::When {
            condition: UiCondition {
                height: Some(UiHeightClass::Tall),
                ..Default::default()
            },
            node: Box::new(text_node("Tall")),
        }));
    parent
        .children
        .push(UiChild::Conditional(UiConditional::When {
            condition: UiCondition::default(),
            node: Box::new(text_node("Always")),
        }));
    parent
        .validate()
        .expect("conditional child should validate");

    let unknown_child = serde_json::from_value::<UiChild>(json!({
        "$kind": "viewport",
        "viewport": "regular"
    }));
    assert!(unknown_child.is_err());
}

#[test]
fn ui_capability_set_serializes_renderer_neutral_wire_shape() {
    let capabilities = UiCapabilitySet {
        width_classes: BTreeMap::from([(UiWidthClass::Compact, ()), (UiWidthClass::Regular, ())])
            .into_keys()
            .collect(),
        height_classes: BTreeMap::from([(UiHeightClass::Regular, ())])
            .into_keys()
            .collect(),
        pointer: UiPointer::Coarse,
        keyboard: UiKeyboardCapability {
            text_entry: true,
            shortcuts: false,
            focus_traversal: true,
        },
        hover: false,
        clipboard: true,
        context_menu: false,
        dialog_presentations: BTreeMap::from([(UiDialogPresentation::Inline, ())])
            .into_keys()
            .collect(),
        table: false,
        terminal_selection: false,
        qr_code: false,
        rich_color: false,
        fallbacks: BTreeMap::from([
            (UiCapabilityFallback::TableAsList, ()),
            (UiCapabilityFallback::DialogInline, ()),
            (UiCapabilityFallback::ConnectionCodeText, ()),
        ])
        .into_keys()
        .collect(),
    };
    let value = serde_json::to_value(&capabilities).expect("serialize capabilities");

    assert_eq!(
        value,
        json!({
            "widthClasses": ["compact", "regular"],
            "heightClasses": ["regular"],
            "pointer": "coarse",
            "keyboard": {
                "textEntry": true,
                "focusTraversal": true
            },
            "clipboard": true,
            "dialogPresentations": ["inline"],
            "fallbacks": ["table_as_list", "dialog_inline", "connection_code_text"]
        })
    );
    assert_eq!(
        serde_json::from_value::<UiCapabilitySet>(value).expect("deserialize capabilities"),
        capabilities
    );
}

#[test]
fn capability_validation_accepts_supported_or_declared_downgrade_nodes() {
    let mut table = node(UiNodeKind::Table, json!({ "columns": ["title", "status"] }));
    table.children.push(text("Row"));
    rich_capabilities()
        .validate_node(&table)
        .expect("rich renderer supports table directly");

    let mut downgraded = rich_capabilities();
    downgraded.table = false;
    downgraded
        .fallbacks
        .insert(UiCapabilityFallback::TableAsList);
    validate_ui_node_with_capabilities(&table, &downgraded)
        .expect("declared table downgrade should pass");

    downgraded
        .fallbacks
        .remove(&UiCapabilityFallback::TableAsList);
    let err = validate_ui_node_with_capabilities(&table, &downgraded)
        .expect_err("missing table fallback should fail");
    assert!(matches!(
        err,
        UiValidationError::Node {
            source,
            ..
        } if matches!(*source, UiValidationError::UnsupportedCapability { capability: "table", .. })
    ));
}

#[test]
fn capability_validation_pins_dialog_terminal_qr_and_color_downgrades() {
    let mut capabilities = rich_capabilities();
    capabilities.dialog_presentations.clear();
    capabilities.terminal_selection = false;
    capabilities.qr_code = false;
    capabilities.rich_color = false;
    capabilities.fallbacks = BTreeMap::from([
        (UiCapabilityFallback::DialogInline, ()),
        (UiCapabilityFallback::TerminalSelectionDisabled, ()),
        (UiCapabilityFallback::ConnectionCodeText, ()),
        (UiCapabilityFallback::RichColorMuted, ()),
    ])
    .into_keys()
    .collect();

    let mut root = node(UiNodeKind::Stack, json!({ "direction": "vertical" }));
    let mut dialog = node(
        UiNodeKind::Dialog,
        json!({ "title": "Confirm", "presentation": "sheet" }),
    );
    dialog.slots.insert("body".to_string(), vec![text("Body")]);
    root.children.push(UiChild::Node(Box::new(dialog)));
    root.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::TerminalView,
        json!({ "session_id": "sess_1" }),
    ))));
    root.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::ConnectionCodeView,
        json!({ "code": "pairing-code" }),
    ))));
    root.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::Text,
        json!({ "text": "Status", "tone": "success" }),
    ))));

    validate_ui_node_with_capabilities(&root, &capabilities)
        .expect("declared downgrades should cover unsupported capabilities");

    capabilities
        .fallbacks
        .remove(&UiCapabilityFallback::TerminalSelectionDisabled);
    let err = validate_ui_node_with_capabilities(&root, &capabilities)
        .expect_err("missing terminal-selection fallback should fail");
    assert!(err.to_string().contains("terminalSelection"));
}

#[test]
fn capability_validation_pins_shortcut_hover_clipboard_and_context_menu_downgrades() {
    let mut capabilities = rich_capabilities();
    capabilities.keyboard.shortcuts = false;
    capabilities.hover = false;
    capabilities.clipboard = false;
    capabilities.context_menu = false;
    capabilities.fallbacks = BTreeMap::from([
        (UiCapabilityFallback::HoverPersistentHints, ()),
        (UiCapabilityFallback::ClipboardManual, ()),
        (UiCapabilityFallback::ContextMenuAsMenu, ()),
    ])
    .into_keys()
    .collect();

    let mut root = node(UiNodeKind::Stack, json!({ "direction": "vertical" }));
    root.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::Text,
        json!({
            "text": "Pairing code",
            "hover_label": "Visible hint when hover is unavailable",
            "copy_value": "pair-fixture"
        }),
    ))));
    root.children.push(UiChild::Node(Box::new(node(
        UiNodeKind::Button,
        json!({
            "label": "More",
            "action": { "id": "fixture.more" },
            "context_menu": [{ "id": "fixture.inspect" }]
        }),
    ))));

    validate_ui_node_with_capabilities(&root, &capabilities)
        .expect("declared hover/clipboard/context-menu fallbacks should pass");

    capabilities
        .fallbacks
        .remove(&UiCapabilityFallback::ContextMenuAsMenu);
    let err = validate_ui_node_with_capabilities(&root, &capabilities)
        .expect_err("missing context-menu fallback should fail");
    assert!(err.to_string().contains("contextMenu"));

    let shortcut = node(
        UiNodeKind::Button,
        json!({
            "label": "Run",
            "action": { "id": "fixture.run" },
            "shortcut": "mod+enter"
        }),
    );
    let err = validate_ui_node_with_capabilities(&shortcut, &capabilities)
        .expect_err("missing shortcut capability should fail");
    assert!(err.to_string().contains("keyboard.shortcuts"));

    capabilities.keyboard.shortcuts = true;
    validate_ui_node_with_capabilities(&shortcut, &capabilities)
        .expect("shortcut capability should permit shortcut metadata");
}

#[test]
fn capability_validation_keeps_controlled_and_renderer_local_state_expectations() {
    let mut capabilities = rich_capabilities();
    capabilities.keyboard.text_entry = false;

    let input = node(
        UiNodeKind::TextInput,
        json!({ "name": "title", "label": "Title", "value": "Owner authored" }),
    );

    let err = validate_ui_node_with_capabilities(&input, &capabilities)
        .expect_err("missing text-entry capability should fail");
    assert!(err.to_string().contains("textEntry"));

    capabilities.keyboard.text_entry = true;
    validate_ui_node_with_capabilities(&input, &capabilities)
        .expect("text entry capability should permit controlled text input");
}

#[test]
fn token_props_are_validated() {
    node(
        UiNodeKind::Stack,
        json!({ "direction": "vertical", "gap": "md" }),
    )
    .validate()
    .expect("valid spacing token should pass");

    node(UiNodeKind::Text, json!({ "text": "OK", "tone": "success" }))
        .validate()
        .expect("valid color token should pass");

    assert_error_contains(
        node(
            UiNodeKind::Stack,
            json!({ "direction": "vertical", "gap": "massive" }),
        ),
        "gap",
    );
    assert_error_contains(
        node(UiNodeKind::Text, json!({ "text": "OK", "tone": "brand" })),
        "tone",
    );
}

#[test]
fn ui_action_descriptor_serializes_semantic_id_and_payload() {
    let action = UiAction {
        id: UiActionId("project-pipelines.advance".to_string()),
        payload: Some(json!({ "ticket_id": "ticket_123" })),
        disabled: true,
    };
    assert_eq!(
        serde_json::to_value(&action).expect("serialize action"),
        json!({
            "id": "project-pipelines.advance",
            "payload": { "ticket_id": "ticket_123" },
            "disabled": true
        })
    );
}

#[test]
fn ui_action_submit_request_round_trips_form_values() {
    let request = UiActionRequest {
        request_id: RequestId("req_123".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
        action_id: UiActionId("project-pipelines.ticket.submit".to_string()),
        kind: UiActionKind::Submit,
        values: Some(UiFormValues(Map::from_iter([
            ("title".to_string(), json!("Fix checkout flow")),
            ("notify".to_string(), json!(true)),
            ("priority".to_string(), json!("high")),
        ]))),
        payload: Some(json!({ "ticket_id": "ticket_123" })),
    };
    let value = serde_json::to_value(&request).expect("serialize submit request");
    assert_eq!(
        value,
        json!({
            "request_id": "req_123",
            "surface_id": "project-pipelines.ticket.form",
            "action_id": "project-pipelines.ticket.submit",
            "node_id": "advance-button",
            "kind": "submit",
            "values": {
                "title": "Fix checkout flow",
                "notify": true,
                "priority": "high"
            },
            "payload": { "ticket_id": "ticket_123" }
        })
    );
    assert_eq!(
        serde_json::from_value::<UiActionRequest>(value).expect("deserialize submit request"),
        request
    );
}

#[test]
fn ui_action_validate_round_trip_returns_field_and_form_errors() {
    let request = UiActionRequest {
        request_id: RequestId("req_123".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: Some(UiNodeId("ticket-form".to_string())),
        action_id: UiActionId("project-pipelines.ticket.validate".to_string()),
        kind: UiActionKind::Validate,
        values: Some(UiFormValues(Map::from_iter([
            ("title".to_string(), json!("")),
            ("priority".to_string(), json!("unknown")),
        ]))),
        payload: None,
    };
    let request_value = serde_json::to_value(&request).expect("serialize validate request");
    assert_eq!(
        serde_json::from_value::<UiActionRequest>(request_value)
            .expect("deserialize validate request"),
        request
    );

    let mut field_errors = UiFieldErrors::new();
    field_errors.insert("title".to_string(), vec!["Title is required".to_string()]);
    field_errors.insert(
        "priority".to_string(),
        vec!["Priority is not selectable".to_string()],
    );

    let result = UiActionResult {
        request_id: RequestId("req_123".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: Some(UiNodeId("ticket-form".to_string())),
        action_id: UiActionId("project-pipelines.ticket.validate".to_string()),
        state: UiActionResultState::Rejected,
        field_errors,
        form_errors: vec!["Fix the highlighted fields".to_string()],
        warnings: Vec::new(),
        normalized_values: None,
        tree_update: None,
        payload: None,
        error: None,
    };
    let value = serde_json::to_value(&result).expect("serialize validation result");
    assert_eq!(
        value,
        json!({
            "request_id": "req_123",
            "surface_id": "project-pipelines.ticket.form",
            "action_id": "project-pipelines.ticket.validate",
            "node_id": "ticket-form",
            "state": "rejected",
            "field_errors": {
                "priority": ["Priority is not selectable"],
                "title": ["Title is required"]
            },
            "form_errors": ["Fix the highlighted fields"]
        })
    );
    assert_eq!(
        serde_json::from_value::<UiActionResult>(value).expect("deserialize validation result"),
        result
    );
}

#[test]
fn ui_action_result_returns_normalized_values_and_warnings() {
    let result = UiActionResult {
        request_id: RequestId("req_125".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: Some(UiNodeId("ticket-form".to_string())),
        action_id: UiActionId("project-pipelines.ticket.submit".to_string()),
        state: UiActionResultState::Accepted,
        field_errors: UiFieldErrors::new(),
        form_errors: Vec::new(),
        warnings: vec!["Title was trimmed".to_string()],
        normalized_values: Some(UiFormValues(Map::from_iter([
            ("title".to_string(), json!("Fix checkout flow")),
            ("notify".to_string(), json!(true)),
        ]))),
        tree_update: None,
        payload: Some(json!({ "ticket_id": "ticket_123" })),
        error: None,
    };
    assert_eq!(
        serde_json::to_value(&result).expect("serialize accepted result"),
        json!({
            "request_id": "req_125",
            "surface_id": "project-pipelines.ticket.form",
            "action_id": "project-pipelines.ticket.submit",
            "node_id": "ticket-form",
            "state": "accepted",
            "warnings": ["Title was trimmed"],
            "normalized_values": {
                "notify": true,
                "title": "Fix checkout flow"
            },
            "payload": { "ticket_id": "ticket_123" }
        })
    );
}

#[test]
fn ui_action_rejected_result_preserves_request_correlation() {
    let result = UiActionResult {
        request_id: RequestId("req_124".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        state: UiActionResultState::Rejected,
        field_errors: UiFieldErrors::new(),
        form_errors: Vec::new(),
        warnings: Vec::new(),
        normalized_values: None,
        tree_update: None,
        payload: None,
        error: Some("gate unmet".to_string()),
    };
    let value = serde_json::to_value(&result).expect("serialize rejected result");
    let round_trip =
        serde_json::from_value::<UiActionResult>(value).expect("deserialize rejected result");
    assert_eq!(round_trip.request_id, RequestId("req_124".to_string()));
    assert_eq!(
        round_trip.surface_id,
        UiSurfaceId("project-pipelines.ticket.form".to_string())
    );
    assert_eq!(
        round_trip.action_id,
        UiActionId("project-pipelines.advance".to_string())
    );
    assert_eq!(
        round_trip.node_id,
        Some(UiNodeId("advance-button".to_string()))
    );
    assert_eq!(round_trip.state, UiActionResultState::Rejected);
}

#[test]
fn ui_action_deferred_and_error_states_are_distinct() {
    let deferred = UiActionResult {
        request_id: RequestId("req_126".to_string()),
        surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
        node_id: None,
        action_id: UiActionId("project-pipelines.advance".to_string()),
        state: UiActionResultState::Deferred,
        field_errors: UiFieldErrors::new(),
        form_errors: Vec::new(),
        warnings: Vec::new(),
        normalized_values: None,
        tree_update: None,
        payload: Some(json!({ "operation_id": "op_1" })),
        error: None,
    };
    let errored = UiActionResult {
        request_id: RequestId("req_127".to_string()),
        state: UiActionResultState::Error,
        error: Some("handler unavailable".to_string()),
        ..deferred.clone()
    };

    let deferred_value = serde_json::to_value(&deferred).expect("serialize deferred");
    let error_value = serde_json::to_value(&errored).expect("serialize error");
    assert_eq!(deferred_value["state"], json!("deferred"));
    assert!(deferred_value.get("error").is_none());
    assert_eq!(error_value["state"], json!("error"));
    assert_eq!(error_value["error"], json!("handler unavailable"));
}

#[test]
fn ui_action_result_can_reference_ui_tree_patch_or_replacement() {
    for tree_update in [
        UiTreeUpdateRef::Patch {
            ref_id: "patch_123".to_string(),
        },
        UiTreeUpdateRef::Replacement {
            ref_id: "tree_456".to_string(),
        },
    ] {
        let result = UiActionResult {
            request_id: RequestId("req_128".to_string()),
            surface_id: UiSurfaceId("project-pipelines.ticket.form".to_string()),
            node_id: None,
            action_id: UiActionId("project-pipelines.refresh".to_string()),
            state: UiActionResultState::Accepted,
            field_errors: UiFieldErrors::new(),
            form_errors: Vec::new(),
            warnings: Vec::new(),
            normalized_values: None,
            tree_update: Some(tree_update.clone()),
            payload: None,
            error: None,
        };
        let value = serde_json::to_value(&result).expect("serialize tree update result");
        assert_eq!(
            serde_json::from_value::<UiActionResult>(value).expect("deserialize tree update"),
            result
        );
    }
}

#[test]
fn public_api_import_path_matches_runtime_contract() {
    let via_module = botster_core::ui::UiNode {
        kind: botster_core::ui::UiNodeKind::Text,
        id: None,
        props: Map::from_iter([("text".to_string(), json!("hello"))]),
        children: Vec::new(),
        slots: BTreeMap::new(),
    };
    let via_root = botster_core::UiNode {
        kind: botster_core::UiNodeKind::Text,
        id: None,
        props: Map::from_iter([("text".to_string(), json!("hello"))]),
        children: Vec::new(),
        slots: BTreeMap::new(),
    };

    validate_ui_node(&via_module).expect("module import should validate");
    assert_eq!(via_module, via_root);

    let via_module_request = botster_core::ui::UiActionRequest {
        request_id: RequestId("req_public".to_string()),
        surface_id: botster_core::ui::UiSurfaceId("surface_public".to_string()),
        node_id: None,
        action_id: botster_core::ui::UiActionId("botster.public.test".to_string()),
        kind: botster_core::ui::UiActionKind::Cancel,
        values: None,
        payload: None,
    };
    let via_root_request = botster_core::UiActionRequest {
        request_id: RequestId("req_public".to_string()),
        surface_id: botster_core::UiSurfaceId("surface_public".to_string()),
        node_id: None,
        action_id: botster_core::UiActionId("botster.public.test".to_string()),
        kind: botster_core::UiActionKind::Cancel,
        values: None,
        payload: None,
    };
    assert_eq!(via_module_request, via_root_request);
}

#[test]
fn public_api_import_path_exposes_v1_form_schema_types() {
    let schema = botster_core::UiFieldSchema {
        kind: botster_core::UiFieldKind::Select,
        name: "status".to_string(),
        label: "Status".to_string(),
        description: Some("Workflow state".to_string()),
        placeholder: None,
        required: true,
        default: Some(json!("open")),
        validation: Some(botster_core::UiFieldValidationHints {
            one_of: vec![json!("open")],
            ..Default::default()
        }),
        options: vec![botster_core::UiFieldOption {
            value: json!("open"),
            label: "Open".to_string(),
            disabled: false,
        }],
    };

    let field = botster_core::UiNode {
        kind: botster_core::UiNodeKind::FormField,
        id: Some(botster_core::UiNodeId("status-field".to_string())),
        props: Map::from_iter([(
            "schema".to_string(),
            serde_json::to_value(schema).expect("serialize schema"),
        )]),
        children: Vec::new(),
        slots: BTreeMap::new(),
    };

    botster_core::ui::validate_ui_node(&field).expect("module import should validate form field");
}

#[test]
fn public_api_import_path_exposes_ui_capability_types() {
    let via_module = botster_core::ui::UiCapabilitySet {
        width_classes: BTreeMap::from([(botster_core::ui::UiWidthClass::Regular, ())])
            .into_keys()
            .collect(),
        height_classes: BTreeMap::from([(botster_core::ui::UiHeightClass::Regular, ())])
            .into_keys()
            .collect(),
        pointer: botster_core::ui::UiPointer::Fine,
        keyboard: botster_core::ui::UiKeyboardCapability {
            text_entry: true,
            shortcuts: true,
            focus_traversal: true,
        },
        hover: true,
        clipboard: true,
        context_menu: true,
        dialog_presentations: BTreeMap::from([(
            botster_core::ui::UiDialogPresentation::Overlay,
            (),
        )])
        .into_keys()
        .collect(),
        table: true,
        terminal_selection: true,
        qr_code: true,
        rich_color: true,
        fallbacks: BTreeSet::new(),
    };
    let via_root = botster_core::UiCapabilitySet {
        pointer: botster_core::UiPointer::Fine,
        keyboard: botster_core::UiKeyboardCapability {
            text_entry: true,
            shortcuts: true,
            focus_traversal: true,
        },
        dialog_presentations: BTreeMap::from([(botster_core::UiDialogPresentation::Overlay, ())])
            .into_keys()
            .collect(),
        ..via_module.clone()
    };

    assert_eq!(via_module, via_root);
}
