//! Renderer-neutral UI conformance fixtures for downstream renderers.

use std::collections::{BTreeMap, BTreeSet};

use botster_core::ui::{
    validate_ui_node_with_capabilities, UiActionId, UiActionKind, UiActionRequest, UiActionResult,
    UiActionResultState, UiBindIf, UiBindList, UiCapabilityFallback, UiCapabilitySet, UiChild,
    UiCondition, UiConditional, UiDialogPresentation, UiFieldErrors, UiFieldKind, UiFieldOption,
    UiFieldSchema, UiFieldValidationHints, UiFormValues, UiHeightClass, UiKeyboardCapability,
    UiNode, UiNodeId, UiNodeKind, UiPointer, UiSurfaceId, UiTreeUpdateRef, UiWidthClass,
};
use botster_core::RequestId;
use serde_json::{json, Map, Value};

/// Reusable UI renderer conformance fixture.
#[derive(Debug, Clone, PartialEq)]
pub struct UiRendererConformanceFixture {
    /// Fixture name for downstream test output.
    pub name: &'static str,
    /// Renderer capabilities required by this fixture.
    pub capabilities: UiCapabilitySet,
    /// Root nodes that exercise the contract surface.
    pub nodes: Vec<UiNode>,
    /// Action requests emitted by clients for the fixture.
    pub action_requests: Vec<UiActionRequest>,
    /// Owner-authored action results returned for the fixture.
    pub action_results: Vec<UiActionResult>,
}

/// Return the stable renderer conformance fixture set for the current contract.
pub fn ui_renderer_conformance_fixtures() -> Vec<UiRendererConformanceFixture> {
    vec![
        primitive_fixture(),
        form_fixture(),
        binding_fixture(),
        responsive_fallback_fixture(),
        action_metadata_fixture(),
        application_dashboard_fixture(),
    ]
}

/// Assert that a fixture validates and round-trips through the public UI contract.
pub fn assert_ui_renderer_conformance_fixture(fixture: &UiRendererConformanceFixture) {
    for node in &fixture.nodes {
        validate_ui_node_with_capabilities(node, &fixture.capabilities)
            .expect("fixture node should validate against declared capabilities");

        let value = serde_json::to_value(node).expect("fixture node should serialize");
        let decoded: UiNode =
            serde_json::from_value(value).expect("fixture node should deserialize");
        assert_eq!(decoded, *node);
    }

    for request in &fixture.action_requests {
        let value = serde_json::to_value(request).expect("action request should serialize");
        let decoded: UiActionRequest =
            serde_json::from_value(value).expect("action request should deserialize");
        assert_eq!(decoded, *request);
    }

    for result in &fixture.action_results {
        let value = serde_json::to_value(result).expect("action result should serialize");
        let decoded: UiActionResult =
            serde_json::from_value(value).expect("action result should deserialize");
        assert_eq!(decoded, *result);
    }
}

/// Assert every bundled UI renderer conformance fixture.
pub fn assert_ui_renderer_conformance_fixtures() {
    for fixture in ui_renderer_conformance_fixtures() {
        assert_ui_renderer_conformance_fixture(&fixture);
    }
}

fn primitive_fixture() -> UiRendererConformanceFixture {
    let mut root = node(
        UiNodeKind::Stack,
        "primitive-root",
        json!({ "direction": "vertical" }),
    );
    root.children = vec![
        child(node(
            UiNodeKind::Text,
            "primitive-title",
            json!({ "text": "Renderer primitives", "tone": "accent" }),
        )),
        child(node(
            UiNodeKind::Badge,
            "primitive-badge",
            json!({ "label": "Ready", "tone": "success" }),
        )),
        child(node(
            UiNodeKind::Table,
            "primitive-table",
            json!({ "columns": ["name", "status"] }),
        )),
        child(node(
            UiNodeKind::TerminalView,
            "primitive-terminal",
            json!({ "session_id": "session-fixture", "title": "Shell" }),
        )),
        child(node(
            UiNodeKind::ConnectionCodeView,
            "primitive-code",
            json!({ "code": "pair-fixture", "label": "Pair" }),
        )),
    ];

    fixture("primitives", rich_capabilities(), vec![root])
}

