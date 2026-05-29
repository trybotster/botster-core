//! UI contract serialization and validation tests.

use std::collections::BTreeMap;

use botster_core::ui::{
    validate_ui_node, UiActionId, UiActionPending, UiActionResult, UiActionStatus, UiBind,
    UiBindIf, UiBindList, UiChild, UiCondition, UiConditional, UiHeightClass, UiNode, UiNodeId,
    UiNodeKind, UiPointer, UiResponsiveHeight, UiResponsiveValue, UiResponsiveWidth,
    UiValidationError, UiWidthClass,
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
