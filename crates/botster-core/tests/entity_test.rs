//! Entity contract tests.

use botster_core::{
    EntityApplyStatus, EntityContract, EntityError, EntityFrame, EntityId, EntityKind, EntityStore,
    EntityStores,
};
use serde_json::{json, Value};

fn kind(value: &str) -> EntityKind {
    EntityKind::from(value)
}

fn id(value: &str) -> EntityId {
    EntityId::from(value)
}

#[test]
fn entity_frame_snapshot_round_trips_wire_shape() {
    let frame = EntityFrame::Snapshot {
        entity_type: kind("session"),
        snapshot_seq: 42,
        items: vec![json!({ "session_uuid": "sess-1" })],
    };

    let wire = serde_json::to_value(&frame).expect("serialize frame");
    assert_eq!(
        wire,
        json!({
            "type": "entity_snapshot",
            "entity_type": "session",
            "snapshot_seq": 42,
            "items": [{ "session_uuid": "sess-1" }]
        })
    );
    assert_eq!(
        serde_json::from_value::<EntityFrame>(wire).expect("deserialize frame"),
        frame
    );
}

#[test]
fn entity_frame_scoped_snapshot_round_trips_wire_shape() {
    let frame = EntityFrame::ScopedSnapshot {
        entity_type: kind("project-pipelines.run_step"),
        snapshot_seq: 7,
        scope: serde_json::from_value(json!({ "run_id": "run-1" })).expect("scope object"),
        items: vec![json!({ "id": "step-1", "run_id": "run-1" })],
    };

    let wire = serde_json::to_value(&frame).expect("serialize frame");
    assert_eq!(
        wire,
        json!({
            "type": "entity_scoped_snapshot",
            "entity_type": "project-pipelines.run_step",
            "snapshot_seq": 7,
            "scope": { "run_id": "run-1" },
            "items": [{ "id": "step-1", "run_id": "run-1" }]
        })
    );
    assert_eq!(
        serde_json::from_value::<EntityFrame>(wire).expect("deserialize frame"),
        frame
    );
}

#[test]
fn entity_delta_frames_round_trip_wire_shape() {
    let frames = [
        (
            EntityFrame::Upsert {
                entity_type: kind("workspace"),
                snapshot_seq: 2,
                id: id("ws-1"),
                entity: json!({ "workspace_id": "ws-1", "name": "Main" }),
            },
            json!({
                "type": "entity_upsert",
                "entity_type": "workspace",
                "snapshot_seq": 2,
                "id": "ws-1",
                "entity": { "workspace_id": "ws-1", "name": "Main" }
            }),
        ),
        (
            EntityFrame::Patch {
                entity_type: kind("workspace"),
                snapshot_seq: 3,
                id: id("ws-1"),
                patch: json!({ "name": "Renamed" }),
            },
            json!({
                "type": "entity_patch",
                "entity_type": "workspace",
                "snapshot_seq": 3,
                "id": "ws-1",
                "patch": { "name": "Renamed" }
            }),
        ),
        (
            EntityFrame::Remove {
                entity_type: kind("workspace"),
                snapshot_seq: 4,
                id: id("ws-1"),
            },
            json!({
                "type": "entity_remove",
                "entity_type": "workspace",
                "snapshot_seq": 4,
                "id": "ws-1"
            }),
        ),
    ];

    for (frame, wire) in frames {
        assert_eq!(serde_json::to_value(&frame).expect("serialize frame"), wire);
        assert_eq!(
            serde_json::from_value::<EntityFrame>(wire).expect("deserialize frame"),
            frame
        );
    }
}

#[test]
fn stale_delta_after_newer_snapshot_is_dropped() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("session"),
            snapshot_seq: 10,
            items: vec![json!({ "session_uuid": "sess-1", "status": "running" })],
        })
        .expect("apply snapshot");

    let status = store
        .apply_frame(&EntityFrame::Patch {
            entity_type: kind("session"),
            snapshot_seq: 9,
            id: id("sess-1"),
            patch: json!({ "status": "stale" }),
        })
        .expect("apply patch");

    assert_eq!(status, EntityApplyStatus::DroppedStale);
    assert_eq!(
        store.get(&id("sess-1")).expect("session record")["status"],
        "running"
    );
    assert_eq!(store.snapshot_seq(), 10);
}