fn form_fixture() -> UiRendererConformanceFixture {
    let schema = UiFieldSchema {
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
                disabled: false,
            },
        ],
    };
    let mut form = node(
        UiNodeKind::Form,
        "form-fixture",
        json!({ "action": { "id": "fixture.form.submit" } }),
    );
    form.children.push(child(node(
        UiNodeKind::TextInput,
        "form-title",
        json!({ "name": "title", "label": "Title", "value": "Owner authored" }),
    )));
    form.children.push(child(node(
        UiNodeKind::FormField,
        "form-status",
        json!({ "schema": schema, "default": "open" }),
    )));

    fixture("forms", rich_capabilities(), vec![form])
}

fn binding_fixture() -> UiRendererConformanceFixture {
    let row = node(
        UiNodeKind::Text,
        "ticket-row-title",
        json!({ "text": { "$bind": "@/title" } }),
    );
    let empty = node(
        UiNodeKind::EmptyState,
        "ticket-empty",
        json!({ "title": "No tickets" }),
    );
    let mut list = node(
        UiNodeKind::List,
        "ticket-list",
        json!({ "aria_label": "Tickets" }),
    );
    list.children.push(UiChild::BindList(UiBindList::BindList {
        source: "/project-pipelines.ticket".to_string(),
        r#where: BTreeMap::from([("status".to_string(), json!("open"))]),
        item_template: Box::new(row),
        empty_template: Some(Box::new(empty)),
    }));
    list.children.push(UiChild::BindIf(UiBindIf::BindIf {
        path: "/project-pipelines.run/active/blocked".to_string(),
        node: Box::new(node(
            UiNodeKind::Badge,
            "blocked-badge",
            json!({ "label": "Blocked", "tone": "warning" }),
        )),
    }));

    fixture("bindings", rich_capabilities(), vec![list])
}

fn responsive_fallback_fixture() -> UiRendererConformanceFixture {
    let mut capabilities = rich_capabilities();
    capabilities.table = false;
    capabilities.terminal_selection = false;
    capabilities.qr_code = false;
    capabilities.rich_color = false;
    capabilities.hover = false;
    capabilities.clipboard = false;
    capabilities.context_menu = false;
    capabilities.dialog_presentations.clear();
    capabilities.fallbacks = BTreeSet::from([
        UiCapabilityFallback::TableAsList,
        UiCapabilityFallback::TerminalSelectionDisabled,
        UiCapabilityFallback::ConnectionCodeText,
        UiCapabilityFallback::RichColorMuted,
        UiCapabilityFallback::DialogInline,
        UiCapabilityFallback::HoverPersistentHints,
        UiCapabilityFallback::ClipboardManual,
        UiCapabilityFallback::ContextMenuAsMenu,
    ]);

    let mut root = node(
        UiNodeKind::Stack,
        "responsive-root",
        json!({
            "direction": {
                "$kind": "responsive",
                "width": { "compact": "vertical", "expanded": "horizontal" }
            }
        }),
    );
    root.children
        .push(UiChild::Conditional(UiConditional::When {
            condition: UiCondition {
                width: Some(UiWidthClass::Compact),
                ..Default::default()
            },
            node: Box::new(node(
                UiNodeKind::Text,
                "compact-copy",
                json!({
                    "text": "Compact",
                    "tone": "muted",
                    "hover_label": "Compact viewport",
                    "copy_value": "compact"
                }),
            )),
        }));
    root.children.push(child(node(
        UiNodeKind::Dialog,
        "responsive-dialog",
        json!({ "title": "Confirm", "presentation": "sheet" }),
    )));
    if let UiChild::Node(dialog) = root.children.last_mut().expect("dialog child") {
        dialog.slots.insert(
            "body".to_string(),
            vec![child(node(
                UiNodeKind::Text,
                "dialog-body",
                json!({ "text": "Body" }),
            ))],
        );
    }
    root.children.push(child(node(
        UiNodeKind::Table,
        "fallback-table",
        json!({ "columns": ["name"] }),
    )));
    root.children.push(child(node(
        UiNodeKind::TerminalView,
        "fallback-terminal",
        json!({ "session_id": "session-fixture" }),
    )));
    root.children.push(child(node(
        UiNodeKind::ConnectionCodeView,
        "fallback-code",
        json!({ "code": "pair-fixture" }),
    )));

    fixture("responsive_fallbacks", capabilities, vec![root])
}

