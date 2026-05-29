//! Contract tests for portable session activity state.

use botster_core::{
    apply_session_activity_event, classify_session_activity, CoreSession, SessionActivityEvent,
    SessionActivityStatus, SessionId, SessionKind, SessionLifecycleState,
};

fn session() -> CoreSession {
    CoreSession::new(
        SessionId("session-activity".to_string()),
        SessionKind::Terminal,
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
fn session_kind_serializes_without_product_only_assumptions() {
    let kinds = vec![
        SessionKind::Terminal,
        SessionKind::Process,
        SessionKind::Agent,
        SessionKind::Plugin {
            plugin_key: "project-pipelines".to_string(),
        },
        SessionKind::Custom("embedder-owned".to_string()),
    ];

    assert_eq!(kinds, round_trip(&kinds));
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
    assert_eq!(session.activity.output_bytes, 24);
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
    assert_eq!(session.activity.input_bytes, 7);
    assert_eq!(session.activity.latest_activity_at(), Some(1_010));
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
fn session_activity_state_round_trips_public_json() {
    let mut session = CoreSession::new(
        SessionId("session-json".to_string()),
        SessionKind::Plugin {
            plugin_key: "project-pipelines".to_string(),
        },
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