#[test]
fn same_sequence_snapshot_replaces_store_as_resync() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("workspace"),
            snapshot_seq: 3,
            items: vec![json!({ "workspace_id": "ws-old", "name": "Old" })],
        })
        .expect("apply snapshot");

    let status = store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("workspace"),
            snapshot_seq: 3,
            items: vec![json!({ "workspace_id": "ws-new", "name": "New" })],
        })
        .expect("apply resync");

    assert_eq!(status, EntityApplyStatus::Applied);
    assert!(store.get(&id("ws-old")).is_none());
    assert_eq!(
        store.get(&id("ws-new")).expect("new workspace")["name"],
        "New"
    );
    assert_eq!(store.snapshot_seq(), 3);
}

#[test]
fn stale_full_snapshot_is_dropped_without_replacing_store() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("workspace"),
            snapshot_seq: 5,
            items: vec![json!({ "workspace_id": "ws-current", "name": "Current" })],
        })
        .expect("apply current snapshot");

    let status = store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("workspace"),
            snapshot_seq: 4,
            items: vec![json!({ "workspace_id": "ws-stale", "name": "Stale" })],
        })
        .expect("apply stale snapshot");

    assert_eq!(status, EntityApplyStatus::DroppedStale);
    assert!(store.get(&id("ws-stale")).is_none());
    assert_eq!(
        store.get(&id("ws-current")).expect("current workspace")["name"],
        "Current"
    );
    assert_eq!(store.snapshot_seq(), 5);
}

#[test]
fn scoped_snapshot_replaces_only_matching_scope() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("project-pipelines.run_step"),
            snapshot_seq: 5,
            items: vec![
                json!({ "id": "step-1", "run_id": "run-1", "name": "old" }),
                json!({ "id": "step-2", "run_id": "run-2", "name": "kept" }),
            ],
        })
        .expect("apply snapshot");

    store
        .apply_frame(&EntityFrame::ScopedSnapshot {
            entity_type: kind("project-pipelines.run_step"),
            snapshot_seq: 5,
            scope: serde_json::from_value(json!({ "run_id": "run-1" })).expect("scope object"),
            items: vec![json!({ "id": "step-3", "run_id": "run-1", "name": "new" })],
        })
        .expect("apply scoped snapshot");

    assert!(store.get(&id("step-1")).is_none());
    assert_eq!(
        store.get(&id("step-2")).expect("unrelated step")["name"],
        "kept"
    );
    assert_eq!(
        store.get(&id("step-3")).expect("replacement step")["name"],
        "new"
    );
    assert_eq!(store.snapshot_seq(), 5);
}

#[test]
fn scoped_snapshot_rejects_empty_scope() {
    let mut store = EntityStore::new();

    assert_eq!(
        store.apply_frame(&EntityFrame::ScopedSnapshot {
            entity_type: kind("project-pipelines.run_step"),
            snapshot_seq: 1,
            scope: serde_json::Map::new(),
            items: vec![],
        }),
        Err(EntityError::InvalidScope {
            entity_type: "project-pipelines.run_step".to_string(),
        })
    );
    assert_eq!(store.snapshot_seq(), 0);
    assert!(store.is_empty());
}

#[test]
fn scoped_snapshot_allows_same_sequence_delta() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 4,
            items: vec![json!({ "id": "ticket-1", "project_id": "p1", "status": "open" })],
        })
        .expect("apply snapshot");
    store
        .apply_frame(&EntityFrame::ScopedSnapshot {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 5,
            scope: serde_json::from_value(json!({ "project_id": "p1" })).expect("scope object"),
            items: vec![json!({ "id": "ticket-1", "project_id": "p1", "status": "open" })],
        })
        .expect("apply scoped snapshot");

    let status = store
        .apply_frame(&EntityFrame::Patch {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 5,
            id: id("ticket-1"),
            patch: json!({ "status": "closed" }),
        })
        .expect("apply same-seq delta");

    assert_eq!(status, EntityApplyStatus::Applied);
    assert_eq!(
        store.get(&id("ticket-1")).expect("ticket record")["status"],
        "closed"
    );
    assert_eq!(store.snapshot_seq(), 5);
}