fn action_metadata_fixture() -> UiRendererConformanceFixture {
    let button = node(
        UiNodeKind::Button,
        "advance-button",
        json!({
            "label": "Advance",
            "shortcut": "mod+enter",
            "action": {
                "id": "project-pipelines.advance",
                "payload": { "ticket_id": "ticket-fixture" }
            },
            "context_menu": [{ "id": "project-pipelines.inspect" }]
        }),
    );
    let request = UiActionRequest {
        request_id: RequestId("req-ui-fixture".to_string()),
        surface_id: UiSurfaceId("fixture.surface".to_string()),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
        kind: UiActionKind::Submit,
        values: Some(UiFormValues(Map::from_iter([(
            "title".to_string(),
            json!("Fixture"),
        )]))),
        payload: Some(json!({ "ticket_id": "ticket-fixture" })),
    };
    let result = UiActionResult {
        request_id: RequestId("req-ui-fixture".to_string()),
        surface_id: UiSurfaceId("fixture.surface".to_string()),
        action_id: UiActionId("project-pipelines.advance".to_string()),
        node_id: Some(UiNodeId("advance-button".to_string())),
        state: UiActionResultState::Accepted,
        field_errors: UiFieldErrors::new(),
        form_errors: Vec::new(),
        warnings: vec!["Fixture warning".to_string()],
        normalized_values: Some(UiFormValues(Map::from_iter([(
            "title".to_string(),
            json!("Fixture"),
        )]))),
        tree_update: Some(UiTreeUpdateRef::Patch {
            ref_id: "patch-fixture".to_string(),
        }),
        payload: Some(json!({ "ticket_id": "ticket-fixture" })),
        error: None,
    };

    let mut fixture = fixture("action_metadata", rich_capabilities(), vec![button]);
    fixture.action_requests.push(request);
    fixture.action_results.push(result);
    fixture
}

