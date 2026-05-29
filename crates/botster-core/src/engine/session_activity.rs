//! Pure session activity reducer and classifier.

use crate::session::{CoreSession, SessionActivity, SessionActivityEvent, SessionActivityStatus};

/// Apply one portable activity event to core session state.
///
/// Byte events refresh activity only when at least one byte was observed.
/// Lifecycle events update lifecycle state but do not make a stale session
/// active by themselves.
pub fn apply_session_activity_event(session: &mut CoreSession, event: SessionActivityEvent) {
    match event {
        SessionActivityEvent::InputBytes { at, bytes } => {
            session.activity.input_bytes = session.activity.input_bytes.saturating_add(bytes);
            if bytes > 0 {
                session.activity.last_input_at = Some(at);
            }
        }
        SessionActivityEvent::OutputBytes { at, bytes } => {
            session.activity.output_bytes = session.activity.output_bytes.saturating_add(bytes);
            if bytes > 0 {
                session.activity.last_output_at = Some(at);
            }
        }
        SessionActivityEvent::DeclaredActivity { at } => {
            session.activity.last_declared_activity_at = Some(at);
        }
        SessionActivityEvent::Lifecycle { state } => {
            session.lifecycle = state;
        }
    }
}

/// Classify activity from an injected clock and threshold.
///
/// A session is active when its latest input, output, or declared activity is
/// at or within `active_threshold_seconds` of `now_seconds`.
#[must_use]
pub fn classify_session_activity(
    activity: &SessionActivity,
    now_seconds: u64,
    active_threshold_seconds: u64,
) -> SessionActivityStatus {
    match activity.latest_activity_at() {
        Some(last_activity_at)
            if now_seconds.saturating_sub(last_activity_at) <= active_threshold_seconds =>
        {
            SessionActivityStatus::Active
        }
        _ => SessionActivityStatus::Idle,
    }
}