#[test]
fn remove_noop_advances_sequence_without_reordering_records() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 1,
            items: vec![
                json!({ "id": "ticket-1", "status": "open" }),
                json!({ "id": "ticket-2", "status": "open" }),
            ],
        })
        .expect("apply snapshot");

    let status = store
        .apply_frame(&EntityFrame::Remove {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 2,
            id: id("missing-ticket"),
        })
        .expect("apply missing remove");

    assert_eq!(status, EntityApplyStatus::Noop);
    assert_eq!(store.snapshot_seq(), 2);
    assert_eq!(
        store
            .iter()
            .map(|(entity_id, _)| entity_id.as_str().to_string())
            .collect::<Vec<_>>(),
        vec!["ticket-1".to_string(), "ticket-2".to_string()]
    );
}

#[test]
fn patch_replaces_nested_values_without_deep_merge() {
    let mut store = EntityStore::new();
    store
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 1,
            items: vec![json!({
                "id": "ticket-1",
                "metadata": { "labels": ["bug"], "assignee": "jason" }
            })],
        })
        .expect("apply snapshot");

    store
        .apply_frame(&EntityFrame::Patch {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 2,
            id: id("ticket-1"),
            patch: json!({ "metadata": { "labels": ["done"] } }),
        })
        .expect("apply patch");

    assert_eq!(
        store.get(&id("ticket-1")).expect("ticket record")["metadata"],
        json!({ "labels": ["done"] })
    );
}

#[test]
fn upsert_rejects_record_id_mismatch() {
    let mut store = EntityStore::new();

    assert_eq!(
        store.apply_frame(&EntityFrame::Upsert {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 1,
            id: id("ticket-envelope"),
            entity: json!({ "id": "ticket-record", "status": "open" }),
        }),
        Err(EntityError::InvalidRecordId {
            entity_type: "project-pipelines.ticket".to_string(),
            id_field: "id".to_string(),
        })
    );
    assert!(store.is_empty());
}

#[test]
fn plugin_entity_family_round_trips_through_generic_stores() {
    let mut stores = EntityStores::new();
    stores
        .apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("kanban.board"),
            snapshot_seq: 1,
            items: vec![json!({ "id": "board-1", "title": "Backlog" })],
        })
        .expect("apply plugin snapshot");

    stores
        .apply_frame(&EntityFrame::Upsert {
            entity_type: kind("kanban.card"),
            snapshot_seq: 1,
            id: id("card-1"),
            entity: json!({ "id": "card-1", "board_id": "board-1" }),
        })
        .expect("apply plugin upsert");

    assert_eq!(
        stores
            .get(&kind("kanban.board"))
            .expect("board store")
            .get(&id("board-1"))
            .expect("board record")["title"],
        "Backlog"
    );
    assert_eq!(
        stores
            .get(&kind("kanban.card"))
            .expect("card store")
            .get(&id("card-1"))
            .expect("card record")["board_id"],
        "board-1"
    );
}

#[test]
fn plugin_entity_type_requires_owner_namespace() {
    assert!(EntityContract::validate_entity_type(&kind("kanban.board"), Some("kanban")).is_ok());
    assert_eq!(
        EntityContract::validate_entity_type(&kind("kanban.board"), Some("notes")),
        Err(EntityError::PluginNamespaceMismatch {
            entity_type: "kanban.board".to_string(),
            owner_plugin: "notes".to_string(),
        })
    );
    assert_eq!(
        EntityContract::validate_entity_type(&kind("board"), Some("kanban")),
        Err(EntityError::InvalidEntityType("board".to_string()))
    );
}

#[test]
fn reserved_builtin_families_do_not_require_plugin_namespace() {
    for entity_type in [
        "session",
        "workspace",
        "spawn_target",
        "worktree",
        "hub",
        "connection_code",
        "template",
        "session_action",
    ] {
        assert!(
            EntityContract::validate_entity_type(&kind(entity_type), None).is_ok(),
            "{entity_type} should be reserved"
        );
    }
}

