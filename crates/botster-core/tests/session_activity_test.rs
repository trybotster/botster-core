//! Contract tests for portable session activity state.

use std::collections::BTreeMap;

use botster_core::{
    apply_session_activity_event, classify_session_activity, CoreSession, CoreSessionMetadata,
    SessionActivityEvent, SessionActivityStatus, SessionId, SessionLifecycleState,
};

fn session() -> CoreSession {
    CoreSession::new(
        SessionId("session-activity".to_string()),
        SessionLifecycleState::Running,
    )
}

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize contract value");
    serde_json::from_str(&json).expect("deserialize contract value")
}

#[test]
fn session_metadata_serializes_host_owned_session_type_without_core_taxonomy() {
    let mut entries = BTreeMap::new();
    entries.insert("botster.session_type".to_string(), "agent".to_string());
    let agent = CoreSession::with_metadata(
        SessionId("session-agent".to_string()),
        SessionLifecycleState::Running,
        CoreSessionMetadata::from_entries(entries),
    );

    let mut entries = BTreeMap::new();
    entries.insert("botster.session_type".to_string(), "accessory".to_string());
    let accessory = CoreSession::with_metadata(
        SessionId("session-accessory".to_string()),
        SessionLifecycleState::Running,
        CoreSessionMetadata::from_entries(entries),
    );

    assert_eq!(agent, round_trip(&agent));
    assert_eq!(accessory, round_trip(&accessory));
    assert_eq!(
        agent.metadata.entries["botster.session_type"],
        "agent".to_string()
    );
    assert_eq!(
        accessory.metadata.entries["botster.session_type"],
        "accessory".to_string()
    );
}

#[test]
fn session_metadata_accepts_namespaced_host_data_without_pii() {
    let mut entries = BTreeMap::new();
    entries.insert(
        "project_pipelines.visibility".to_string(),
        "plugin".to_string(),
    );
    entries.insert(
        "project_pipelines.surface".to_string(),
        "pipelines".to_string(),
    );
    let session = CoreSession::with_metadata(
        SessionId("session-metadata".to_string()),
        SessionLifecycleState::Running,
        CoreSessionMetadata::from_entries(entries),
    );

    assert!(session.metadata.is_within_encoded_len_limit());
    assert_eq!(session, round_trip(&session));
    assert!(!session.metadata.entries.contains_key("cwd"));
    assert!(!session.metadata.entries.contains_key("title"));
    assert!(!session.metadata.entries.contains_key("username"));
    assert!(!session.metadata.entries.contains_key("prompt"));
    assert!(!session.metadata.entries.contains_key("content"));
}

#[test]
fn session_activity_updates_from_output_bytes() {
    let mut session = session();

    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_000,
            bytes: 24,
        },
    );

    assert_eq!(session.activity.last_output_at, Some(1_000));
    assert_eq!(session.activity.latest_activity_at(), Some(1_000));
}

#[test]
fn session_activity_updates_from_input_bytes() {
    let mut session = session();

    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::InputBytes {
            at: 1_010,
            bytes: 7,
        },
    );

    assert_eq!(session.activity.last_input_at, Some(1_010));
    assert_eq!(session.activity.latest_activity_at(), Some(1_010));
}

#[test]
fn zero_byte_events_do_not_refresh_activity() {
    let mut session = session();

    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::InputBytes {
            at: 1_000,
            bytes: 0,
        },
    );
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_001,
            bytes: 0,
        },
    );

    assert_eq!(session.activity.latest_activity_at(), None);
    assert_eq!(
        classify_session_activity(&session.activity, 1_001, 30),
        SessionActivityStatus::Idle
    );
}

#[test]
fn session_activity_updates_from_process_events_without_byte_activity() {
    let mut session = session();

    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::Lifecycle {
            state: SessionLifecycleState::Exited { code: Some(0) },
        },
    );

    assert_eq!(
        session.lifecycle,
        SessionLifecycleState::Exited { code: Some(0) }
    );
    assert_eq!(session.activity.latest_activity_at(), None);
    assert_eq!(
        classify_session_activity(&session.activity, 1_000, 30),
        SessionActivityStatus::Idle
    );
}

#[test]
fn declared_activity_signal_updates_latest_activity() {
    let mut session = session();

    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::DeclaredActivity { at: 1_020 },
    );

    assert_eq!(session.activity.last_declared_activity_at, Some(1_020));
    assert_eq!(
        classify_session_activity(&session.activity, 1_025, 10),
        SessionActivityStatus::Active
    );
}

#[test]
fn active_idle_classification_uses_injected_clock_and_threshold() {
    let mut session = session();
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_000,
            bytes: 3,
        },
    );

    assert_eq!(
        classify_session_activity(&session.activity, 1_029, 30),
        SessionActivityStatus::Active
    );
    assert_eq!(
        classify_session_activity(&session.activity, 1_030, 30),
        SessionActivityStatus::Active
    );
    assert_eq!(
        classify_session_activity(&session.activity, 1_031, 30),
        SessionActivityStatus::Idle
    );
}

#[test]
fn running_session_without_recent_activity_is_idle() {
    let mut session = session();
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_000,
            bytes: 10,
        },
    );

    assert_eq!(session.lifecycle, SessionLifecycleState::Running);
    assert_eq!(
        classify_session_activity(&session.activity, 1_500, 30),
        SessionActivityStatus::Idle
    );
}

#[test]
fn exited_session_with_recent_output_is_active_by_activity_only() {
    let mut session = session();
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_000,
            bytes: 10,
        },
    );
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::Lifecycle {
            state: SessionLifecycleState::Exited { code: Some(0) },
        },
    );

    assert_eq!(
        classify_session_activity(&session.activity, 1_010, 30),
        SessionActivityStatus::Active
    );
}

#[test]
fn session_activity_state_round_trips_public_json() {
    let mut session = CoreSession::new(
        SessionId("session-json".to_string()),
        SessionLifecycleState::Running,
    );
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::InputBytes {
            at: 1_000,
            bytes: 5,
        },
    );
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: 1_001,
            bytes: 9,
        },
    );

    assert_eq!(session, round_trip(&session));
}
