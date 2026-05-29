//! Reusable regression-shape fixture builders for core contract tests.
//!
//! These fixtures translate old runtime regression evidence into public
//! `botster_core` contract data. They intentionally do not encode runtime
//! policy, parser behavior, Lua VM execution, or browser store mechanics.

use botster_core::actor::{
    BackpressureRoute, BackpressureSummary, ClientConnectionHealth, ClientControlFrame,
    HubControlMessage, HubControlOrigin, PluginHandlerKind, PluginHandlerRef,
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationSuccess, PluginKey,
    PluginWorkerEvent, QueueSource, TerminalAttachState,
};
use botster_core::boundary::BoundaryJson;
use botster_core::client::ClientId;
use botster_core::entity::{EntityFrame, EntityId, EntityKind};
use botster_core::session::{CoreSession, RequestId, SessionActivityEvent, SessionId, SessionKind};
use botster_core::session_activity::apply_session_activity_event;
use botster_core::transport::TransportEgress;
use botster_core::{classify_session_activity, SubscriptionId};
use botster_core::{SessionActivityStatus, SessionLifecycleState};

/// Translate noisy PTY replay into ordered opaque output byte chunks.
///
/// Verdict: translate. Core preserves byte order and opacity, not Ghostty or
/// parser fidelity assertions.
#[must_use]
pub fn noisy_pty_replay(chunks: &[&[u8]]) -> Vec<Vec<u8>> {
    chunks.iter().map(|chunk| (*chunk).to_vec()).collect()
}

/// Translate last-output activity evidence into core output-byte accounting.
///
/// Verdict: translate. Activity is derived from observed output bytes and an
/// injected clock/threshold, independent of whether any client is attached.
#[must_use]
pub fn last_output_activity(
    session_id: SessionId,
    output_at: u64,
    output_bytes: u64,
    now_seconds: u64,
    active_threshold_seconds: u64,
) -> (CoreSession, SessionActivityStatus) {
    let mut session = CoreSession::new(
        session_id,
        SessionKind::Terminal,
        SessionLifecycleState::Running,
    );
    apply_session_activity_event(
        &mut session,
        SessionActivityEvent::OutputBytes {
            at: output_at,
            bytes: output_bytes,
        },
    );
    let status =
        classify_session_activity(&session.activity, now_seconds, active_threshold_seconds);

    (session, status)
}

/// Translate stale reconnect generation evidence into current identity fields.
///
/// Verdict: translate. Stale/current is represented by old and current
/// `SubscriptionId` values plus reconnect health. No generation or epoch field
/// is introduced by this fixture.
#[must_use]
pub fn stale_reconnect_cycle(
    client_id: ClientId,
    session_id: SessionId,
    stale_subscription_id: SubscriptionId,
    current_subscription_id: SubscriptionId,
) -> Vec<HubControlMessage> {
    vec![
        HubControlMessage::AttachClient {
            origin: HubControlOrigin::Client(client_id.clone()),
            request_id: RequestId("req-stale-reconnect-old".to_string()),
            client_id: client_id.clone(),
            session_id: session_id.clone(),
            subscription_id: stale_subscription_id,
        },
        HubControlMessage::AttachClient {
            origin: HubControlOrigin::Client(client_id.clone()),
            request_id: RequestId("req-stale-reconnect-current".to_string()),
            client_id,
            session_id,
            subscription_id: current_subscription_id,
        },
    ]
}

/// Reconnect health marker paired with `stale_reconnect_cycle`.
///
/// Verdict: translate. This is the existing client stream health vocabulary,
/// not a stale-drop runtime policy.
#[must_use]
pub fn reconnecting_health() -> ClientControlFrame {
    ClientControlFrame::Health {
        health: ClientConnectionHealth::Reconnecting,
    }
}

/// Preserve bounded queue saturation as capacity plus typed pressure context.
///
/// Verdict: preserve. Core owns bounded queue metadata and pressure summaries.
#[must_use]
pub fn bounded_queue_saturation(
    source: QueueSource,
    depth: usize,
    route: BackpressureRoute,
) -> BackpressureSummary {
    let config = source.default_config();
    BackpressureSummary {
        source,
        capacity: config.capacity,
        depth,
        route,
    }
}