#[test]
fn plugin_records_require_non_empty_string_ids() {
    assert_eq!(
        EntityContract::extract_record_id(&kind("kanban.board"), &json!({ "id": 12 })),
        Err(EntityError::InvalidRecordId {
            entity_type: "kanban.board".to_string(),
            id_field: "id".to_string(),
        })
    );
    assert_eq!(
        EntityContract::extract_record_id(&kind("kanban.board"), &json!({ "id": "" })),
        Err(EntityError::InvalidRecordId {
            entity_type: "kanban.board".to_string(),
            id_field: "id".to_string(),
        })
    );
}

#[test]
fn plugin_entity_id_field_helper_rejects_non_default_id_fields() {
    assert_eq!(
        EntityContract::validate_id_field(&kind("kanban.board"), "board_id"),
        Err(EntityError::InvalidPluginIdField {
            entity_type: "kanban.board".to_string(),
        })
    );
    assert_eq!(
        EntityContract::extract_record_id_with_field(
            &kind("kanban.board"),
            &json!({ "board_id": "board-1" }),
            "board_id"
        ),
        Err(EntityError::InvalidPluginIdField {
            entity_type: "kanban.board".to_string(),
        })
    );
}

#[test]
fn builtin_id_field_defaults_extract_expected_ids() {
    let cases = [
        ("session", json!({ "session_uuid": "sess-1" }), "sess-1"),
        ("workspace", json!({ "workspace_id": "ws-1" }), "ws-1"),
        (
            "spawn_target",
            json!({ "target_id": "target-1" }),
            "target-1",
        ),
        (
            "worktree",
            json!({ "worktree_path": "/tmp/worktree" }),
            "/tmp/worktree",
        ),
        ("hub", json!({ "hub_id": "hub-1" }), "hub-1"),
        ("connection_code", json!({ "hub_id": "hub-2" }), "hub-2"),
        ("template", json!({ "id": "template-1" }), "template-1"),
        (
            "session_action",
            json!({ "id": "sess-1:close" }),
            "sess-1:close",
        ),
    ];

    for (entity_type, record, expected_id) in cases {
        assert_eq!(
            EntityContract::extract_record_id(&kind(entity_type), &record)
                .expect("extract id")
                .as_str(),
            expected_id
        );
    }
}

#[test]
fn invalid_snapshot_and_upsert_records_are_rejected() {
    let mut store = EntityStore::new();

    assert_eq!(
        store.apply_frame(&EntityFrame::Snapshot {
            entity_type: kind("session"),
            snapshot_seq: 1,
            items: vec![json!({ "id": "not-session-id" })],
        }),
        Err(EntityError::InvalidRecordId {
            entity_type: "session".to_string(),
            id_field: "session_uuid".to_string(),
        })
    );
    assert_eq!(store.len(), 0);

    assert_eq!(
        store.apply_frame(&EntityFrame::Upsert {
            entity_type: kind("project-pipelines.ticket"),
            snapshot_seq: 1,
            id: id("ticket-1"),
            entity: json!({ "title": "Missing id" }),
        }),
        Err(EntityError::InvalidRecordId {
            entity_type: "project-pipelines.ticket".to_string(),
            id_field: "id".to_string(),
        })
    );
}

#[test]
fn public_api_applies_protocol_frames_like_downstream_client() {
    let frames: Vec<EntityFrame> = serde_json::from_value(json!([
        {
            "type": "entity_snapshot",
            "entity_type": "project-pipelines.ticket",
            "snapshot_seq": 1,
            "items": [{ "id": "ticket-1", "status": "open" }]
        },
        {
            "type": "entity_patch",
            "entity_type": "project-pipelines.ticket",
            "snapshot_seq": 2,
            "id": "ticket-1",
            "patch": { "status": "closed", "metadata": { "pr": 123 } }
        }
    ]))
    .expect("deserialize protocol frames");

    let mut stores = EntityStores::new();
    for frame in frames {
        stores.apply_frame(&frame).expect("apply protocol frame");
    }

    let record: &Value = stores
        .get(&kind("project-pipelines.ticket"))
        .expect("ticket store")
        .get(&id("ticket-1"))
        .expect("ticket record");
    assert_eq!(record["status"], "closed");
    assert_eq!(record["metadata"], json!({ "pr": 123 }));
}