fn application_dashboard_fixture() -> UiRendererConformanceFixture {
    let mut root = node(
        UiNodeKind::Section,
        "dashboard-section",
        json!({
            "title": "Project dashboard",
            "description": "Operator status and queue",
            "density": "regular",
            "variant": "plain"
        }),
    );

    root.slots.insert(
        "toolbar".to_string(),
        vec![child({
            let mut toolbar = node(
                UiNodeKind::Toolbar,
                "dashboard-toolbar",
                json!({ "label": "Dashboard tools", "density": "compact" }),
            );
            toolbar.slots.insert(
                "commands".to_string(),
                vec![child(node(
                    UiNodeKind::Button,
                    "refresh-dashboard",
                    json!({
                        "label": "Refresh",
                        "action": { "id": "project-pipelines.dashboard.refresh" }
                    }),
                ))],
            );
            toolbar.slots.insert(
                "filters".to_string(),
                vec![child(node(
                    UiNodeKind::Badge,
                    "open-filter",
                    json!({ "label": "Open", "tone": "accent" }),
                ))],
            );
            toolbar.slots.insert(
                "search".to_string(),
                vec![child(node(
                    UiNodeKind::TextInput,
                    "ticket-search",
                    json!({
                        "name": "query",
                        "label": "Search",
                        "placeholder": "Ticket or run"
                    }),
                ))],
            );
            toolbar
        })],
    );

    root.slots.insert(
        "body".to_string(),
        vec![
            child({
                let mut grid = node(
                    UiNodeKind::MetricGrid,
                    "dashboard-metrics",
                    json!({ "density": "compact", "compact": true }),
                );
                grid.children.push(child(node(
                    UiNodeKind::Metric,
                    "metric-active-runs",
                    json!({
                        "label": "Active runs",
                        "value": 3,
                        "caption": "Across projects",
                        "tone": "success",
                        "status": "healthy",
                        "trend": { "direction": "up", "value": "+1", "label": "One more than yesterday" },
                        "action": { "id": "project-pipelines.runs.open" }
                    }),
                )));
                grid.children.push(child(node(
                    UiNodeKind::Metric,
                    "metric-blocked",
                    json!({
                        "label": "Blocked",
                        "value": 1,
                        "caption": "Needs attention",
                        "tone": "warning",
                        "status": "blocked"
                    }),
                )));
                grid
            }),
            child(node(
                UiNodeKind::Table,
                "dashboard-table",
                json!({
                    "columns": [
                        { "id": "title", "label": "Title", "align": "start" },
                        { "id": "status", "label": "Status", "align": "start" }
                    ],
                    "rows": [{
                        "id": "ticket_1",
                        "cells": {
                            "title": "Add renderer fixtures",
                            "status": {
                                "type": "status_badge",
                                "id": "ticket_1_status",
                                "props": {
                                    "label": "Review",
                                    "status": "review",
                                    "tone": "warning"
                                }
                            }
                        },
                        "action": {
                            "id": "project-pipelines.ticket.open",
                            "payload": { "ticket_id": "ticket_1" }
                        }
                    }],
                    "empty_state": {
                        "type": "empty_state",
                        "id": "dashboard-empty",
                        "props": {
                            "title": "No work queued",
                            "description": "New tickets will appear here.",
                            "primary_action": { "id": "project-pipelines.ticket.new" },
                            "secondary_action": { "id": "project-pipelines.docs.open" }
                        }
                    },
                    "selection": { "mode": "single", "selected": ["ticket_1"] },
                    "row_action": { "id": "project-pipelines.ticket.open" }
                }),
            )),
        ],
    );

    root.slots.insert(
        "empty".to_string(),
        vec![child(node(
            UiNodeKind::EmptyState,
            "dashboard-section-empty",
            json!({
                "title": "No dashboard data",
                "primary_action": { "id": "project-pipelines.dashboard.refresh" }
            }),
        ))],
    );

    fixture("application_dashboard", rich_capabilities(), vec![root])
}

fn fixture(
    name: &'static str,
    capabilities: UiCapabilitySet,
    nodes: Vec<UiNode>,
) -> UiRendererConformanceFixture {
    UiRendererConformanceFixture {
        name,
        capabilities,
        nodes,
        action_requests: Vec::new(),
        action_results: Vec::new(),
    }
}

fn node(kind: UiNodeKind, id: &str, props: Value) -> UiNode {
    UiNode {
        kind,
        id: Some(UiNodeId(id.to_string())),
        props: props.as_object().cloned().unwrap_or_default(),
        children: Vec::new(),
        slots: BTreeMap::new(),
    }
}

fn child(node: UiNode) -> UiChild {
    UiChild::Node(Box::new(node))
}

fn rich_capabilities() -> UiCapabilitySet {
    UiCapabilitySet {
        width_classes: BTreeSet::from([
            UiWidthClass::Compact,
            UiWidthClass::Regular,
            UiWidthClass::Expanded,
        ]),
        height_classes: BTreeSet::from([UiHeightClass::Short, UiHeightClass::Regular]),
        pointer: UiPointer::Fine,
        keyboard: UiKeyboardCapability {
            text_entry: true,
            shortcuts: true,
            focus_traversal: true,
        },
        hover: true,
        clipboard: true,
        context_menu: true,
        dialog_presentations: BTreeSet::from([
            UiDialogPresentation::Inline,
            UiDialogPresentation::Overlay,
            UiDialogPresentation::Sheet,
            UiDialogPresentation::Fullscreen,
        ]),
        table: true,
        terminal_selection: true,
        qr_code: true,
        iframe: true,
        rich_color: true,
        fallbacks: BTreeSet::new(),
    }
}