/// Translate unknown-peer bursts into transport-adapter pressure.
///
/// Verdict: translate. Core records boundary pressure context and leaves
/// unknown-peer coalescing or rate-limit algorithms to adapters.
#[must_use]
pub fn unknown_peer_burst_pressure(peer_ids: &[&str]) -> BackpressureSummary {
    BackpressureSummary {
        source: QueueSource::TransportAdapter,
        capacity: QueueSource::TransportAdapter.default_capacity(),
        depth: peer_ids.len(),
        route: BackpressureRoute::queue_only(),
    }
}

/// Preserve snapshot-before-live-output ordering as transport-neutral egress.
///
/// Verdict: preserve/translate. Core preserves ordered snapshot/live contract
/// data and uses attach state vocabulary without adding pushed terminal-mode
/// event variants.
#[must_use]
pub fn snapshot_before_live_output(
    session_id: SessionId,
    snapshot: &[u8],
    live_output: &[u8],
) -> Vec<TransportEgress> {
    let subscription_id = SubscriptionId("sub-regression-snapshot".to_string());
    vec![
        TransportEgress::AttachState {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            state: TerminalAttachState::Attaching,
        },
        TransportEgress::Snapshot {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            data: snapshot.to_vec(),
        },
        TransportEgress::AttachState {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            state: TerminalAttachState::Attached,
        },
        TransportEgress::TerminalOutput {
            session_id,
            subscription_id,
            data: live_output.to_vec(),
        },
    ]
}

/// Translate plugin-scoped hydration into existing entity frame variants.
///
/// Verdict: translate. Scope is encoded in plugin-owned kind/id/record data;
/// this fixture does not add client-store policy or new frame variants.
#[must_use]
pub fn entity_scoped_hydration(plugin_key: &str, scope_id: &str) -> Vec<EntityFrame> {
    let kind = EntityKind(format!("{plugin_key}.ticket"));
    let record_id = EntityId(format!("{scope_id}:ticket-1"));
    vec![
        EntityFrame::Snapshot {
            entity_type: kind.clone(),
            snapshot_seq: 1,
            items: vec![serde_json::json!({
                "id": record_id.0,
                "scope_id": scope_id,
                "title": "Build contract fixtures"
            })],
        },
        EntityFrame::Upsert {
            entity_type: kind.clone(),
            snapshot_seq: 2,
            id: record_id.clone(),
            entity: serde_json::json!({
                "id": record_id.0,
                "scope_id": scope_id,
                "status": "running"
            }),
        },
        EntityFrame::Patch {
            entity_type: kind.clone(),
            snapshot_seq: 3,
            id: record_id.clone(),
            patch: serde_json::json!({ "status": "review" }),
        },
        EntityFrame::Remove {
            entity_type: kind,
            snapshot_seq: 4,
            id: record_id,
        },
    ]
}

/// Preserve plugin-worker timeout/backpressure as handler refs and queue events.
///
/// Verdict: preserve/translate. Core preserves typed handler identity,
/// backpressure, and timeout-shaped failure payloads while dropping Lua VM
/// execution and blocking timeout mechanics.
#[must_use]
pub fn plugin_worker_timeout_backpressure(
    plugin_key: PluginKey,
    handler_id: &str,
) -> Vec<PluginWorkerEvent> {
    let handler = PluginHandlerRef {
        plugin_key: plugin_key.clone(),
        kind: PluginHandlerKind::Command,
        handler_id: handler_id.to_string(),
    };
    vec![
        PluginWorkerEvent::Backpressure(BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: QueueSource::PluginWorker.default_capacity(),
            depth: QueueSource::PluginWorker.default_capacity(),
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(plugin_key.clone()),
            },
        }),
        PluginWorkerEvent::Failed {
            request_id: RequestId("req-plugin-timeout".to_string()),
            plugin_key: plugin_key.clone(),
            reason: "handler timed out".to_string(),
        },
        PluginWorkerEvent::InvocationTimedOut(PluginInvocationFailure {
            request_id: RequestId("req-plugin-timeout-observed".to_string()),
            handler: handler.clone(),
            kind: PluginInvocationFailureKind::TimedOut,
            timeout_ms: Some(1_000),
            reason: "handler timed out".to_string(),
        }),
        PluginWorkerEvent::InvocationCompleted(PluginInvocationSuccess {
            request_id: RequestId("req-plugin-timeout-observed".to_string()),
            handler,
            payload: Some(BoundaryJson(serde_json::json!({
                "timeout": true,
                "dropped_policy": "lua_vm_blocking_timeout_mechanics"
            }))),
        }),
    ]
}
