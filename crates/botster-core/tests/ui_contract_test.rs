//! UI contract serialization and validation tests.

use std::collections::BTreeMap;

use botster_core::ui::{
    validate_ui_node, UiActionId, UiActionPending, UiActionResult, UiActionStatus, UiBind,
    UiBindIf, UiBindList, UiChild, UiCondition, UiConditional, UiFieldKind, UiFieldOption,
    UiFieldSchema, UiFieldValidationHints, UiHeightClass, UiNode, UiNodeId, UiNodeKind, UiPointer,
    UiResponsiveHeight, UiResponsiveValue, UiResponsiveWidth, UiValidationError, UiWidthClass,
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
fn action_pending_and_result_identity_is_representable() {
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

    let pending = UiActionPending {
        request_id: RequestId("req_123".to_string()),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
    };
    let value = serde_json::to_value(&pending).expect("serialize pending");
    assert_eq!(
        value,
        json!({
            "request_id": "req_123",
            "action_id": "project-pipelines.advance",
            "node_id": "advance-button"
        })
    );
    assert_eq!(
        serde_json::from_value::<UiActionPending>(value).expect("deserialize pending"),
        pending
    );

    let success = UiActionResult {
        request_id: RequestId("req_123".to_string()),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
        status: UiActionStatus::Success,
        payload: Some(json!({ "advanced": true })),
        error: None,
    };
    assert_eq!(
        serde_json::to_value(&success).expect("serialize success"),
        json!({
            "request_id": "req_123",
            "action_id": "project-pipelines.advance",
            "node_id": "advance-button",
            "status": "success",
            "payload": { "advanced": true }
        })
    );

    let failure = UiActionResult {
        request_id: RequestId("req_124".to_string()),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        node_id: None,
        status: UiActionStatus::Failure,
        payload: None,
        error: Some("gate unmet".to_string()),
    };
    let value = serde_json::to_value(&failure).expect("serialize failure");
    assert_eq!(
        value,
        json!({
            "request_id": "req_124",
            "action_id": "project-pipelines.advance",
            "status": "failure",
            "error": "gate unmet"
        })
    );
    assert_eq!(
        serde_json::from_value::<UiActionResult>(value).expect("deserialize failure"),
        failure
    );
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
