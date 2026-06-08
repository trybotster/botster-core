//! UI contract serialization and validation tests.

use std::collections::BTreeMap;

use botster_core::ui::{
    validate_ui_node, UiActionId, UiActionKind, UiActionRequest, UiActionResult,
    UiActionResultState, UiBind, UiBindIf, UiBindList, UiChild, UiCondition, UiConditional,
    UiFieldErrors, UiFormValues, UiHeightClass, UiNode, UiNodeId, UiNodeKind, UiPointer,
    UiResponsiveHeight, UiResponsiveValue, UiResponsiveWidth, UiSurfaceId, UiTreeUpdateRef,
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
