#![allow(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::fs;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use botster_core::TerminalScreenSize;
use botster_core::{
    BotsterEngineObservation, ClientId, CoreSessionMetadata, EndpointId, EnvelopeCursor,
    EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget, ModeFlags, NotificationContent,
    NotificationDeliveryStatus, NotificationId, NotificationItem, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp, RequestId, ResizePayload, Rgb,
    RoutedEnvelope, RoutedEnvelopeObservation, RoutedEnvelopePayload, RoutedEnvelopeQueueConfig,
    SessionId, SessionLifecycleState, SessionSpawnRequest, SessionWorkerHealthReason,
    SessionWorkerStaleReason, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TerminalAttachState, TerminalColorProfile, TransportEgress, MAX_CORE_SESSION_METADATA_LEN,
};
use botster_core_daemon::{
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest,
    CaptureColorAndSnapshotRequest, CaptureSnapshotRequest, CoreDaemon, CoreDaemonConfig,
    CoreDaemonError, DaemonSession, DrainNotificationsRequest, DrainRoutedEnvelopesRequest,
    GuardedWriteDecision, GuardedWriteDeliveryState, GuardedWriteRequest, ModeGatedInputOutcome,
    PostNotificationRequest, PublishRoutedEnvelopeRequest, ReadModeFlagsRequest, ReadScreenRequest,
    ReadinessEvidence, RegistrySessionState, SafeWriteIndicator, SessionAdoptionState,
    SessionLifecycleBaseline, SessionLifecycleChangeKind, SessionLifecycleChanges,
    SessionLifecycleRecord, SessionLifecycleResyncReason, SpawnSessionRequest,
};
use botster_core_daemon::{
    DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY,
};
use botster_core_test_support::conformance::{
    assert_color_profile_authority, assert_ghostty_snapshot_authority, assert_mode_flags_authority,
    GHOSTSNP_MAGIC,
};
use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;
use botster_terminal_ghostty::{
    GhosttyAdapterConfig, GhosttyClientProjection, GhosttySnapshotDecodeProgress, GhosttyTerminal,
    COLOR_INDEX_BACKGROUND, COLOR_INDEX_CURSOR, COLOR_INDEX_FOREGROUND,
};

const EXPECTED_SNAPSHOT_FORMAT: &str = "ghostty-terminal-snapshot-v1";
const EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING: usize = 16 * 1024 * 1024;
const EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS: usize = 4_000;
const EXPECTED_GHOSTTY_DROPPED_MARKER: &str = "echo:scrollback-line-00000";
const LOW_GHOSTTY_MAX_SCROLLBACK_BYTES: usize = 1_000_000;
const REAL_WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const REAL_WORKER_COMPLETION_TIMEOUT: Duration = Duration::from_secs(180);

#[test]
fn daemon_config_defaults_to_production_ghostty_scrollback_byte_budget() {
    let config = CoreDaemonConfig::new("daemon-config-default");

    assert_eq!(
        config.ghostty_max_scrollback_bytes,
        DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES
    );
    assert_eq!(
        config.lifecycle_journal_capacity,
        DEFAULT_LIFECYCLE_JOURNAL_CAPACITY
    );
}

#[test]
fn lifecycle_changes_reject_a_cursor_ahead_of_the_source() {
    let data_dir = temp_data_dir("lifecycle-cursor-ahead");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let mut ahead = daemon
        .lifecycle_baseline()
        .expect("empty lifecycle baseline")
        .cursor;
    ahead.sequence += 1;

    let changes = daemon.lifecycle_changes(&ahead);

    assert!(changes.changes.is_empty());
    assert_eq!(
        changes.resync_required,
        Some(SessionLifecycleResyncReason::CursorAhead)
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_spawns_lists_attaches_drains_inputs_resizes_and_shuts_down() {
    let data_dir = temp_data_dir("daemon-api");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-api-session".to_string());
    let client_id = ClientId("daemon-api-client".to_string());
    let subscription_id = SubscriptionId("daemon-api-subscription".to_string());

    let session = daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn through core engine");
    assert_eq!(session.session_id, session_id);

    let listed = daemon.list().expect("registry list should load");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, session_id);
    assert_eq!(listed[0].registry_state, RegistrySessionState::Running);
    assert!(
        data_dir
            .join("sessions")
            .join("daemon-api-session.json")
            .exists(),
        "spawn should persist a non-PII registry record"
    );

    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("attach should use core subscription path");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"ping\n".to_vec(),
            12,
        )
        .expect("input should use core write path");
    daemon
        .resize(client_id, session_id.clone(), 30, 100, 13)
        .expect("resize should use core resize path");

    let drained = drain_until(&mut daemon, &session_id, "echo:ping");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:ping"),
        "input should echo through daemon-drained client egress: {output:?}"
    );

    let listed = daemon
        .list()
        .expect("registry list should load after resize");
    assert_eq!(listed[0].size.rows, 30);
    assert_eq!(listed[0].size.cols, 100);

    daemon
        .shutdown(Some(session_id.clone()), 30)
        .expect("shutdown should route through core shutdown");
    let listed = daemon
        .list()
        .expect("registry list should load after shutdown");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_late_attach_drains_initial_history_before_later_live_output() {
    let data_dir = temp_data_dir("daemon-late-attach-history");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-late-history-session".to_string());
    let primary_client = ClientId("daemon-late-history-primary".to_string());
    let primary_subscription =
        SubscriptionId("daemon-late-history-primary-subscription".to_string());
    let late_client = ClientId("daemon-late-history-late".to_string());
    let late_subscription = SubscriptionId("daemon-late-history-late-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            primary_subscription,
            11,
        )
        .expect("initial attach should subscribe through CoreDaemon");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    daemon
        .input(
            primary_client,
            session_id.clone(),
            b"before-late-attach\n".to_vec(),
            12,
        )
        .expect("prior marker should write through CoreDaemon input");
    let primary_replay_source = drain_until(&mut daemon, &session_id, "echo:before-late-attach");
    assert!(
        renderable_output(&primary_replay_source.client_egress).contains("echo:before-late-attach"),
        "fixture must prove prior marker reached core terminal state before late attach"
    );

    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            13,
        )
        .expect("late attach should return initial route output");
    daemon
        .input(
            late_client.clone(),
            session_id.clone(),
            b"after-late-attach\n".to_vec(),
            14,
        )
        .expect("later live marker should still write after late attach");

    let late_drain = drain_until_for_client(
        &mut daemon,
        &session_id,
        &late_client,
        "echo:after-late-attach",
    );
    let mut combined_egress = late_attach.client_egress;
    combined_egress.extend(late_drain.client_egress);
    {
        let (snapshot_index, snapshot) = first_snapshot_for_client(&combined_egress, &late_client)
            .expect("late Ghostty attach should deliver an opaque snapshot replay");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:before-late-attach");
        let live_index = first_terminal_output_index_for_client_containing(
            &combined_egress,
            &late_client,
            "echo:after-late-attach",
        )
        .expect("late client should receive later live output");
        assert!(
            snapshot_index < live_index,
            "late Ghostty snapshot replay should precede later live output: {:?}",
            combined_egress
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn rapid_reattach_drops_pending_egress_for_detached_subscription() {
    let data_dir = temp_data_dir("rapid-reattach-drops-stale-egress");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("rapid-reattach-session".to_string());
    let client_id = ClientId("rapid-reattach-client".to_string());
    let old_subscription = SubscriptionId("rapid-reattach-old".to_string());
    let new_subscription = SubscriptionId("rapid-reattach-new".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            old_subscription.clone(),
            11,
        )
        .expect("first attach should succeed");
    daemon
        .detach(
            client_id.clone(),
            session_id.clone(),
            old_subscription.clone(),
            12,
        )
        .expect("detach should succeed before drain");
    let replacement = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            new_subscription.clone(),
            13,
        )
        .expect("replacement attach should succeed");

    let drained = daemon.drain(&session_id, 14).expect("drain attach egress");
    assert!(replacement
        .client_egress
        .iter()
        .all(|(received_client, frame)| {
            received_client != &client_id
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &old_subscription
                )
        }));
    assert!(drained
        .client_egress
        .iter()
        .all(|(received_client, frame)| {
            received_client != &client_id
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &new_subscription
                )
        }));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn guarded_write_states_are_fail_closed_and_write_through_input_path() {
    let data_dir = temp_data_dir("daemon-guarded");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-guarded-session".to_string());
    let client_id = ClientId("daemon-guarded-client".to_string());
    let subscription_id = SubscriptionId("daemon-guarded-subscription".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("daemon should attach");

    let mode_flags = ModeFlags {
        cursor_visible: true,
        ..ModeFlags::default()
    };
    let ready = daemon
        .guarded_write(GuardedWriteRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
            data: b"guarded\n".to_vec(),
            readiness: ReadinessEvidence::ready(mode_flags),
            now_seconds: 12,
        })
        .expect("ready guarded write should run");
    assert!(matches!(ready.decision, GuardedWriteDecision::Write));
    assert_eq!(
        ready.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Written
        ],
        "plain PTY injection must not fabricate delivered or acknowledged"
    );

    let drained = drain_until(&mut daemon, &session_id, "echo:guarded");
    assert!(terminal_output(&drained.client_egress).contains("echo:guarded"));

    let deferred = daemon
        .guarded_write(GuardedWriteRequest {
            session_id: session_id.clone(),
            client_id: client_id.clone(),
            data: b"deferred\n".to_vec(),
            readiness: ReadinessEvidence::default(),
            now_seconds: 13,
        })
        .expect("absent evidence should defer");
    assert!(matches!(
        deferred.decision,
        GuardedWriteDecision::Defer { .. }
    ));
    assert_eq!(
        deferred.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Deferred
        ]
    );

    let rejected = daemon
        .guarded_write(GuardedWriteRequest {
            session_id,
            client_id,
            data: b"rejected\n".to_vec(),
            readiness: ReadinessEvidence {
                safe_write: SafeWriteIndicator::Unsafe,
                ..ReadinessEvidence::default()
            },
            now_seconds: 14,
        })
        .expect("unsafe evidence should reject");
    assert!(matches!(
        rejected.decision,
        GuardedWriteDecision::Reject { .. }
    ));
    assert_eq!(
        rejected.states,
        vec![
            GuardedWriteDeliveryState::Accepted,
            GuardedWriteDeliveryState::Rejected
        ]
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_posts_drains_and_acknowledges_notifications() {
    let data_dir = temp_data_dir("daemon-notification-ack");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let id = NotificationId("daemon-notification-1".to_string());
    let target = notification_session_target("daemon-notification-session");

    let posted = daemon
        .post_notification(PostNotificationRequest {
            item: notification("daemon-notification-1", target.clone(), 10),
        })
        .expect("daemon should queue notification through CoreDaemon");
    assert_eq!(posted.id, id);
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Queued)
    );

    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("daemon should drain notification target");
    assert_eq!(drained.items.len(), 1);
    assert_eq!(drained.items[0].id, id);
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Delivered)
    );

    let acknowledged = daemon
        .acknowledge_notification(AcknowledgeNotificationRequest { id: id.clone() })
        .expect("daemon should acknowledge notification");
    assert_eq!(
        acknowledged.status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );
    assert_eq!(
        daemon.notification_status(&id).status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_drain_is_target_scoped_and_once_only() {
    let data_dir = temp_data_dir("daemon-notification-target-scope");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_target = notification_session_target("target-scope-session");
    let client_target = notification_client_target("target-scope-client");

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("session-notification", session_target.clone(), 10),
        })
        .expect("session notification should queue");
    daemon
        .post_notification(PostNotificationRequest {
            item: notification("client-notification", client_target.clone(), 10),
        })
        .expect("client notification should queue");

    let session_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: session_target.clone(),
            now: NotificationTimestamp(12),
        })
        .expect("session target should drain");
    let second_session_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: session_target,
            now: NotificationTimestamp(12),
        })
        .expect("session target second drain should run");
    let client_drain = daemon
        .drain_notifications(DrainNotificationsRequest {
            target: client_target,
            now: NotificationTimestamp(12),
        })
        .expect("client target should drain independently");

    assert_eq!(
        session_drain
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["session-notification"]
    );
    assert!(
        second_session_drain.items.is_empty(),
        "notification drains are one-shot per target"
    );
    assert_eq!(
        client_drain
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["client-notification"]
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_expiry_matches_core_inbox() {
    let data_dir = temp_data_dir("daemon-notification-expiry");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let target = notification_session_target("expiry-session");
    let expired_id = NotificationId("expired-notification".to_string());
    let live_id = NotificationId("live-notification".to_string());

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("expired-notification", target.clone(), 10)
                .with_expiry(NotificationTimestamp(20)),
        })
        .expect("expired fixture should queue");
    daemon
        .post_notification(PostNotificationRequest {
            item: notification("live-notification", target.clone(), 10)
                .with_expiry(NotificationTimestamp(40)),
        })
        .expect("live fixture should queue");

    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(30),
        })
        .expect("expiry drain should run");

    assert_eq!(
        drained
            .items
            .iter()
            .map(|item| item.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["live-notification"]
    );
    assert_eq!(
        daemon.notification_status(&expired_id).status,
        Some(NotificationDeliveryStatus::Expired)
    );
    assert_eq!(
        daemon.notification_status(&live_id).status,
        Some(NotificationDeliveryStatus::Delivered)
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_routed_envelope_cursor_ack_and_backpressure_are_exposed_when_needed() {
    let data_dir = temp_data_dir("daemon-routed-envelope");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_routed_envelope_queue(RoutedEnvelopeQueueConfig::new(1)),
    );
    let slow = envelope_endpoint("slow");
    let fast = envelope_endpoint("fast");

    let first = daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope("env-1", vec![slow.clone(), fast.clone()]),
        })
        .expect("first envelope should publish");
    assert_eq!(first.deliveries.len(), 2);

    let fast_first = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: fast.clone(),
            after: None,
            limit: 1,
        })
        .expect("fast target should drain first envelope");
    assert_eq!(fast_first.envelopes[0].id, EnvelopeId("env-1".to_string()));
    assert_eq!(fast_first.next_cursor, Some(EnvelopeCursor(2)));

    let second = daemon
        .publish_routed_envelope(PublishRoutedEnvelopeRequest {
            envelope: envelope("env-2", vec![slow.clone(), fast.clone()]),
        })
        .expect("second envelope should publish with slow pressure");
    assert!(second.observations.iter().any(|observation| {
        matches!(
            observation,
            RoutedEnvelopeObservation::Backpressured {
                envelope_id,
                target,
                capacity: 1,
                depth: 1
            } if envelope_id == &EnvelopeId("env-2".to_string()) && target == &slow
        )
    }));

    let fast_second = daemon
        .drain_routed_envelopes(DrainRoutedEnvelopesRequest {
            target: fast.clone(),
            after: fast_first.next_cursor,
            limit: 1,
        })
        .expect("cursor drain should deliver second fast envelope");
    assert_eq!(
        fast_second
            .envelopes
            .iter()
            .map(|envelope| envelope.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["env-2"]
    );

    let acknowledged = daemon
        .acknowledge_routed_envelope(AcknowledgeRoutedEnvelopeRequest {
            target: fast.clone(),
            envelope_id: EnvelopeId("env-2".to_string()),
        })
        .expect("fast envelope should acknowledge");
    assert_eq!(
        acknowledged
            .state
            .expect("delivery state should exist")
            .status,
        EnvelopeDeliveryStatus::Acknowledged
    );
    assert_eq!(
        daemon
            .routed_envelope_delivery_state(&slow, &EnvelopeId("env-2".to_string()))
            .state
            .expect("slow delivery state should exist")
            .status,
        EnvelopeDeliveryStatus::Backpressured
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notifications_work_for_worker_backed_daemon_without_worker_engine_notification_methods() {
    let data_dir = temp_data_dir("daemon-worker-notification");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let id = NotificationId("worker-notification".to_string());
    let target = notification_session_target("worker-notification-session");

    daemon
        .post_notification(PostNotificationRequest {
            item: notification("worker-notification", target.clone(), 10),
        })
        .expect("worker-backed daemon should queue notification");
    let drained = daemon
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("worker-backed daemon should drain notification");
    let acknowledged = daemon
        .acknowledge_notification(AcknowledgeNotificationRequest { id: id.clone() })
        .expect("worker-backed daemon should acknowledge notification");

    assert_eq!(drained.items.len(), 1);
    assert_eq!(drained.items[0].id, id);
    assert_eq!(
        acknowledged.status,
        Some(NotificationDeliveryStatus::Acknowledged)
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_read_screen_drains_before_read_and_preserves_client_egress_once() {
    let data_dir = temp_data_dir("dwrs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwrs-session".to_string());
    let client_id = ClientId("dwrs-client".to_string());
    let subscription_id = SubscriptionId("dwrs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"screen-marker\n".to_vec(),
            12,
        )
        .expect("marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:screen-marker", 13);
    assert_eq!(
        screen.screen.request_id,
        RequestId("read-screen-marker".to_string())
    );
    assert_eq!(screen.screen.session_id, session_id);
    assert!(
        screen.screen.text.contains("echo:screen-marker"),
        "read_screen should internally drain before reading: {:?}",
        screen.screen.text
    );

    let drained = daemon
        .drain(&session_id, 30)
        .expect("drain after read_screen should succeed");
    let output = terminal_output(&drained.client_egress);
    assert_eq!(
        count_occurrences(&output, "echo:screen-marker"),
        1,
        "internal read_screen drain must retain client egress exactly once: {output:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_capture_snapshot_drains_before_capture_and_preserves_client_egress_once() {
    let data_dir = temp_data_dir("dwcs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwcs-session".to_string());
    let client_id = ClientId("dwcs-client".to_string());
    let subscription_id = SubscriptionId("dwcs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"snapshot-marker\n".to_vec(),
            12,
        )
        .expect("marker input should write");

    let (captured, output) = capture_snapshot_and_retained_output_until(
        &mut daemon,
        &session_id,
        "echo:snapshot-marker",
        13,
    );
    assert_eq!(
        captured.snapshot.request_id,
        RequestId("capture-snapshot-marker".to_string())
    );
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_snapshot_format(&captured.payload);
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert_snapshot_payload_observed_marker_when_plain(&captured.payload, "echo:snapshot-marker");
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:snapshot-marker");
    assert_eq!(
        count_occurrences(&output, "echo:snapshot-marker"),
        1,
        "internal capture_snapshot drain must retain client egress exactly once: {output:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn local_daemon_read_screen_and_capture_snapshot_use_configured_terminal_backend() {
    let data_dir = temp_data_dir("dlrs");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("dlrs-session".to_string());
    let client_id = ClientId("dlrs-client".to_string());
    let subscription_id = SubscriptionId("dlrs-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("local daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("local daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"local-marker\n".to_vec(),
            12,
        )
        .expect("local marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:local-marker", 13);
    assert_eq!(screen.screen.session_id, session_id);
    assert!(
        screen.screen.text.contains("echo:local-marker"),
        "local daemon read_screen should use the configured terminal backend"
    );

    let captured = capture_snapshot_until(&mut daemon, &session_id, "echo:local-marker", 14);
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_snapshot_format(&captured.payload);
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert_snapshot_payload_observed_marker_when_plain(&captured.payload, "echo:local-marker");
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:local-marker");

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_default_path_uses_ghostty_terminal_fidelity() {
    let data_dir = temp_data_dir("dwgf");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgf-session".to_string());
    let client_id = ClientId("dwgf-client".to_string());
    let subscription_id = SubscriptionId("dwgf-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"\x1b[31mghostty-red\x1b[0m\n".to_vec(),
            12,
        )
        .expect("styled marker input should write");

    let screen = read_screen_until(&mut daemon, &session_id, "echo:ghostty-red", 13);
    assert!(
        !screen.screen.text.contains("\x1b["),
        "Ghostty screen reads should format terminal state as plain text, not raw VT bytes: {:?}",
        screen.screen.text
    );

    let captured = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("ghostty-fidelity-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("Ghostty-backed daemon should capture snapshot");
    assert_snapshot_format(&captured.payload);
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:ghostty-red");
    assert!(
        captured.payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        captured.payload.bytes.len()
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_ghostty_same_session_reattach_restores_retained_history() {
    let data_dir = temp_data_dir("dwgrh");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgrh-session".to_string());
    let original_client = ClientId("dwgrh-original-client".to_string());
    let original_subscription = SubscriptionId("dwgrh-original-subscription".to_string());
    let reattached_client = ClientId("dwgrh-reattached-client".to_string());
    let reattached_subscription = SubscriptionId("dwgrh-reattached-subscription".to_string());
    let prior_marker = "echo:dwgrh-prior-marker";
    let visible_marker = "echo:dwgrh-visible-marker";
    let live_marker = "echo:dwgrh-live-marker";

    daemon
        .spawn(retained_history_spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    let authoritative = read_screen_until(&mut daemon, &session_id, prior_marker, 11);
    assert!(
        authoritative.screen.text.contains(prior_marker),
        "authoritative terminal state must contain the startup marker before attach: {:?}",
        authoritative.screen.text
    );
    daemon
        .attach(
            original_client.clone(),
            session_id.clone(),
            original_subscription.clone(),
            12,
        )
        .expect("original client should attach");
    let _ = daemon
        .drain(&session_id, 13)
        .expect("original attach bootstrap should drain");
    daemon
        .input(
            original_client.clone(),
            session_id.clone(),
            b"dwgrh-visible-marker\n".to_vec(),
            14,
        )
        .expect("second visible marker should write");
    let _ = drain_until(&mut daemon, &session_id, visible_marker);

    let authoritative = read_screen_until(&mut daemon, &session_id, visible_marker, 15);
    assert!(
        authoritative.screen.text.contains(prior_marker),
        "authoritative terminal state must contain the prior marker before detach: {:?}",
        authoritative.screen.text
    );

    daemon
        .detach(
            original_client.clone(),
            session_id.clone(),
            original_subscription.clone(),
            16,
        )
        .expect("original client should detach");
    let reattached = daemon
        .attach(
            reattached_client.clone(),
            session_id.clone(),
            reattached_subscription.clone(),
            17,
        )
        .expect("fresh client and subscription should reattach the running session");
    let reattach_drain = drain_until_attached(&mut daemon, &session_id, &reattached_client);
    let mut reattach_egress = reattached.client_egress;
    reattach_egress.extend(reattach_drain.client_egress);
    let (snapshot_index, snapshot) =
        first_snapshot_for_client(&reattach_egress, &reattached_client)
            .expect("reattach should deliver the authoritative Ghostty snapshot");
    let snapshot_subscription = match &reattach_egress[snapshot_index] {
        (
            _,
            TransportEgress::Snapshot {
                subscription_id, ..
            },
        ) => subscription_id,
        _ => unreachable!("snapshot helper returned a non-snapshot frame"),
    };
    assert_eq!(
        snapshot_subscription, &reattached_subscription,
        "retained snapshot must target the fresh reattach subscription"
    );
    let replayed = ghostty_snapshot_plain_text(&snapshot);
    assert_eq!(
        count_occurrences(&replayed, prior_marker),
        1,
        "reattached snapshot should replay the prior marker exactly once: {replayed:?}"
    );
    assert_eq!(
        count_occurrences(&replayed, visible_marker),
        1,
        "reattached snapshot should replay the second visible marker exactly once: {replayed:?}"
    );
    assert!(
        replayed.find(prior_marker) < replayed.find(visible_marker),
        "reattached snapshot should preserve marker order: {replayed:?}"
    );

    let attaching_index = reattach_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &reattached_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &reattached_subscription
                )
        })
        .expect("fresh subscription should receive Attaching");
    let attached_index = reattach_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &reattached_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attached,
                        ..
                    } if subscription_id == &reattached_subscription
                )
        })
        .expect("fresh subscription should receive Attached");
    assert!(
        attaching_index < snapshot_index && snapshot_index < attached_index,
        "reattach bootstrap must order Attaching, retained snapshot, then Attached: {:?}",
        reattach_egress
    );
    assert!(
        reattach_egress
            .iter()
            .all(|(client_id, _)| client_id != &original_client),
        "detached client must not receive reattach-only delivery: {:?}",
        reattach_egress
    );

    let post_attach_drain = daemon
        .drain(&session_id, 18)
        .expect("post-attach drain should succeed");
    assert!(post_attach_drain
        .client_egress
        .iter()
        .all(|(client_id, frame)| {
            client_id != &reattached_client
                || !matches!(
                    frame,
                    TransportEgress::Snapshot { subscription_id, .. }
                        | TransportEgress::AttachState { subscription_id, .. }
                        if subscription_id == &reattached_subscription
                )
        }));

    daemon
        .input(
            reattached_client.clone(),
            session_id.clone(),
            b"dwgrh-live-marker\n".to_vec(),
            19,
        )
        .expect("post-attach live marker should write");
    let live_drain =
        drain_until_for_client(&mut daemon, &session_id, &reattached_client, live_marker);
    assert_eq!(
        count_occurrences(
            &renderable_output_for_client(&live_drain.client_egress, &reattached_client),
            live_marker,
        ),
        1,
        "post-attach live marker should be delivered exactly once"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_streams_ready_pages_finish_then_queued_work_and_live_output() {
    let data_dir = temp_data_dir("worker-incremental-contract");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1)),
    );
    let session_id = SessionId("worker-incremental-contract-session".to_string());
    let client_id = ClientId("worker-incremental-contract-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-contract-sub".to_string());
    let ready_path = data_dir.join("history-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'PRE-BARRIER-MARKER'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );

    daemon.spawn(request, 10).expect("spawn real worker");
    wait_for_file(&ready_path);
    let recovery = daemon
        .registry()
        .load(&session_id)
        .expect("load worker record")
        .and_then(|record| record.recovery_identity)
        .expect("worker recovery identity");
    assert_eq!(
        recovery
            .get("snapshot_delivery")
            .and_then(serde_json::Value::as_str),
        Some("ready_then_history")
    );

    let attached = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start incremental attach");
    assert_eq!(attached.client_egress.len(), 1);
    assert!(matches!(
        &attached.client_egress[0],
        (
            target,
            TransportEgress::AttachState {
                state: TerminalAttachState::Attaching,
                ..
            }
        ) if target == &client_id
    ));

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"POST-BARRIER-MARKER\n".to_vec(),
            13,
        )
        .expect("queue input");
    daemon
        .resize(client_id.clone(), session_id.clone(), 30, 90, 14)
        .expect("queue first resize");
    daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 15)
        .expect("replace queued resize");

    let mut projection = GhosttyClientProjection::new(TerminalScreenSize::new(24, 80))
        .expect("create incremental client");
    let mut sequence = vec!["attaching"];
    let mut history_frames = 0;
    let mut saw_ready = false;
    let mut saw_finish = false;
    let mut saw_attached = false;
    let mut all_egress = attached.client_egress;
    for tick in 0..10_000 {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("drain one incremental frame");
        let snapshots_in_drain = drained
            .client_egress
            .iter()
            .filter(|(target, frame)| {
                target == &client_id && matches!(frame, TransportEgress::Snapshot { .. })
            })
            .count();
        assert!(snapshots_in_drain <= 1, "one client-paced frame per drain");
        for (target, frame) in &drained.client_egress {
            if target != &client_id {
                continue;
            }
            match frame {
                TransportEgress::Snapshot { data, .. } if !saw_ready => {
                    assert_eq!(
                        projection
                            .install_ghostsnp_ready(data.clone())
                            .expect("READY must decode"),
                        GhosttySnapshotDecodeProgress::Ready
                    );
                    let ready_viewport = projection.project_viewport().expect("paint at READY");
                    assert_eq!(ready_viewport.rows, 24);
                    assert_eq!(ready_viewport.cols, 80);
                    assert!(viewport_contains_marker(
                        &ready_viewport,
                        "PRE-BARRIER-MARKER"
                    ));
                    saw_ready = true;
                    sequence.push("ready");
                }
                TransportEgress::Snapshot { data, .. } => {
                    match projection
                        .apply_ghostsnp_history(data.clone())
                        .expect("decode one PAGE or FINISH")
                    {
                        GhosttySnapshotDecodeProgress::History => {
                            history_frames += 1;
                            sequence.push("history");
                        }
                        GhosttySnapshotDecodeProgress::Finish => {
                            saw_finish = true;
                            sequence.push("finish");
                        }
                        GhosttySnapshotDecodeProgress::Ready => {
                            panic!("READY must occur once")
                        }
                    }
                }
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } => {
                    assert!(saw_finish, "Attached must follow FINISH");
                    saw_attached = true;
                    sequence.push("attached");
                }
                TransportEgress::TerminalOutput { .. } => {
                    assert!(saw_attached, "live output must follow Attached")
                }
                _ => {}
            }
        }
        all_egress.extend(drained.client_egress);
        if saw_attached {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(saw_ready, "worker must emit READY");
    assert!(history_frames > 0, "large history must emit PAGE frames");
    assert!(saw_finish, "FINISH must map from decoder NO_VALUE");
    assert!(saw_attached, "attach must complete");
    assert_eq!(sequence.first(), Some(&"attaching"));
    assert_eq!(sequence.last(), Some(&"attached"));

    let barrier_resized = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("incremental-barrier-resize-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 49,
        })
        .expect("capture immediately after Attached");
    assert_eq!(
        barrier_resized.payload.size,
        TerminalScreenSize::new(40, 120),
        "the worker must apply the latest resize before Attached"
    );

    let live = drain_until_for_client(
        &mut daemon,
        &session_id,
        &client_id,
        "echo:POST-BARRIER-MARKER",
    );
    all_egress.extend(live.client_egress);
    let attached_index = all_egress
        .iter()
        .position(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                }
            )
        })
        .expect("Attached index");
    let post_index = first_terminal_output_index_for_client_containing(
        &all_egress,
        &client_id,
        "echo:POST-BARRIER-MARKER",
    )
    .expect("queued input output");
    assert!(attached_index < post_index);

    let resized = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("incremental-resize-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 50,
        })
        .expect("capture after queued resize");
    assert_eq!(resized.payload.size, TerminalScreenSize::new(40, 120));
    assert_ghostty_snapshot_replays_marker(&resized.payload, "echo:POST-BARRIER-MARKER");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_blank_history_is_ready_finish_attached() {
    let data_dir = temp_data_dir("worker-incremental-blank");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-incremental-blank-session".to_string());
    let client_id = ClientId("worker-incremental-blank-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-blank-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn blank worker");
    let initial = daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 11)
        .expect("start blank attach");
    let drained = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut frames = initial.client_egress;
    frames.extend(drained.client_egress);

    let mut projection =
        GhosttyClientProjection::new(TerminalScreenSize::new(24, 80)).expect("blank client");
    let mut sequence = Vec::new();
    for (target, frame) in frames {
        if target != client_id {
            continue;
        }
        match frame {
            TransportEgress::AttachState {
                state: TerminalAttachState::Attaching,
                ..
            } => sequence.push("attaching"),
            TransportEgress::Snapshot { data, .. } if sequence == ["attaching"] => {
                assert_eq!(
                    projection
                        .install_ghostsnp_ready(data)
                        .expect("blank READY"),
                    GhosttySnapshotDecodeProgress::Ready
                );
                sequence.push("ready");
            }
            TransportEgress::Snapshot { data, .. } => {
                assert_eq!(
                    projection
                        .apply_ghostsnp_history(data)
                        .expect("blank FINISH"),
                    GhosttySnapshotDecodeProgress::Finish
                );
                sequence.push("finish");
            }
            TransportEgress::AttachState {
                state: TerminalAttachState::Attached,
                ..
            } => sequence.push("attached"),
            _ => {}
        }
    }
    assert_eq!(sequence, ["attaching", "ready", "finish", "attached"]);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_bound_adapter_receives_ready_finish_without_drain_snapshots() {
    let data_dir = temp_data_dir("worker-bound-adapter");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-bound-adapter-session".to_string());
    let client_id = ClientId("worker-bound-adapter-client".to_string());
    let subscription_id = SubscriptionId("worker-bound-adapter-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string();
    daemon.spawn(request, 10).expect("spawn bound worker");
    let initial = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("start bound attach");
    assert!(matches!(
        &initial.client_egress[0],
        (_, TransportEgress::AttachState { .. })
    ));
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("inventory after attach")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            generation,
            Box::new(adapter.clone()),
        )
        .expect("bind worker adapter");

    let started = Instant::now();
    let mut phases = Vec::new();
    let mut sent_live_input = false;
    let mut saw_live = false;
    while started.elapsed() < REAL_WORKER_COMPLETION_TIMEOUT {
        let drained = daemon.drain(&session_id, 20).expect("drain bound worker");
        assert!(
            drained.client_egress.iter().all(|(_, frame)| {
                !matches!(
                    frame,
                    TransportEgress::Snapshot {
                        subscription_id: route,
                        ..
                    }
                    | TransportEgress::TerminalOutput {
                        subscription_id: route,
                        ..
                    }
                    | TransportEgress::AttachState {
                        subscription_id: route,
                        ..
                    } if route == &subscription_id
                )
            }),
            "bound route must not appear on drain: {:?}",
            drained.client_egress
        );
        for bytes in adapter.snapshot_delivered_frame_bytes() {
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("opaque frame is JSON");
            match value.get("type").and_then(serde_json::Value::as_str) {
                Some("snapshot") => {
                    if let Some(phase) = value.get("phase").and_then(serde_json::Value::as_str) {
                        if !phases.iter().any(|seen| seen == phase) {
                            phases.push(phase.to_string());
                        }
                    }
                }
                Some("attach_state") => {
                    if value.get("state").and_then(serde_json::Value::as_str) == Some("attached")
                        && !phases.iter().any(|seen| seen == "attached")
                    {
                        phases.push("attached".to_string());
                    }
                }
                Some("terminal_output") => {
                    if sent_live_input {
                        saw_live = true;
                    }
                }
                _ => {}
            }
        }
        if phases.iter().any(|phase| phase == "attached") && !sent_live_input {
            daemon
                .input(
                    client_id.clone(),
                    session_id.clone(),
                    b"BOUND-LIVE\n".to_vec(),
                    21,
                )
                .expect("post-attach live input");
            sent_live_input = true;
        }
        if phases
            .windows(2)
            .any(|window| window == ["ready", "finish"])
            && saw_live
        {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        phases
            .windows(2)
            .any(|window| window == ["ready", "finish"]),
        "bound adapter must receive READY then FINISH: {phases:?}"
    );
    assert!(saw_live, "bound adapter must receive live output");
    assert!(daemon
        .list()
        .expect("list")
        .iter()
        .any(|row| row.session_id == session_id));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_pending_replacement_does_not_start_the_old_subscription() {
    let data_dir = temp_data_dir("worker-pending-replace");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-pending-replace-session".to_string());
    let active = ClientId("worker-pending-replace-active".to_string());
    let pending = ClientId("worker-pending-replace-pending".to_string());
    let active_sub = SubscriptionId("worker-pending-replace-active-sub".to_string());
    let old_sub = SubscriptionId("worker-pending-replace-old-sub".to_string());
    let new_sub = SubscriptionId("worker-pending-replace-new-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(active.clone(), session_id.clone(), active_sub.clone(), 11)
        .expect("active attach");
    daemon
        .attach(pending.clone(), session_id.clone(), old_sub.clone(), 12)
        .expect("queue old pending");
    daemon
        .attach(pending, session_id.clone(), new_sub.clone(), 13)
        .expect("replace pending");
    let live: Vec<_> = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .map(|row| row.subscription_id)
        .collect();
    assert!(live.contains(&active_sub));
    assert!(live.contains(&new_sub));
    assert!(
        !live.contains(&old_sub),
        "replaced pending subscription must not stay in inventory: {live:?}"
    );

    let started = Instant::now();
    let mut saw_old_after_replace = false;
    while started.elapsed() < Duration::from_secs(2) {
        let drained = daemon.drain(&session_id, 20).expect("drain");
        for (_, frame) in drained.client_egress {
            if matches!(
                frame,
                TransportEgress::Snapshot {
                    subscription_id,
                    ..
                }
                | TransportEgress::AttachState {
                    subscription_id,
                    ..
                } if subscription_id == old_sub
            ) {
                saw_old_after_replace = true;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !saw_old_after_replace,
        "old pending subscription must never start a snapshot boundary"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_owner_replacement_cancels_the_active_boundary() {
    let data_dir = temp_data_dir("worker-same-key-replace");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-same-key-replace-session".to_string());
    let first = ClientId("worker-same-key-replace-a".to_string());
    let second = ClientId("worker-same-key-replace-b".to_string());
    let subscription = SubscriptionId("worker-same-key-replace-sub".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = "printf ready; while IFS= read -r line; do :; done".to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), subscription.clone(), 11)
        .expect("attach first owner");
    let first_gen = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription)
        .expect("first inventory")
        .generation;
    let replaced = daemon
        .attach(second.clone(), session_id.clone(), subscription.clone(), 12)
        .expect("replace owner before first boundary finishes");
    assert!(
        replaced.client_egress.iter().any(|(client_id, frame)| {
            client_id == &second
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &subscription
                )
        }),
        "replacement must start its attach immediately: {:?}",
        replaced.client_egress
    );
    assert!(
        replaced
            .client_egress
            .iter()
            .all(|(client_id, _)| client_id != &first),
        "cancelled owner must not receive the replacement attach frames"
    );
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].client_id, second);
    assert_eq!(live[0].subscription_id, subscription);
    assert_eq!(
        live[0].generation,
        botster_core::TerminalSubscriptionGeneration(first_gen.0 + 1)
    );

    let mut saw_first_after_replace = false;
    let replacement = drain_until_attached(&mut daemon, &session_id, &second);
    for (client_id, frame) in &replacement.client_egress {
        if client_id == &first
            && matches!(
                frame,
                TransportEgress::Snapshot { .. } | TransportEgress::AttachState { .. }
            )
        {
            saw_first_after_replace = true;
        }
    }
    assert!(
        !saw_first_after_replace,
        "cancelled owner must not receive later snapshot or attach frames: {:?}",
        replacement.client_egress
    );
    assert!(
        replacement.client_egress.iter().any(|(client_id, frame)| {
            client_id == &second && matches!(frame, TransportEgress::Snapshot { .. })
        }),
        "replacement must start its own snapshot boundary: {:?}",
        replacement.client_egress
    );
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].client_id, second);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_takeover_preserves_pending_sibling_input_and_resize() {
    let data_dir = temp_data_dir("worker-takeover-sibling");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-takeover-sibling-session".to_string());
    let first = ClientId("worker-takeover-sibling-a".to_string());
    let second = ClientId("worker-takeover-sibling-b".to_string());
    let sibling = ClientId("worker-takeover-sibling-c".to_string());
    let first_sub = SubscriptionId("worker-takeover-sibling-sub-a".to_string());
    let sibling_sub = SubscriptionId("worker-takeover-sibling-sub-c".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first owner");
    daemon
        .attach(sibling.clone(), session_id.clone(), sibling_sub.clone(), 12)
        .expect("queue sibling");
    daemon
        .input(
            sibling.clone(),
            session_id.clone(),
            b"SIBLING-KEEP\n".to_vec(),
            13,
        )
        .expect("queue sibling input");
    daemon
        .resize(sibling.clone(), session_id.clone(), 30, 100, 14)
        .expect("queue sibling resize");
    daemon
        .attach(second.clone(), session_id.clone(), first_sub.clone(), 15)
        .expect("take over first key");
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert!(live
        .iter()
        .any(|row| row.client_id == second && row.subscription_id == first_sub));
    assert!(live
        .iter()
        .any(|row| row.client_id == sibling && row.subscription_id == sibling_sub));
    assert!(live.iter().all(|row| row.client_id != first));
    let _ = drain_until_attached(&mut daemon, &session_id, &second);
    let _ = drain_until_attached(&mut daemon, &session_id, &sibling);
    drain_until_terminal_marker(&mut daemon, &session_id, "echo:SIBLING-KEEP", 30);
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_same_key_takeover_drops_the_new_owners_obsolete_pending_subscription() {
    let data_dir = temp_data_dir("worker-takeover-stale-pending");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(0),
    );
    let session_id = SessionId("worker-takeover-stale-pending-session".to_string());
    let first = ClientId("worker-takeover-stale-pending-a".to_string());
    let second = ClientId("worker-takeover-stale-pending-b".to_string());
    let first_sub = SubscriptionId("worker-takeover-stale-pending-x".to_string());
    let stale_sub = SubscriptionId("worker-takeover-stale-pending-y".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(first.clone(), session_id.clone(), first_sub.clone(), 11)
        .expect("attach first owner");
    daemon
        .attach(second.clone(), session_id.clone(), stale_sub.clone(), 12)
        .expect("queue obsolete pending");
    daemon
        .attach(second.clone(), session_id.clone(), first_sub.clone(), 13)
        .expect("take over first key");
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert!(live
        .iter()
        .any(|row| row.client_id == second && row.subscription_id == first_sub));
    assert!(
        live.iter().all(|row| row.subscription_id != stale_sub),
        "obsolete pending subscription must leave inventory: {live:?}"
    );
    let mut saw_stale = false;
    let replacement = drain_until_attached(&mut daemon, &session_id, &second);
    for (_, frame) in &replacement.client_egress {
        if matches!(
            frame,
            TransportEgress::Snapshot {
                subscription_id,
                ..
            }
            | TransportEgress::AttachState {
                subscription_id,
                ..
            } if subscription_id == &stale_sub
        ) {
            saw_stale = true;
        }
    }
    assert!(
        !saw_stale,
        "obsolete pending subscription must never start a snapshot boundary: {:?}",
        replacement.client_egress
    );
    let live: Vec<_> = daemon.list_terminal_subscriptions();
    assert!(live.iter().all(|row| row.subscription_id != stale_sub));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_subscription_drain_retains_foreign_route_frames() {
    let data_dir = temp_data_dir("worker-route-drain");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-route-drain-session".to_string());
    let client_a = ClientId("worker-route-drain-a".to_string());
    let client_b = ClientId("worker-route-drain-b".to_string());
    let subscription_a = SubscriptionId("worker-route-drain-sub-a".to_string());
    let subscription_b = SubscriptionId("worker-route-drain-sub-b".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn route worker");
    daemon
        .attach(
            client_a.clone(),
            session_id.clone(),
            subscription_a.clone(),
            11,
        )
        .expect("attach route A");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_a);
    daemon
        .attach(
            client_b.clone(),
            session_id.clone(),
            subscription_b.clone(),
            12,
        )
        .expect("attach route B");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_b);
    daemon
        .input(
            client_a.clone(),
            session_id.clone(),
            b"ROUTE-DRAIN-MARKER\n".to_vec(),
            13,
        )
        .expect("write route marker");

    let mut route_a = botster_core_daemon::DrainResult::default();
    for tick in 0..100 {
        let drained = daemon
            .drain_subscription(&client_a, &session_id, &subscription_a, 20 + tick)
            .expect("drain route A");
        assert!(drained.client_egress.iter().all(|(target, frame)| {
            target == &client_a
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    }
                    | TransportEgress::Snapshot {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    }
                    | TransportEgress::AttachState {
                        session_id: routed_session,
                        subscription_id: routed_subscription,
                        ..
                    } if routed_session == &session_id
                        && routed_subscription == &subscription_a
                )
        }));
        route_a.client_egress.extend(drained.client_egress);
        if renderable_output_for_client(&route_a.client_egress, &client_a)
            .contains("echo:ROUTE-DRAIN-MARKER")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        renderable_output_for_client(&route_a.client_egress, &client_a)
            .contains("echo:ROUTE-DRAIN-MARKER")
    );

    let route_b = daemon
        .drain_subscription(&client_b, &session_id, &subscription_b, 200)
        .expect("drain retained route B");
    assert!(
        renderable_output_for_client(&route_b.client_egress, &client_b)
            .contains("echo:ROUTE-DRAIN-MARKER")
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_concurrent_attaches_serialize_without_pre_attached_live_output() {
    let data_dir = temp_data_dir("worker-concurrent-attach");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-concurrent-attach-session".to_string());
    let client_a = ClientId("worker-concurrent-attach-a".to_string());
    let client_b = ClientId("worker-concurrent-attach-b".to_string());
    let sub_a = SubscriptionId("worker-concurrent-attach-sub-a".to_string());
    let sub_b = SubscriptionId("worker-concurrent-attach-sub-b".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn concurrent worker");
    let first = daemon
        .attach(client_a.clone(), session_id.clone(), sub_a, 11)
        .expect("start first attach");
    let second = daemon
        .attach(client_b.clone(), session_id.clone(), sub_b, 12)
        .expect("queue second attach");
    daemon
        .input(
            client_b.clone(),
            session_id.clone(),
            b"CONCURRENT-POST\n".to_vec(),
            13,
        )
        .expect("queue second client input");

    let mut egress = first.client_egress;
    egress.extend(second.client_egress);
    let mut attached_a = false;
    let mut attached_b = false;
    for tick in 0..10_000 {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("drain serialized attaches");
        for (target, frame) in &drained.client_egress {
            match frame {
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } if target == &client_a => attached_a = true,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                } if target == &client_b => {
                    assert!(attached_a, "the first worker encode must finish first");
                    attached_b = true;
                }
                TransportEgress::TerminalOutput { .. } if target == &client_a => {
                    assert!(attached_a, "client A output must follow Attached")
                }
                TransportEgress::TerminalOutput { .. } if target == &client_b => {
                    assert!(attached_b, "client B output must follow Attached")
                }
                _ => {}
            }
        }
        egress.extend(drained.client_egress);
        if attached_b
            && renderable_output_for_client(&egress, &client_b).contains("echo:CONCURRENT-POST")
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(attached_a && attached_b);
    assert!(renderable_output_for_client(&egress, &client_b).contains("echo:CONCURRENT-POST"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_history_failure_reports_incomplete_then_attached() {
    let data_dir = temp_data_dir("worker-incremental-history-failure");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_fail_snapshot_history_after_ready(true),
    );
    let session_id = SessionId("worker-incremental-history-failure-session".to_string());
    let client_id = ClientId("worker-incremental-history-failure-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-history-failure-sub".to_string());
    let ready_path = data_dir.join("failure-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 1000 ]; do printf 'failure-history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'FAILURE-READY'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );
    daemon.spawn(request, 10).expect("spawn failure worker");
    wait_for_file(&ready_path);
    let initial = daemon
        .attach(client_id.clone(), session_id.clone(), subscription_id, 12)
        .expect("start failure attach");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"FAILURE-POST\n".to_vec(),
            13,
        )
        .expect("queue post-failure input");
    let drained = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut frames = initial.client_egress;
    frames.extend(drained.client_egress);
    let states = frames
        .iter()
        .filter_map(|(target, frame)| match frame {
            TransportEgress::AttachState { state, .. } if target == &client_id => Some(state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        [
            &TerminalAttachState::Attaching,
            &TerminalAttachState::SnapshotHistoryIncomplete,
            &TerminalAttachState::Attached,
        ]
    );
    assert_eq!(
        frames
            .iter()
            .filter(|(target, frame)| {
                target == &client_id && matches!(frame, TransportEgress::Snapshot { .. })
            })
            .count(),
        1,
        "history failure must occur after READY and before any PAGE delivery"
    );
    let live = drain_until_for_client(&mut daemon, &session_id, &client_id, "echo:FAILURE-POST");
    assert!(
        renderable_output_for_client(&live.client_egress, &client_id).contains("echo:FAILURE-POST")
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_incremental_attach_cancel_releases_snapshot_barrier() {
    let data_dir = temp_data_dir("worker-incremental-cancel");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_worker_egress_capacity(Some(1)),
    );
    let session_id = SessionId("worker-incremental-cancel-session".to_string());
    let client_id = ClientId("worker-incremental-cancel-client".to_string());
    let subscription_id = SubscriptionId("worker-incremental-cancel-sub".to_string());
    let ready_path = data_dir.join("cancel-ready");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "i=0; while [ $i -lt 2000 ]; do printf 'cancel-history-%04d\\n' \"$i\"; i=$((i+1)); done; ",
            "printf 'CANCEL-READY'; : > '{}'; ",
            "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
        ),
        ready_path.display()
    );
    daemon.spawn(request, 10).expect("spawn cancel worker");
    wait_for_file(&ready_path);
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            12,
        )
        .expect("start cancel attach");
    let ready = drain_until_snapshot(&mut daemon, &session_id, &client_id);
    assert_eq!(
        ready
            .client_egress
            .iter()
            .filter(|(_, frame)| matches!(frame, TransportEgress::Snapshot { .. }))
            .count(),
        1
    );
    daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 13)
        .expect("queue resize before cancel");
    daemon
        .detach(client_id, session_id.clone(), subscription_id, 14)
        .expect("cancel attach");

    let started = Instant::now();
    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("cancel-release-proof".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("capture after cancel");
    assert!(snapshot.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_eq!(snapshot.payload.size, TerminalScreenSize::new(24, 80));
    let record = daemon
        .registry()
        .load(&session_id)
        .expect("load registry after cancelled resize")
        .expect("worker registry record");
    assert_eq!((record.rows, record.cols), (24, 80));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "cancel must release the worker barrier"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_default_ghostty_path_replays_configured_scrollback_window() {
    let data_dir = temp_data_dir("dwgs");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwgs-session".to_string());
    let primary_client = ClientId("dwgs-primary-client".to_string());
    let late_client = ClientId("dwgs-late-client".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-primary-subscription".to_string()),
            11,
        )
        .expect("primary attach should succeed");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let mut scrollback_input = Vec::new();
    for line in 0..80 {
        scrollback_input.extend_from_slice(format!("scrollback-line-{line:04}\n").as_bytes());
    }
    daemon
        .input(
            primary_client.clone(),
            session_id.clone(),
            scrollback_input,
            12,
        )
        .expect("scrollback generator should write");
    let _ = drain_until(&mut daemon, &session_id, "echo:scrollback-line-0079");

    let visible = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("visible-scrollback-check".to_string()),
            session_id: session_id.clone(),
            now_seconds: 100,
        })
        .expect("daemon read_screen should succeed");
    assert!(
        visible.screen.text.contains("echo:scrollback-line-0000"),
        "default Ghostty read_screen should retain scrollback history beyond visible rows: {:?}",
        visible.screen.text
    );

    let late_subscription = SubscriptionId("dwgs-late-subscription".to_string());
    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            101,
        )
        .expect("late attach should receive a scrollback snapshot");
    let late_drain = drain_until_attached(&mut daemon, &session_id, &late_client);
    let mut late_egress = late_attach.client_egress;
    late_egress.extend(late_drain.client_egress);
    let (_, snapshot) = first_snapshot_for_client(&late_egress, &late_client)
        .expect("late Ghostty attach should include a snapshot frame");
    assert_ghostty_snapshot_replays_marker(&snapshot, "echo:scrollback-line-0000");

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_daemon_honors_host_ghostty_scrollback_byte_budget() {
    let data_dir = temp_data_dir("dwgs-override");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_ghostty_max_scrollback_bytes(LOW_GHOSTTY_MAX_SCROLLBACK_BYTES),
    );
    let session_id = SessionId("dwgs-override-session".to_string());
    let primary_client = ClientId("dwgs-override-primary-client".to_string());
    let late_client = ClientId("dwgs-override-late-client".to_string());
    let marker_count = 4_500;

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-override-primary-subscription".to_string()),
            11,
        )
        .expect("primary attach should succeed");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    for chunk_start in (0..marker_count).step_by(10) {
        let chunk_end = (chunk_start + 10).min(marker_count);
        let mut scrollback_input = Vec::new();
        for line in chunk_start..chunk_end {
            scrollback_input.extend_from_slice(format!("scrollback-line-{line:05}\n").as_bytes());
        }
        daemon
            .input(
                primary_client.clone(),
                session_id.clone(),
                scrollback_input,
                12 + chunk_start as u64,
            )
            .expect("scrollback generator chunk should write");
        let chunk_marker = format!("echo:scrollback-line-{:05}", chunk_end - 1);
        drain_until_terminal_marker(
            &mut daemon,
            &session_id,
            &chunk_marker,
            20 + chunk_start as u64,
        );
    }
    let newest_marker = format!("echo:scrollback-line-{:05}", marker_count - 1);

    let late_subscription = SubscriptionId("dwgs-override-late-subscription".to_string());
    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            101,
        )
        .expect("late attach should receive a scrollback snapshot");
    let late_drain = drain_until_attached(&mut daemon, &session_id, &late_client);
    let mut late_egress = late_attach.client_egress;
    late_egress.extend(late_drain.client_egress);
    let (_, snapshot) = first_snapshot_for_client(&late_egress, &late_client)
        .expect("late Ghostty attach should include a snapshot frame");
    let plain_text = ghostty_snapshot_plain_text(&snapshot);
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, marker_count);
    let retained_marker_count = retained_markers.len();
    let replayed_text_length = plain_text.len();
    let retains_newest_marker = plain_text.contains(&newest_marker);

    daemon
        .shutdown(Some(session_id), 103)
        .expect("worker-backed daemon should shut down");
    let _ = fs::remove_dir_all(data_dir);

    assert!(
        retained_marker_count > 0,
        "low Ghostty byte budget should remain above the page-allocation floor"
    );
    assert!(
        retained_marker_count < EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
        "host override should retain fewer markers than the default budget's pinned minimum; retained markers: {}; replayed text length: {}",
        retained_marker_count,
        replayed_text_length
    );
    assert!(
        retains_newest_marker,
        "low Ghostty byte budget should retain the newest marker"
    );
}

#[cfg(unix)]
#[test]
fn daemon_default_ghostty_scrollback_byte_budget_pins_effective_window() {
    let mut terminal = GhosttyTerminal::with_config(
        TerminalScreenSize::new(24, 80),
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty terminal with daemon default config");
    for chunk_start in (0..12_000).step_by(100) {
        let mut output = Vec::new();
        for line in chunk_start..chunk_start + 100 {
            output.extend_from_slice(format!("echo:scrollback-line-{line:05}\n").as_bytes());
        }
        terminal.write_output_bytes(&output);
        std::thread::sleep(Duration::from_millis(1));
    }

    let plain_text = terminal
        .plain_text()
        .expect("test should format Ghostty terminal text");
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, 12_000);
    assert!(
        retained_markers.len() >= EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
        "default Ghostty byte budget should retain a material history window at 24x80; retained markers: {}; text length: {}",
        retained_markers.len(),
        plain_text.len()
    );
    assert!(
        !plain_text.contains(EXPECTED_GHOSTTY_DROPPED_MARKER),
        "default Ghostty byte budget should drop history beyond the configured window at 24x80; text length: {}",
        plain_text.len()
    );

    let snapshot = terminal
        .export_snapshot()
        .expect("test should export Ghostty snapshot");
    assert_ghostty_snapshot_replays_minimum_markers(
        &snapshot,
        12_000,
        EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS,
    );
    assert_ghostty_snapshot_does_not_replay_marker(&snapshot, EXPECTED_GHOSTTY_DROPPED_MARKER);
}

#[cfg(unix)]
#[test]
fn worker_backed_late_attach_and_read_screen_pending_drains_merge_in_order() {
    let data_dir = temp_data_dir("dwrsl");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("dwrsl-session".to_string());
    let primary_client = ClientId("dwrsl-primary".to_string());
    let primary_subscription = SubscriptionId("dwrsl-primary-sub".to_string());
    let late_client = ClientId("dwrsl-late".to_string());
    let late_subscription = SubscriptionId("dwrsl-late-sub".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            primary_client.clone(),
            session_id.clone(),
            primary_subscription,
            11,
        )
        .expect("primary attach should subscribe");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            primary_client,
            session_id.clone(),
            b"worker-before-late\n".to_vec(),
            12,
        )
        .expect("history marker should write");
    let _ = drain_until(&mut daemon, &session_id, "echo:worker-before-late");

    let late_attach = daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription.clone(),
            13,
        )
        .expect("late attach should return initial history");
    daemon
        .input(
            late_client.clone(),
            session_id.clone(),
            b"worker-after-read\n".to_vec(),
            14,
        )
        .expect("post-read marker should write");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:worker-after-read", 15);

    let late_drain = drain_until_for_client(
        &mut daemon,
        &session_id,
        &late_client,
        "echo:worker-after-read",
    );
    let mut combined_egress = late_attach.client_egress;
    combined_egress.extend(late_drain.client_egress);
    let attaching_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attaching,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should emit Attaching");
    let snapshot_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::Snapshot {
                        subscription_id,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should deliver initial history");
    let attached_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        subscription_id,
                        state: TerminalAttachState::Attached,
                        ..
                    } if subscription_id == &late_subscription
                )
        })
        .expect("late worker-backed attach should emit Attached after history");
    let live_index = combined_egress
        .iter()
        .position(|(client_id, frame)| {
            client_id == &late_client
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        subscription_id,
                        data,
                        ..
                    } if subscription_id == &late_subscription
                        && String::from_utf8_lossy(data).contains("echo:worker-after-read")
                )
        })
        .expect("late worker-backed client should receive later live output");
    assert!(
        attaching_index < snapshot_index
            && snapshot_index < attached_index
            && attached_index < live_index,
        "worker-backed readiness must order Attaching before history before Attached before live output: {:?}",
        combined_egress
    );
    {
        let (snapshot_index, snapshot) = first_snapshot_for_client(&combined_egress, &late_client)
            .expect("late Ghostty attach should return an opaque snapshot replay");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:worker-before-late");
        let live_index = first_terminal_output_index_for_client_containing(
            &combined_egress,
            &late_client,
            "echo:worker-after-read",
        )
        .expect("read_screen internal drain should remain pending");
        assert!(
            snapshot_index < live_index,
            "attach snapshot replay should merge before read_screen pending drain: {:?}",
            combined_egress
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_attach_during_alt_screen_resize_gets_complete_current_state() {
    for iteration in 0..5 {
        let data_dir = temp_data_dir(&format!("atomic-resize-attach-{iteration}"));
        let midpoint = data_dir.join("redraw-midpoint");
        let mut daemon = CoreDaemon::new(
            CoreDaemonConfig::new(&data_dir)
                .with_worker_path(worker_path())
                .with_pty_reader_chunk_capacity(Some(1))
                .with_test_hold_after_read_ms(Some(2))
                .with_test_worker_egress_capacity(Some(4)),
        );
        let session_id = SessionId(format!("atomic-resize-attach-{iteration}"));
        let primary_client = ClientId(format!("atomic-primary-{iteration}"));
        let late_client = ClientId(format!("atomic-late-{iteration}"));
        let mut request = spawn_request(&session_id);
        request.request.arguments[1] = format!(
            concat!(
                "trap 'printf \"\\033[?1049h\\033[2J\"; ",
                "i=0; while [ $i -lt 100 ]; do ",
                "printf \"\\033]0;filler-%03d\\007\" \"$i\"; i=$((i+1)); done; ",
                "i=1; while [ $i -le 49 ]; do ",
                "printf \"\\033[%d;1HROW-%02d-COMPLETE\" \"$i\" \"$i\"; ",
                "sleep 0.003; i=$((i+1)); done; ",
                "printf done > \"{}\"; sleep 1; ",
                "printf \"\\033[50;1HROW-50-COMPLETE\"' WINCH; ",
                "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            ),
            midpoint.display()
        );

        daemon.spawn(request, 10).expect("spawn real worker");
        daemon
            .attach(
                primary_client.clone(),
                session_id.clone(),
                SubscriptionId(format!("atomic-primary-sub-{iteration}")),
                11,
            )
            .expect("primary attach");
        let _ = drain_until(&mut daemon, &session_id, "ready");
        daemon
            .resize(primary_client.clone(), session_id.clone(), 50, 152, 12)
            .expect("production resize");

        let deadline = Instant::now() + Duration::from_secs(10);
        while !midpoint.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            midpoint.exists(),
            "child must reach the controlled redraw midpoint"
        );

        let late_attach = daemon
            .attach(
                late_client.clone(),
                session_id.clone(),
                SubscriptionId(format!("atomic-late-sub-{iteration}")),
                13,
            )
            .expect("attach during alternate-screen redraw");

        let drained =
            drain_until_for_client(&mut daemon, &session_id, &late_client, "ROW-50-COMPLETE");
        assert!(
            drained.client_egress.iter().any(|(client_id, frame)| {
                client_id == &primary_client
                    && matches!(frame, TransportEgress::TerminalOutput { .. })
            }),
            "attach must retain pre-boundary live output for the existing client"
        );
        let mut combined_egress = late_attach.client_egress;
        combined_egress.extend(drained.client_egress);
        let (snapshot_index, snapshot) = first_snapshot_for_client_at_size(
            &combined_egress,
            &late_client,
            TerminalScreenSize::new(50, 152),
        )
        .expect("late attach must receive GHOSTSNP");
        assert_ghostty_snapshot_authority(&snapshot);
        let snapshot_text = ghostty_snapshot_plain_text(&snapshot);
        assert!(
            snapshot_text.contains("ROW-49-COMPLETE"),
            "snapshot must contain the pre-boundary redraw prefix"
        );
        assert!(
            !snapshot_text.contains("ROW-50-COMPLETE"),
            "row 50 must remain a post-boundary live-output suffix"
        );
        let mut client = GhosttyTerminal::with_config(
            snapshot.size,
            GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
        )
        .expect("client Ghostty");
        client.import_snapshot(&snapshot).expect("install GHOSTSNP");
        for (index, (client_id, frame)) in combined_egress.iter().enumerate() {
            if index <= snapshot_index {
                continue;
            }
            if client_id == &late_client {
                if let TransportEgress::TerminalOutput { data, .. } = frame {
                    client.write_output_bytes(data);
                }
            }
        }
        let text = client.plain_text().expect("render client state");
        for row in 1..=50 {
            assert!(
                text.contains(&format!("ROW-{row:02}-COMPLETE")),
                "iteration {iteration} missed row {row}; text={text:?}"
            );
        }
        assert_eq!(client.size(), TerminalScreenSize::new(50, 152));
        assert!(client.read_mode_flags().expect("mode flags").alt_screen);

        let screen = daemon
            .read_screen(ReadScreenRequest {
                request_id: RequestId(format!("atomic-screen-{iteration}")),
                session_id: session_id.clone(),
                now_seconds: 14,
            })
            .expect("parent screen after attach boundary");
        assert!(screen.screen.text.contains("ROW-50-COMPLETE"));
        daemon.shutdown(Some(session_id), 15).ok();
        let _ = fs::remove_dir_all(data_dir);
    }
}

#[cfg(unix)]
#[test]
fn worker_backed_duplicate_attach_refreshes_same_subscription_with_current_snapshot() {
    let data_dir = temp_data_dir("worker-duplicate-attach-refresh");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-duplicate-attach-session".to_string());
    let client_id = ClientId("worker-duplicate-attach-client".to_string());
    let subscription_id = SubscriptionId("worker-duplicate-attach-subscription".to_string());

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn real worker");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("first attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"duplicate-current-state\n".to_vec(),
            12,
        )
        .expect("write current-state marker");
    let _ = drain_until(&mut daemon, &session_id, "echo:duplicate-current-state");

    let duplicate_attach = daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            13,
        )
        .expect("duplicate attach must refresh the route");
    let duplicate_drain = drain_until_attached(&mut daemon, &session_id, &client_id);
    let mut duplicate_egress = duplicate_attach.client_egress;
    duplicate_egress.extend(duplicate_drain.client_egress);
    let (_, snapshot) = first_snapshot_for_client(&duplicate_egress, &client_id)
        .expect("duplicate attach must deliver a fresh GHOSTSNP");
    assert_ghostty_snapshot_replays_marker(&snapshot, "echo:duplicate-current-state");
    assert!(duplicate_egress.iter().all(|(received_client, frame)| {
        received_client == &client_id
            && matches!(
                frame,
                TransportEgress::Snapshot {
                    subscription_id: received_subscription,
                    ..
                } | TransportEgress::AttachState {
                    subscription_id: received_subscription,
                    ..
                } if received_subscription == &subscription_id
            )
    }));
    let attaching_index = duplicate_egress
        .iter()
        .position(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attaching,
                    ..
                }
            )
        })
        .expect("duplicate attach must return Attaching");
    let snapshot_index = duplicate_egress
        .iter()
        .position(|(_, frame)| matches!(frame, TransportEgress::Snapshot { .. }))
        .expect("duplicate attach must return Snapshot");
    let attached_index = duplicate_egress
        .iter()
        .position(|(_, frame)| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    state: TerminalAttachState::Attached,
                    ..
                }
            )
        })
        .expect("duplicate attach must return Attached");
    assert!(attaching_index < snapshot_index && snapshot_index < attached_index);
    assert!(duplicate_egress.iter().any(|(received_client, frame)| {
        received_client == &client_id
            && matches!(
                frame,
                TransportEgress::AttachState {
                    subscription_id: received_subscription,
                    state: TerminalAttachState::Attached,
                    ..
                } if received_subscription == &subscription_id
            )
    }));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_rapid_switch_attach_always_builds_complete_current_screen() {
    let data_dir = temp_data_dir("worker-rapid-switch-attach");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-rapid-switch-session".to_string());
    let client_id = ClientId("worker-rapid-switch-client".to_string());
    let prefix_ack = data_dir.join("prefix-ack");
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = format!(
        concat!(
            "stty -echo; printf '\\033[?1049h\\033[2J'; ",
            "i=1; while [ $i -le 23 ]; do ",
            "printf '\\033[%d;1HSWITCH-ROW-%02d-COMPLETE' \"$i\" \"$i\"; ",
            "i=$((i+1)); done; ",
            "printf '\\033[24;1HREADY'; ",
            "while IFS= read -r line; do ",
            "printf '\\033[24;1H%-40s' \"$line\"; ",
            "printf '%s' \"$line\" > \"{}\"; done"
        ),
        prefix_ack.display()
    );

    daemon.spawn(request, 10).expect("spawn real worker");
    let initial_subscription = SubscriptionId("rapid-switch-a".to_string());
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            initial_subscription,
            11,
        )
        .expect("initial attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);

    for iteration in 0..20 {
        let subscription_id = SubscriptionId(format!("rapid-switch-{}", (iteration / 2) % 2));
        let prefix_marker = format!("PREFIX-SWITCH-{iteration:02}");
        daemon
            .input(
                client_id.clone(),
                session_id.clone(),
                format!("{prefix_marker}\n").into_bytes(),
                19 + iteration,
            )
            .expect("pre-attach marker");
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(&prefix_ack).ok().as_deref() != Some(prefix_marker.as_str())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            fs::read_to_string(&prefix_ack).ok().as_deref(),
            Some(prefix_marker.as_str()),
            "child must publish the pre-attach marker before attach"
        );
        if iteration % 3 == 0 {
            let transient = SubscriptionId(format!("rapid-switch-transient-{iteration}"));
            daemon
                .attach(
                    client_id.clone(),
                    session_id.clone(),
                    transient.clone(),
                    20 + iteration,
                )
                .expect("transient attach");
            daemon
                .detach(
                    client_id.clone(),
                    session_id.clone(),
                    transient,
                    21 + iteration,
                )
                .expect("transient detach before drain");
        }
        let attached = daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                22 + iteration,
            )
            .expect("rapid attach");

        let live_marker = format!("LIVE-SWITCH-{iteration:02}");
        daemon
            .input(
                client_id.clone(),
                session_id.clone(),
                format!("{live_marker}\n").into_bytes(),
                23 + iteration,
            )
            .expect("post-attach live marker");
        let drained = drain_until_for_client(&mut daemon, &session_id, &client_id, &live_marker);
        let mut combined_egress = attached.client_egress;
        combined_egress.extend(drained.client_egress);
        assert!(combined_egress.iter().all(|(received_client, frame)| {
            received_client != &client_id
                || match frame {
                    TransportEgress::TerminalOutput {
                        subscription_id: received,
                        ..
                    }
                    | TransportEgress::Snapshot {
                        subscription_id: received,
                        ..
                    }
                    | TransportEgress::AttachState {
                        subscription_id: received,
                        ..
                    } => received == &subscription_id,
                    _ => true,
                }
        }));

        let (snapshot_index, snapshot) = first_snapshot_for_client_at_size(
            &combined_egress,
            &client_id,
            TerminalScreenSize::new(24, 80),
        )
        .expect("each attach must deliver a current GHOSTSNP");
        assert!(ghostty_snapshot_plain_text(&snapshot).contains(&prefix_marker));
        assert!(combined_egress[..snapshot_index]
            .iter()
            .all(|(received_client, frame)| received_client != &client_id
                || !matches!(frame, TransportEgress::TerminalOutput { .. })));
        assert_ghostty_snapshot_authority(&snapshot);
        let mut client = GhosttyTerminal::with_config(
            snapshot.size,
            GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
        )
        .expect("fresh client Ghostty");
        client.import_snapshot(&snapshot).expect("install GHOSTSNP");
        for (index, (received_client, frame)) in combined_egress.iter().enumerate() {
            if index > snapshot_index && received_client == &client_id {
                if let TransportEgress::TerminalOutput { data, .. } = frame {
                    client.write_output_bytes(data);
                }
            }
        }
        let text = client.plain_text().expect("render client state");
        for row in 1..=23 {
            assert!(
                text.contains(&format!("SWITCH-ROW-{row:02}-COMPLETE")),
                "iteration {iteration} missed row {row}; text={text:?}"
            );
        }
        assert!(text.contains(&live_marker));
        assert!(client.read_mode_flags().expect("mode flags").alt_screen);
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_screen_and_snapshot_retain_shutdown_truth_and_keep_negative_paths() {
    let data_dir = temp_data_dir("dssn");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let missing_session = SessionId("missing-rb-session".to_string());

    assert!(matches!(
        daemon.read_screen(ReadScreenRequest {
            request_id: RequestId("missing-screen".to_string()),
            session_id: missing_session.clone(),
            now_seconds: 10,
        }),
        Err(CoreDaemonError::UnknownSession(session)) if session == missing_session
    ));
    assert!(matches!(
        daemon.capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("missing-snapshot".to_string()),
            session_id: missing_session.clone(),
            now_seconds: 10,
        }),
        Err(CoreDaemonError::UnknownSession(session)) if session == missing_session
    ));

    let session_id = SessionId("dssn-empty-session".to_string());
    let client_id = ClientId("dssn-client".to_string());
    daemon
        .spawn(mode_flags_spawn_request(&session_id), 11)
        .expect("worker-backed daemon should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("dssn-subscription".to_string()),
            12,
        )
        .expect("worker-backed daemon should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"shutdown-final\n".to_vec(),
            13,
        )
        .expect("shutdown marker input should write");
    let screen = read_screen_until(&mut daemon, &session_id, "echo:shutdown-final", 14);
    assert_eq!(screen.screen.session_id, session_id);
    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("empty-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("spawned never explicitly drained session should still capture snapshot");
    assert_eq!(snapshot.snapshot.session_id, session_id);
    assert_snapshot_format(&snapshot.payload);
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"enable-mouse\n".to_vec(),
            16,
        )
        .expect("mouse mode DECSET should write");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-mouse", 17);
    let live_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("live-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 18,
        })
        .expect("live mode flags should be authoritative");
    assert_eq!(live_mode_flags.mode_flags.mode_flags.mouse_mode, 9);

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should succeed");
    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("shutdown screen should serve retained terminal truth");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-snapshot-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("shutdown snapshot should serve retained terminal truth");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 23,
        })
        .expect("shutdown screen should be repeatable");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-snapshot-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 24,
        })
        .expect("shutdown snapshot should be repeatable");
    let retained_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("retained-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 25,
        })
        .expect("shutdown mode flags should serve retained terminal truth");

    assert!(first_screen.screen.text.contains("echo:shutdown-final"));
    assert_eq!(retained_mode_flags.mode_flags.mode_flags.mouse_mode, 9);
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_ne!(
        first_screen.screen.request_id,
        second_screen.screen.request_id
    );
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_ne!(
        first_snapshot.snapshot.request_id,
        second_snapshot.snapshot.request_id
    );

    assert!(daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"late-input\n".to_vec(),
            25,
        )
        .is_err());
    assert!(daemon
        .resize(client_id.clone(), session_id.clone(), 40, 120, 26)
        .is_err());
    assert!(daemon
        .attach(
            client_id,
            session_id.clone(),
            SubscriptionId("dssn-late-subscription".to_string()),
            27,
        )
        .is_err());
    let unchanged = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen-unchanged".to_string()),
            session_id: session_id.clone(),
            now_seconds: 28,
        })
        .expect("failed mutation attempts must not change retained truth");
    assert_eq!(unchanged.screen.text, first_screen.screen.text);

    daemon
        .mark_stale(&session_id, 29)
        .expect("test should mark retained registry record stale");
    assert!(matches!(
        daemon.read_screen(ReadScreenRequest {
            request_id: RequestId("stale-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 30,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
    ));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn natural_exit_read_screen_and_capture_snapshot_freeze_repeatable_truth() {
    let data_dir = temp_data_dir("dnex");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    let screen_session = SessionId("dnex-screen-session".to_string());
    daemon
        .spawn(self_exit_spawn_request(&screen_session), 10)
        .expect("screen self-exit session should spawn");
    daemon
        .attach(
            ClientId("dnex-screen-client".to_string()),
            screen_session.clone(),
            SubscriptionId("dnex-screen-subscription".to_string()),
            11,
        )
        .expect("screen self-exit session should attach");
    let _ = drain_until(&mut daemon, &screen_session, "ready");
    daemon
        .input(
            ClientId("dnex-screen-client".to_string()),
            screen_session.clone(),
            b"screen-exit\n".to_vec(),
            12,
        )
        .expect("screen self-exit input should write");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-screen-1".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 13,
        })
        .expect("first natural-exit read_screen should freeze and serve terminal truth");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-screen-snapshot-1".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 14,
        })
        .expect("natural-exit snapshot should reuse retained truth");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-screen-2".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 15,
        })
        .expect("natural-exit screen should be repeatable");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-screen-snapshot-2".to_string()),
            session_id: screen_session.clone(),
            now_seconds: 16,
        })
        .expect("natural-exit snapshot should be repeatable");
    assert!(first_screen.screen.text.contains("echo:screen-exit"));
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_ne!(
        first_screen.screen.request_id,
        second_screen.screen.request_id
    );
    assert_ne!(
        first_snapshot.snapshot.request_id,
        second_snapshot.snapshot.request_id
    );
    let screen_drain = daemon
        .drain(&screen_session, 17)
        .expect("drain after retained read_screen should return final output");
    assert_retained_exit_output(&screen_drain, &screen_session, "echo:screen-exit");
    let second_screen_drain = daemon
        .drain(&screen_session, 18)
        .expect("second drain after retained readback should succeed");
    assert_no_duplicate_exit_output(&second_screen_drain, "echo:screen-exit");

    let snapshot_session = SessionId("dnex-snapshot-session".to_string());
    daemon
        .spawn(self_exit_spawn_request(&snapshot_session), 20)
        .expect("snapshot self-exit session should spawn");
    daemon
        .attach(
            ClientId("dnex-snapshot-client".to_string()),
            snapshot_session.clone(),
            SubscriptionId("dnex-snapshot-subscription".to_string()),
            21,
        )
        .expect("snapshot self-exit session should attach");
    let _ = drain_until(&mut daemon, &snapshot_session, "ready");
    daemon
        .input(
            ClientId("dnex-snapshot-client".to_string()),
            snapshot_session.clone(),
            b"snapshot-exit\n".to_vec(),
            22,
        )
        .expect("snapshot self-exit input should write");
    std::thread::sleep(std::time::Duration::from_millis(500));

    let snapshot_result = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-1".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 23,
        })
        .expect("first natural-exit capture_snapshot should freeze and serve terminal truth");
    let snapshot_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("natural-exit-snapshot-screen".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 24,
        })
        .expect("screen should share snapshot-triggered retained truth");
    let repeated_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-2".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 25,
        })
        .expect("snapshot-triggered retained truth should be repeatable");
    assert!(snapshot_screen.screen.text.contains("echo:snapshot-exit"));
    assert_eq!(snapshot_result.payload, repeated_snapshot.payload);
    assert_ne!(
        snapshot_result.snapshot.request_id,
        repeated_snapshot.snapshot.request_id
    );
    let snapshot_drain = daemon
        .drain(&snapshot_session, 26)
        .expect("drain after retained capture_snapshot should return final output");
    assert_retained_exit_output(&snapshot_drain, &snapshot_session, "echo:snapshot-exit");
    let second_snapshot_drain = daemon
        .drain(&snapshot_session, 27)
        .expect("second drain after retained snapshot should succeed");
    assert_no_duplicate_exit_output(&second_snapshot_drain, "echo:snapshot-exit");

    let registry_json =
        fs::read_to_string(data_dir.join("sessions").join("dnex-snapshot-session.json"))
            .expect("registry JSON should remain readable");
    assert!(!registry_json.contains("echo:snapshot-exit"));
    drop(daemon);
    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    assert!(matches!(
        restarted.read_screen(ReadScreenRequest {
            request_id: RequestId("restart-screen".to_string()),
            session_id: snapshot_session.clone(),
            now_seconds: 28,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == snapshot_session
    ));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_reconciles_a_worker_natural_exit_after_output_is_observable() {
    let data_dir = short_temp_data_dir("shutdown-race");
    let session_id = SessionId("shutdown-race-session".to_string());
    let client_id = ClientId("shutdown-race-client".to_string());
    let subscription_id = SubscriptionId("shutdown-race-subscription".to_string());
    let marker_path = data_dir.join("marker");
    let release_path = data_dir.join("release");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(
            controlled_exit_spawn_request(&session_id, &marker_path, &release_path),
            10,
        )
        .expect("controlled natural-exit session should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("controlled natural-exit session should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should emit the marker");
    wait_for_condition("fixture marker file", || marker_path.exists());
    let marker_screen = read_screen_until(
        &mut daemon,
        &session_id,
        "botster-core-natural-exit-marker",
        13,
    );
    assert!(marker_screen
        .screen
        .text
        .contains("botster-core-natural-exit-marker"));
    let observed_output = daemon
        .drain(&session_id, 14)
        .expect("observable marker output should remain drainable");
    assert!(terminal_output(&observed_output.client_egress)
        .contains("botster-core-natural-exit-marker"));
    assert_eq!(
        daemon.list().expect("list pre-exit session")[0].registry_state,
        RegistrySessionState::Running
    );

    let (_, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    fs::write(&release_path, b"release").expect("release controlled natural exit");
    wait_for_condition("worker process and control route completion", || {
        !process_exists(pty_child_pid) && UnixStream::connect(&socket_path).is_err()
    });
    assert_eq!(
        daemon.list().expect("list unreconciled session")[0].registry_state,
        RegistrySessionState::Running
    );

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should reconcile the completed worker");
    daemon
        .shutdown(Some(session_id.clone()), 21)
        .expect("repeated terminal shutdown should remain idempotent");

    let recovered = daemon
        .drain(&session_id, 22)
        .expect("attached client should drain shutdown recovery egress");
    assert!(
        recovered
            .client_egress
            .iter()
            .all(|(frame_client_id, _)| frame_client_id == &client_id),
        "shutdown recovery egress must remain targeted to the attached client: {:?}",
        recovered.client_egress
    );
    let tail_index = recovered
        .client_egress
        .iter()
        .position(|(frame_client_id, frame)| {
            frame_client_id == &client_id
                && matches!(
                    frame,
                    TransportEgress::TerminalOutput {
                        session_id: frame_session_id,
                        subscription_id: frame_subscription_id,
                        data,
                    } if frame_session_id == &session_id
                        && frame_subscription_id == &subscription_id
                        && String::from_utf8_lossy(data)
                            .contains("botster-core-natural-exit-tail")
                )
        })
        .expect("shutdown recovery should preserve the final terminal tail");
    let process_exits = recovered
        .client_egress
        .iter()
        .enumerate()
        .filter_map(|(index, (frame_client_id, frame))| match frame {
            TransportEgress::ProcessExit {
                session_id: frame_session_id,
                subscription_id: frame_subscription_id,
                code,
            } if frame_client_id == &client_id
                && frame_session_id == &session_id
                && frame_subscription_id == &subscription_id =>
            {
                Some((index, *code))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        process_exits.len(),
        1,
        "shutdown recovery should deliver exactly one ProcessExit: {:?}",
        recovered.client_egress
    );
    assert_eq!(
        process_exits[0].1,
        Some(0),
        "shutdown recovery should deliver the successful exit code"
    );
    assert!(
        tail_index < process_exits[0].0,
        "final terminal output must precede ProcessExit: {:?}",
        recovered.client_egress
    );

    let reconciled = daemon.list().expect("list reconciled session");
    assert_eq!(reconciled[0].registry_state, RegistrySessionState::Exited);
    assert!(matches!(
        daemon
            .lifecycle_baseline()
            .expect("lifecycle baseline")
            .sessions[0]
            .lifecycle,
        Some(SessionLifecycleState::Exited { code: Some(0) })
    ));

    let first_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-race-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 23,
        })
        .expect("retained terminal screen");
    let second_screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-race-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 24,
        })
        .expect("repeat retained terminal screen");
    let first_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-race-snapshot-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 25,
        })
        .expect("retained terminal snapshot");
    let second_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-race-snapshot-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 26,
        })
        .expect("repeat retained terminal snapshot");

    assert!(first_screen
        .screen
        .text
        .contains("botster-core-natural-exit-tail"));
    assert_eq!(first_screen.screen.text, second_screen.screen.text);
    assert_eq!(first_snapshot.payload, second_snapshot.payload);
    assert_snapshot_format(&first_snapshot.payload);
    assert_snapshot_payload_observed_marker_when_plain(
        &first_snapshot.payload,
        "botster-core-natural-exit-tail",
    );
    assert_ghostty_snapshot_replays_marker(
        &first_snapshot.payload,
        "botster-core-natural-exit-tail",
    );
    let repeated_drain = daemon
        .drain(&session_id, 27)
        .expect("shutdown recovery egress should be delivered exactly once");
    assert!(
        repeated_drain.client_egress.iter().all(|(_, frame)| {
            !matches!(
                frame,
                TransportEgress::TerminalOutput { data, .. }
                    if String::from_utf8_lossy(data)
                        .contains("botster-core-natural-exit-tail")
            ) && !matches!(frame, TransportEgress::ProcessExit { .. })
        }),
        "second drain must not duplicate the recovered tail or ProcessExit: {:?}",
        repeated_drain.client_egress
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn shutdown_all_continues_after_a_raced_natural_exit_and_cleans_a_live_session() {
    let data_dir = short_temp_data_dir("shutdown-all-race");
    let raced_session = SessionId("shutdown-all-raced".to_string());
    let live_session = SessionId("shutdown-all-live".to_string());
    let marker_path = data_dir.join("marker");
    let release_path = data_dir.join("release");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(
            controlled_exit_spawn_request(&raced_session, &marker_path, &release_path),
            10,
        )
        .expect("controlled natural-exit session should spawn");
    daemon
        .attach(
            ClientId("shutdown-all-raced-client".to_string()),
            raced_session.clone(),
            SubscriptionId("shutdown-all-raced-subscription".to_string()),
            11,
        )
        .expect("controlled natural-exit session should attach");
    let _ = drain_until(&mut daemon, &raced_session, "ready");
    daemon
        .input(
            ClientId("shutdown-all-raced-client".to_string()),
            raced_session.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should emit the marker");
    wait_for_condition("shutdown-all fixture marker", || marker_path.exists());
    let _ = read_screen_until(
        &mut daemon,
        &raced_session,
        "botster-core-natural-exit-marker",
        13,
    );
    let (_, raced_child_pid, raced_socket_path) = worker_process_evidence(&daemon, &raced_session);
    fs::write(&release_path, b"release").expect("release controlled natural exit");
    wait_for_condition("shutdown-all raced worker completion", || {
        !process_exists(raced_child_pid) && UnixStream::connect(&raced_socket_path).is_err()
    });

    daemon
        .spawn(spawn_request(&live_session), 20)
        .expect("live session should spawn");
    let (_, live_child_pid, live_socket_path) = worker_process_evidence(&daemon, &live_session);

    daemon
        .shutdown(None, 30)
        .expect("raced and live sessions should both shut down");
    let listed = daemon.list().expect("list shutdown-all sessions");
    assert_eq!(listed.len(), 2);
    assert!(listed
        .iter()
        .all(|record| record.registry_state == RegistrySessionState::Exited));
    assert!(!process_exists(live_child_pid));
    assert!(!live_socket_path.exists());

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn daemon_notification_state_is_not_restart_durable_today() {
    let data_dir = temp_data_dir("daemon-notification-not-durable");
    let target = notification_session_target("non-durable-session");
    {
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        daemon
            .post_notification(PostNotificationRequest {
                item: notification("non-durable-notification", target.clone(), 10),
            })
            .expect("first daemon should queue in-memory notification");
    }

    let mut restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let drained = restarted
        .drain_notifications(DrainNotificationsRequest {
            target,
            now: NotificationTimestamp(12),
        })
        .expect("fresh daemon should have an empty in-memory inbox");

    assert!(
        drained.items.is_empty(),
        "daemon notification/envelope state is in-memory and not restart durable today"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_restart_adopts_live_worker_and_reattaches() {
    let data_dir = temp_data_dir("daemon-restart-adopts-live-worker");
    let session_id = SessionId("daemon-restart-session".to_string());
    let client_id = ClientId("daemon-restart-client".to_string());
    let subscription_id = SubscriptionId("daemon-restart-subscription".to_string());

    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("first daemon should spawn live session");
        let mut record = daemon
            .registry()
            .load(&session_id)
            .expect("registry load should succeed")
            .expect("spawn should persist restart evidence");
        assert!(
            record
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("worker_control_socket"))
                .is_some(),
            "worker-backed daemon should persist a reconnectable worker endpoint"
        );
        record
            .recovery_identity
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
            .expect("recovery identity object")
            .remove("atomic_snapshot_boundary");
        daemon
            .registry()
            .save(&record)
            .expect("persist a legacy v2 worker record without the capability");
        daemon.release_for_restart();
    }

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let reports = restarted
        .adoption_scan()
        .expect("restarted daemon should scan registry");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].state, SessionAdoptionState::Adoptable);
    restarted
        .adopt_session(&session_id, 12)
        .expect("fresh daemon should adopt live worker process");

    let listed = restarted
        .list()
        .expect("restarted daemon should list durable sessions");
    assert_eq!(listed[0].session_id, session_id);
    assert_eq!(listed[0].registry_state, RegistrySessionState::Running);

    restarted
        .attach(client_id.clone(), session_id.clone(), subscription_id, 13)
        .expect("restarted daemon should attach through live engine route");
    restarted
        .input(
            client_id.clone(),
            session_id.clone(),
            b"after-restart\n".to_vec(),
            14,
        )
        .expect("restarted daemon should send input through adopted route");
    let drained = drain_until(&mut restarted, &session_id, "echo:after-restart");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:after-restart"),
        "reattached daemon should drain live worker output: {output:?}"
    );

    restarted
        .shutdown(Some(session_id.clone()), 30)
        .expect("restarted daemon should shut down adopted session");
    let listed = restarted
        .list()
        .expect("registry list should load after adopted shutdown");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn production_worker_root_handles_canonical_and_long_session_ids() {
    let data_dir = temp_data_dir("production-worker-id-length");
    let canonical = SessionId("123e4567-e89b-12d3-a456-426614174000".to_string());
    let long = SessionId(format!("sess-long-{}", "x".repeat(180)));
    let canonical_client = ClientId("canonical-client".to_string());
    let long_client = ClientId("long-client".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));

    daemon
        .spawn(spawn_request(&canonical), 10)
        .expect("spawn canonical worker-backed session");
    daemon
        .spawn(spawn_request(&long), 11)
        .expect("spawn long-id worker-backed session");
    let listed = daemon.list().expect("list both worker-backed sessions");
    assert!(listed.iter().any(|session| session.session_id == canonical));
    assert!(listed.iter().any(|session| session.session_id == long));

    let (canonical_worker, canonical_pty, canonical_socket) =
        worker_process_evidence(&daemon, &canonical);
    let (long_worker, long_pty, long_socket) = worker_process_evidence(&daemon, &long);
    let worker_root = canonical_socket
        .parent()
        .expect("canonical worker socket root")
        .to_path_buf();
    assert_eq!(long_socket.parent(), Some(worker_root.as_path()));
    assert!(worker_root.starts_with(std::env::temp_dir()));
    assert_ne!(canonical_socket, long_socket);
    assert_eq!(
        canonical_socket
            .file_name()
            .expect("canonical basename")
            .len(),
        27
    );
    assert_eq!(long_socket.file_name().expect("long basename").len(), 27);
    assert!(
        canonical_socket.as_os_str().len() <= 103,
        "canonical production endpoint must fit macOS SUN_LEN: {canonical_socket:?}"
    );
    assert!(
        long_socket.as_os_str().len() <= 103,
        "long production endpoint must fit macOS SUN_LEN: {long_socket:?}"
    );

    for (session, client, subscription, marker, now) in [
        (
            &canonical,
            &canonical_client,
            "canonical-subscription",
            "canonical-production-marker",
            12,
        ),
        (
            &long,
            &long_client,
            "long-subscription",
            "long-production-marker",
            13,
        ),
    ] {
        daemon
            .attach(
                client.clone(),
                session.clone(),
                SubscriptionId(subscription.to_string()),
                now,
            )
            .expect("attach worker-backed session");
        daemon
            .input(
                client.clone(),
                session.clone(),
                format!("{marker}\n").into_bytes(),
                now + 1,
            )
            .expect("send marker through worker-backed session");
        let drained = drain_until_for_client(&mut daemon, session, client, marker);
        assert!(
            renderable_output_for_client(&drained.client_egress, client).contains(marker),
            "worker-backed session should read back its own marker"
        );
    }

    daemon
        .shutdown(Some(long.clone()), 30)
        .expect("shut down long-id session");
    daemon
        .shutdown(Some(canonical.clone()), 31)
        .expect("shut down canonical session");
    wait_for_condition("production worker and PTY cleanup", || {
        !process_exists(canonical_worker)
            && !process_exists(long_worker)
            && !process_exists(canonical_pty)
            && !process_exists(long_pty)
            && !canonical_socket.exists()
            && !long_socket.exists()
    });
    assert!(
        !worker_root.exists(),
        "worker-owned production root should be removed when empty"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn adoption_of_live_process_with_reaped_socket_fails_without_rebinding() {
    let data_dir = temp_data_dir("reaped-worker-socket");
    let session_id = SessionId("reaped-worker-session".to_string());
    let mut original =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    original
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker before simulated socket reaping");
    let (worker_pid, pty_pid, socket_path) = worker_process_evidence(&original, &session_id);
    original.release_for_restart();
    assert!(process_exists(worker_pid));
    assert!(process_exists(pty_pid));
    fs::remove_file(&socket_path).expect("simulate macOS reaping the live socket pathname");

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let error = restarted
        .adopt_session(&session_id, 12)
        .expect_err("missing persisted endpoint must not create a replacement worker");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(
            botster_core::SessionRuntimeError {
                kind: botster_core::SessionRuntimeErrorKind::SpawnFailed,
                ref message,
            }
        )) if message.starts_with("connect worker control socket failed: ")
    ));
    assert!(
        !socket_path.exists(),
        "adoption must not bind a replacement endpoint"
    );
    assert!(
        process_exists(worker_pid),
        "adoption must not replace or kill worker"
    );

    original
        .shutdown(Some(session_id.clone()), 20)
        .expect("the connected owner should still shut down the reaped worker");
    assert!(!process_exists(worker_pid));
    assert!(!process_exists(pty_pid));
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_lifecycle_source_drives_projection_through_exit_and_removal() {
    let data_dir = temp_data_dir("lifecycle-source-exit-removal");
    let session_id = SessionId("lifecycle-source-session".to_string());
    let client_id = ClientId("lifecycle-source-client".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(8),
    );

    let baseline = daemon
        .lifecycle_baseline()
        .expect("empty lifecycle baseline");
    assert!(baseline.sessions.is_empty());

    daemon
        .spawn(self_exit_spawn_request(&session_id), 10)
        .expect("worker-backed lifecycle fixture should spawn");
    let (_, _, lifecycle_socket) = worker_process_evidence(&daemon, &session_id);
    let lifecycle_worker_root = lifecycle_socket
        .parent()
        .expect("worker socket parent")
        .to_path_buf();
    let running = daemon.lifecycle_changes(&baseline.cursor);
    assert!(running.resync_required.is_none());
    assert_eq!(running.changes.len(), 1);
    assert!(matches!(
        &running.changes[0].kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
                && matches!(record.lifecycle, Some(SessionLifecycleState::Running))
    ));
    assert!(!daemon
        .remove_session(&session_id)
        .expect("live session removal should be rejected without mutation"));

    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("lifecycle-source-subscription".to_string()),
            11,
        )
        .expect("fixture should attach through the production daemon facade");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"finish\n".to_vec(),
            12,
        )
        .expect("fixture input should cause natural process exit");

    let mut terminal_drain = botster_core_daemon::DrainResult::default();
    let exited = (0..100).find_map(|tick| {
        let drained = daemon
            .drain(&session_id, 20 + tick)
            .expect("natural-exit drain should succeed");
        terminal_drain.client_egress.extend(drained.client_egress);
        terminal_drain.observations.extend(drained.observations);
        terminal_drain.backpressure.extend(drained.backpressure);
        let changes = daemon.lifecycle_changes(&running.cursor);
        let observed_exit = changes.changes.iter().any(|change| {
            matches!(
                &change.kind,
                SessionLifecycleChangeKind::Upsert { record }
                    if record.session.session_id == session_id
                        && record.session.registry_state == RegistrySessionState::Exited
                        && matches!(
                            record.lifecycle,
                            Some(SessionLifecycleState::Exited { code: Some(0) })
                        )
            )
        });
        if observed_exit {
            Some(changes)
        } else {
            std::thread::sleep(Duration::from_millis(10));
            None
        }
    });
    let exited = exited.expect("natural exit should publish one terminal lifecycle upsert");
    assert!(terminal_output(&terminal_drain.client_egress).contains("echo:finish"));
    assert_eq!(exited.changes.len(), 1);
    assert!(
        !lifecycle_worker_root.exists(),
        "natural terminal transition should remove the empty worker root"
    );

    let empty = daemon.lifecycle_changes(&exited.cursor);
    assert!(empty.changes.is_empty());
    assert!(empty.resync_required.is_none());
    let _ = daemon
        .drain(&session_id, 200)
        .expect("repeat terminal drain should remain reachable");
    assert!(daemon.lifecycle_changes(&exited.cursor).changes.is_empty());

    assert!(daemon
        .remove_session(&session_id)
        .expect("host should explicitly forget the terminal session"));
    let removed = daemon.lifecycle_changes(&exited.cursor);
    assert_eq!(removed.changes.len(), 1);
    assert!(matches!(
        &removed.changes[0].kind,
        SessionLifecycleChangeKind::Removed { session_id: removed_id }
            if removed_id == &session_id
    ));
    assert!(daemon
        .lifecycle_baseline()
        .expect("post-removal baseline")
        .sessions
        .is_empty());
    assert!(matches!(
        daemon.drain(&session_id, 201),
        Err(CoreDaemonError::UnknownSession(id)) if id == session_id
    ));

    daemon
        .spawn(spawn_request(&session_id), 210)
        .expect("same stable id should be reusable after complete removal");
    daemon
        .attach(
            client_id,
            session_id.clone(),
            SubscriptionId("lifecycle-source-reused-subscription".to_string()),
            211,
        )
        .expect("removed subscription state must not block a fresh attach");
    let fresh = drain_until(&mut daemon, &session_id, "ready");
    assert!(!terminal_output(&fresh.client_egress).contains("echo:finish"));
    assert!(fresh.observations.iter().all(|observation| !matches!(
        observation,
        BotsterEngineObservation::Subscription(
            botster_core::SubscriptionMultiplexerObservation::ClientStream {
                observation: botster_core::ClientStreamObservation::ReplacedSubscription { .. },
                ..
            }
        )
    )));

    daemon
        .shutdown(Some(session_id.clone()), 220)
        .expect("fresh worker should shut down cleanly");
    assert!(daemon
        .remove_session(&session_id)
        .expect("fresh terminal worker should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_lifecycle_source_orders_shutdown_and_requires_overflow_resync() {
    let data_dir = temp_data_dir("lifecycle-source-overflow");
    let session_id = SessionId("lifecycle-overflow-session".to_string());
    let client_id = ClientId("lifecycle-overflow-client".to_string());
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_lifecycle_journal_capacity(2),
    );
    let baseline = daemon
        .lifecycle_baseline()
        .expect("empty overflow baseline");

    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("overflow fixture should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("lifecycle-overflow-subscription".to_string()),
            11,
        )
        .expect("overflow fixture should attach");
    let _ = drain_until_attached(&mut daemon, &session_id, &client_id);
    daemon
        .resize(client_id.clone(), session_id.clone(), 25, 80, 12)
        .expect("first material row update");
    daemon
        .resize(client_id, session_id.clone(), 26, 81, 13)
        .expect("second material row update");

    let overflow = daemon.lifecycle_changes(&baseline.cursor);
    assert!(overflow.changes.is_empty());
    assert!(matches!(
        overflow.resync_required,
        Some(SessionLifecycleResyncReason::CursorExpired { .. })
    ));
    let refreshed = daemon
        .lifecycle_baseline()
        .expect("overflow recovery baseline");
    assert_eq!(refreshed.sessions.len(), 1);
    assert_eq!(refreshed.sessions[0].session.size.rows, 26);
    assert_eq!(refreshed.sessions[0].session.size.cols, 81);

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("explicit shutdown should complete");
    let terminal = daemon.lifecycle_changes(&refreshed.cursor);
    assert!(terminal.resync_required.is_none());
    let terminal_states: Vec<_> = terminal
        .changes
        .iter()
        .filter_map(|change| match &change.kind {
            SessionLifecycleChangeKind::Upsert { record } => {
                Some(record.session.registry_state.clone())
            }
            _ => None,
        })
        .collect();
    assert!(matches!(
        terminal_states.as_slice(),
        [RegistrySessionState::Exited]
            | [RegistrySessionState::Stopping, RegistrySessionState::Exited]
    ));
    assert!(terminal
        .changes
        .windows(2)
        .all(|pair| pair[0].cursor.sequence < pair[1].cursor.sequence));

    assert!(daemon
        .remove_session(&session_id)
        .expect("shutdown session should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_restart_invalidates_cursor_and_adopts_same_session_id() {
    let data_dir = temp_data_dir("lifecycle-source-restart");
    let session_id = SessionId("lcr".to_string());
    let metadata = CoreSessionMetadata::from_entries(BTreeMap::from([
        ("host.example/class".to_string(), "interactive".to_string()),
        ("host.example/source".to_string(), "embedded".to_string()),
    ]));
    let mut projection = BTreeMap::new();
    let old_cursor = {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        let baseline = serialized_lifecycle_baseline(
            &daemon
                .lifecycle_baseline()
                .expect("first-generation baseline"),
        );
        replace_lifecycle_projection(&mut projection, &baseline);
        let mut request = spawn_request(&session_id);
        request.metadata = metadata.clone();
        let spawned = daemon
            .spawn(request, 10)
            .expect("first generation should spawn worker");
        assert_eq!(spawned.metadata, metadata);
        let running = serialized_lifecycle_changes(&daemon.lifecycle_changes(&baseline.cursor));
        apply_lifecycle_changes(&mut projection, &running);
        assert_eq!(
            projection
                .get(&session_id.0)
                .expect("spawn upsert should populate consumer projection")
                .metadata,
            metadata
        );
        let current = serialized_lifecycle_baseline(
            &daemon
                .lifecycle_baseline()
                .expect("current first-generation baseline"),
        );
        assert_eq!(current.sessions[0].metadata, metadata);
        let cursor = running.cursor;
        daemon.release_for_restart();
        cursor
    };

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let restarted_baseline = serialized_lifecycle_baseline(
        &restarted
            .lifecycle_baseline()
            .expect("restart baseline should expose durable registry truth"),
    );
    replace_lifecycle_projection(&mut projection, &restarted_baseline);
    assert_eq!(restarted_baseline.sessions.len(), 1);
    assert_eq!(
        restarted_baseline.sessions[0].session.session_id,
        session_id
    );
    assert_eq!(restarted_baseline.sessions[0].metadata, metadata);
    assert!(restarted_baseline.sessions[0].lifecycle.is_none());
    let foreign = restarted.lifecycle_changes(&old_cursor);
    assert!(foreign.changes.is_empty());
    assert_eq!(
        foreign.resync_required,
        Some(SessionLifecycleResyncReason::SourceChanged)
    );

    let adopted_session = restarted
        .adopt_session(&session_id, 12)
        .expect("fresh daemon should adopt from real worker protocol evidence");
    assert_eq!(adopted_session.metadata, metadata);
    let adopted =
        serialized_lifecycle_changes(&restarted.lifecycle_changes(&restarted_baseline.cursor));
    apply_lifecycle_changes(&mut projection, &adopted);
    assert_eq!(adopted.changes.len(), 1);
    assert!(matches!(
        &adopted.changes[0].kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
                && matches!(record.lifecycle, Some(SessionLifecycleState::Running))
                && record.metadata == metadata
    ));
    let post_adoption = serialized_lifecycle_baseline(
        &restarted
            .lifecycle_baseline()
            .expect("post-adoption baseline"),
    );
    assert_eq!(
        post_adoption.sessions.len(),
        1,
        "adoption must not fabricate a duplicate session"
    );
    assert_eq!(post_adoption.sessions[0].metadata, metadata);
    assert_eq!(projection.len(), 1);
    assert_eq!(
        projection
            .get(&session_id.0)
            .expect("adoption upsert should update the same projected row")
            .metadata,
        metadata
    );

    restarted
        .shutdown(Some(session_id.clone()), 20)
        .expect("adopted worker should shut down cleanly");
    assert!(restarted
        .remove_session(&session_id)
        .expect("adopted terminal worker should be removable"));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn metadata_free_registry_and_lifecycle_json_default_to_empty_metadata() {
    let registry_record = botster_core_daemon::RegistryRecord::running(
        SessionId("legacy-registry".to_string()),
        None,
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        1,
    );
    let mut registry_json = serde_json::to_value(&registry_record).expect("serialize registry");
    registry_json
        .as_object_mut()
        .expect("registry JSON object")
        .remove("metadata");
    let decoded_registry: botster_core_daemon::RegistryRecord =
        serde_json::from_value(registry_json).expect("decode legacy registry JSON");
    assert_eq!(decoded_registry.metadata, CoreSessionMetadata::new());

    let lifecycle_record = SessionLifecycleRecord {
        session: DaemonSession::from(&registry_record),
        metadata: CoreSessionMetadata::new(),
        lifecycle: None,
    };
    let mut lifecycle_json =
        serde_json::to_value(&lifecycle_record).expect("serialize lifecycle record");
    lifecycle_json
        .as_object_mut()
        .expect("lifecycle JSON object")
        .remove("metadata");
    let decoded_lifecycle: SessionLifecycleRecord =
        serde_json::from_value(lifecycle_json).expect("decode legacy lifecycle JSON");
    assert_eq!(decoded_lifecycle.metadata, CoreSessionMetadata::new());
}

#[cfg(unix)]
#[test]
fn oversized_persisted_metadata_fails_adoption_without_touching_the_live_worker() {
    let data_dir = temp_data_dir("oversized-adoption-metadata");
    let session_id = SessionId("oversized-adoption".to_string());
    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("spawn worker before editing persisted metadata");
        daemon.release_for_restart();
    }

    let record_path = data_dir.join("sessions").join("oversized-adoption.json");
    let valid_record = fs::read(&record_path).expect("read valid registry record for cleanup");
    let mut record: botster_core_daemon::RegistryRecord =
        serde_json::from_slice(&valid_record).expect("decode persisted registry record");
    record.metadata = CoreSessionMetadata::from_entries(BTreeMap::from([(
        "host.example/oversized".to_string(),
        "x".repeat(MAX_CORE_SESSION_METADATA_LEN + 1),
    )]));
    fs::write(
        &record_path,
        serde_json::to_vec_pretty(&record).expect("encode oversized registry record"),
    )
    .expect("write oversized registry record");
    let oversized_record = fs::read(&record_path).expect("read oversized registry bytes");

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let cursor = restarted
        .lifecycle_baseline()
        .expect("baseline before failed adoption")
        .cursor;
    let error = restarted
        .adopt_session(&session_id, 12)
        .expect_err("oversized persisted metadata must fail loudly");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Multiplexer(
            botster_core::MultiplexerEngineError::MetadataTooLarge
        ))
    ));
    assert_eq!(
        fs::read(&record_path).expect("read registry after failed adoption"),
        oversized_record,
        "failed adoption must not mutate persisted metadata"
    );
    assert!(restarted.lifecycle_changes(&cursor).changes.is_empty());

    fs::write(&record_path, valid_record).expect("repair persisted metadata after rejection");
    drop(restarted);
    let mut cleanup =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    cleanup
        .adopt_session(&session_id, 13)
        .expect("rejected adoption must leave the repaired worker adoptable");
    cleanup
        .shutdown(Some(session_id.clone()), 14)
        .expect("cleanup adopted worker");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_shutdown_waits_for_delayed_progress_before_release_can_preserve_nothing() {
    let data_dir = short_temp_data_dir("delayed");
    let session_id = SessionId("delay".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker-backed session");
    let (worker_pid, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    let mut stopped = StoppedProcess::new(worker_pid);
    let resume = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        signal_process(worker_pid, "CONT");
    });

    let started = Instant::now();
    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should complete after worker progress resumes");
    resume.join().expect("resume worker thread");
    stopped.resumed = true;

    assert!(
        started.elapsed() >= Duration::from_millis(150),
        "shutdown must not report success while the worker is stopped"
    );
    let listed = daemon.list().expect("list completed worker session");
    assert_eq!(listed[0].registry_state, RegistrySessionState::Exited);
    let first = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("delayed-final-screen-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("read retained final screen");
    let second = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("delayed-final-screen-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("repeat retained final screen");
    assert_eq!(first.screen.text, second.screen.text);

    daemon.release_for_restart();
    drop(daemon);
    assert!(!process_exists(worker_pid));
    assert!(!process_exists(pty_child_pid));
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_shutdown_timeout_is_typed_and_keeps_non_exited_cleanup_ownership() {
    let data_dir = short_temp_data_dir("timeout");
    let session_id = SessionId("timeout".to_string());
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn worker-backed session");
    let (worker_pid, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    let mut stopped = StoppedProcess::new(worker_pid);

    let error = daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect_err("stopped worker must not produce truthful completion");
    assert!(matches!(
        error,
        CoreDaemonError::Engine(botster_core::ManagedSessionRuntimeError::Runtime(
            botster_core::SessionRuntimeError {
                kind: botster_core::SessionRuntimeErrorKind::ShutdownFailed,
                ..
            }
        ))
    ));
    let listed = daemon.list().expect("list timed-out worker session");
    assert_ne!(listed[0].registry_state, RegistrySessionState::Exited);
    assert!(process_exists(worker_pid));
    assert!(socket_path.exists());

    stopped.resume();
    for tick in 0..500 {
        let _ = daemon.drain(&session_id, 30 + tick);
        let listed = daemon.list().expect("list resumed worker session");
        if listed[0].registry_state == RegistrySessionState::Exited {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        daemon.list().expect("list cleaned worker session")[0].registry_state,
        RegistrySessionState::Exited
    );

    daemon.release_for_restart();
    drop(daemon);
    assert!(!process_exists(worker_pid));
    assert!(!process_exists(pty_child_pid));
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_registry_reopened_without_worker_path_is_not_restart_durable() {
    let data_dir = temp_data_dir("local-reopen");
    let session_id = SessionId("local-reopen-session".to_string());

    {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("worker-backed daemon should spawn live session");
        let record = daemon
            .registry()
            .load(&session_id)
            .expect("registry load should succeed")
            .expect("spawn should persist restart evidence");
        assert!(
            record
                .recovery_identity
                .as_ref()
                .and_then(|identity| identity.get("worker_control_socket"))
                .is_some(),
            "worker-backed daemon should persist worker control socket evidence"
        );
        daemon.release_for_restart();
    }

    let mut local = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = local
        .adoption_scan()
        .expect("local daemon should scan worker-created registry");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::InProcessDaemonNotRestartDurable
    );

    let error = local
        .adopt_session(&session_id, 12)
        .expect_err("local daemon should fail loudly before registry adoption");
    assert!(
        matches!(error, CoreDaemonError::MissingWorkerPath),
        "expected MissingWorkerPath, got {error:?}"
    );

    let mut cleanup =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    cleanup
        .adopt_session(&session_id, 13)
        .expect("cleanup daemon should adopt released worker");
    cleanup
        .shutdown(Some(session_id.clone()), 14)
        .expect("cleanup daemon should shut down released worker");

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn in_process_non_restart_durable_adoption_state_has_stable_json_tag() {
    let json = serde_json::to_string(&SessionAdoptionState::InProcessDaemonNotRestartDurable)
        .expect("adoption state should serialize");
    assert_eq!(json, "\"in_process_daemon_not_restart_durable\"");
}

#[test]
fn registry_records_are_durable_enough_for_adoption_scan() {
    let data_dir = temp_data_dir("daemon-adoption");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-adoption-session".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("daemon should spawn");

    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read persisted records");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].record.session_id, session_id);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::MissingProtocolEvidence
    );

    let mut record = daemon
        .registry()
        .load(&session_id)
        .expect("registry record should load")
        .expect("spawn should persist a record");
    record.observe_restart_contract(serde_json::json!({"session": "daemon-adoption"}), 11);
    daemon
        .registry()
        .save(&record)
        .expect("observed restart-contract evidence should persist");

    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read persisted records");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::WorkerDied
        }
    );

    daemon
        .shutdown(Some(SessionId("daemon-adoption-session".to_string())), 20)
        .expect("shutdown should update registry lifecycle");
    let restarted = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let reports = restarted
        .adoption_scan()
        .expect("adoption scan should read shut down records");
    assert_eq!(reports[0].state, SessionAdoptionState::Terminal);

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_dead_worker_with_live_registry_record() {
    let data_dir = temp_data_dir("daemon-dead-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-dead-worker-session".to_string());
    let record = adoptable_record(&session_id, 10);
    daemon
        .registry()
        .save(&record)
        .expect("dead-worker fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify dead worker");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::WorkerDied
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_incompatible_protocol_version() {
    let data_dir = temp_data_dir("daemon-incompatible-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-incompatible-worker-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.protocol_version = botster_core::PROTOCOL_VERSION.saturating_add(1);
    daemon
        .registry()
        .save(&record)
        .expect("incompatible fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify incompatible protocol");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::StaleWorker {
            reason: SessionWorkerStaleReason::IncompatibleProtocol
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_missed_heartbeat() {
    let data_dir = temp_data_dir("daemon-missed-heartbeat");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-missed-heartbeat-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.ping_pong_supported = false;
    daemon
        .registry()
        .save(&record)
        .expect("heartbeat fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify heartbeat failure");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::UnhealthyWorker {
            reason: SessionWorkerHealthReason::MissedHeartbeat
        }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_reports_duplicate_worker_for_session() {
    let data_dir = temp_data_dir("daemon-duplicate-worker");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-duplicate-worker-session".to_string());
    let mut record = adoptable_record(&session_id, 10);
    record.duplicate_worker_candidates = 1;
    daemon
        .registry()
        .save(&record)
        .expect("duplicate fixture should save");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify duplicate candidates");
    assert_eq!(
        reports[0].state,
        SessionAdoptionState::DuplicateWorker { candidates: 2 }
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn adoption_scan_is_read_only_until_explicit_mark_stale() {
    let data_dir = temp_data_dir("daemon-adoption-read-only");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-adoption-read-only-session".to_string());
    let record = adoptable_record(&session_id, 10);
    daemon
        .registry()
        .save(&record)
        .expect("read-only fixture should save");
    let record_path = data_dir
        .join("sessions")
        .join("daemon-adoption-read-only-session.json");
    let before = fs::read(&record_path).expect("record bytes should load before scan");

    let reports = daemon
        .adoption_scan()
        .expect("adoption scan should classify without mutation");
    assert!(matches!(
        reports[0].state,
        SessionAdoptionState::StaleWorker { .. }
    ));
    let after_scan = fs::read(&record_path).expect("record bytes should load after scan");
    assert_eq!(before, after_scan, "adoption_scan must be read-only");

    daemon
        .mark_stale(&session_id, 20)
        .expect("explicit stale mark should persist");
    let marked = daemon
        .registry()
        .load(&session_id)
        .expect("marked record should load")
        .expect("marked record should exist");
    assert_eq!(marked.state, RegistrySessionState::Stale);
    assert_eq!(marked.updated_at, 20);

    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn registry_load_all_skips_malformed_records_without_blocking_good_records() {
    let data_dir = temp_data_dir("daemon-corrupt-registry");
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("daemon-good-record".to_string());
    let mut record = botster_core_daemon::RegistryRecord::running(
        session_id.clone(),
        None,
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        1,
    );
    record.observe_restart_contract(serde_json::json!({"session": "daemon-good-record"}), 2);
    daemon
        .registry()
        .save(&record)
        .expect("good registry record should save");
    fs::write(
        data_dir.join("sessions").join("daemon-bad-record.json"),
        b"not json",
    )
    .expect("malformed registry fixture should be written");

    let listed = daemon.list().expect("bad record should not block listing");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, session_id);

    let _ = fs::remove_dir_all(data_dir);
}

fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
                    .to_string(),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

#[cfg(unix)]
#[test]
fn worker_backed_capture_color_and_snapshot_agrees_with_ghostsnp_import_after_osc_mutations() {
    // Production path: worker-backed CoreDaemon atomic dual-return after OSC
    // palette/special mutations, with GHOSTSNP import equality and stability.
    let data_dir = temp_data_dir("color-snap-atomic");
    let host_color_profile = host_test_color_profile();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_terminal_color_profile(host_color_profile.clone()),
    );
    let session_id = SessionId("color-snap-session".to_string());
    let client_id = ClientId("color-snap-client".to_string());

    // Child applies OSC 4 (palette index 3), OSC 10/11/12 specials, then echoes
    // a marker so drain-before-read has terminal truth to observe.
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = [
        "printf ready; ",
        // palette index 3 -> rgb 0x11/0x22/0x33",
        "printf '\\033]4;3;rgb:1111/2222/3333\\033\\\\'; ",
        // FG 0xaa/0xbb/0xcc, BG 0x01/0x02/0x03, cursor 0xfe/0xfd/0xfc",
        "printf '\\033]10;rgb:aaaa/bbbb/cccc\\033\\\\'; ",
        "printf '\\033]11;rgb:0101/0202/0303\\033\\\\'; ",
        "printf '\\033]12;rgb:fefe/fdfd/fcfc\\033\\\\'; ",
        "printf 'echo:color-mutated\\n'; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = mutate-again ]; then ",
        "printf '\\033]4;3;rgb:4444/5555/6666\\033\\\\'; ",
        "printf '\\033]10;rgb:1212/3434/5656\\033\\\\'; ",
        "printf 'echo:color-mutated-again\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done",
    ]
    .concat();

    daemon.spawn(request, 10).expect("spawn color-snap session");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("color-snap-sub".to_string()),
            11,
        )
        .expect("attach color-snap session");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    // Wait until OSC mutations reach the Ghostty shadow via drain-before-read.
    let first = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-mutated",
        12,
        |profile| {
            profile.colors.get(&3)
                == Some(&Rgb {
                    r: 0x11,
                    g: 0x22,
                    b: 0x33,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0xaa,
                        g: 0xbb,
                        b: 0xcc,
                    })
                && profile.colors.get(&COLOR_INDEX_BACKGROUND)
                    == Some(&Rgb {
                        r: 0x01,
                        g: 0x02,
                        b: 0x03,
                    })
                && profile.colors.get(&COLOR_INDEX_CURSOR)
                    == Some(&Rgb {
                        r: 0xfe,
                        g: 0xfd,
                        b: 0xfc,
                    })
        },
    );

    assert_eq!(
        first.snapshot.request_id,
        RequestId("capture-color-and-snapshot".to_string())
    );
    assert_eq!(first.snapshot.session_id, session_id);
    assert_eq!(first.snapshot.data, first.payload.bytes);
    assert_ghostty_snapshot_authority(&first.payload);
    assert!(first.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_color_profile_authority(&first.color_profile);
    assert_eq!(
        first.color_profile.colors.get(&3),
        Some(&Rgb {
            r: 0x11,
            g: 0x22,
            b: 0x33
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        Some(&Rgb {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        Some(&Rgb {
            r: 0x01,
            g: 0x02,
            b: 0x03
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        Some(&Rgb {
            r: 0xfe,
            g: 0xfd,
            b: 0xfc
        })
    );
    // Host baseline must not rewrite live OSC-owned colors after start.
    assert_ne!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        host_color_profile.colors.get(&COLOR_INDEX_FOREGROUND)
    );

    // GHOSTSNP content proof: import into a fresh Ghostty terminal and fully
    // compare the restored profile with the paired atomic result (all 256
    // palette entries + specials), while keeping targeted mutation diagnostics.
    let restored = ghostty_snapshot_color_profile(&first.payload);
    assert_eq!(
        restored.colors.get(&3),
        first.color_profile.colors.get(&3),
        "imported GHOSTSNP palette index 3 must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_FOREGROUND),
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        "imported GHOSTSNP foreground must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_BACKGROUND),
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        "imported GHOSTSNP background must match atomic pair"
    );
    assert_eq!(
        restored.colors.get(&COLOR_INDEX_CURSOR),
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        "imported GHOSTSNP cursor must match atomic pair"
    );
    assert_eq!(
        restored, first.color_profile,
        "imported GHOSTSNP full color profile must equal the paired atomic profile"
    );

    // Hold the first pair; mutate live; prove held pair is stable and a new
    // capture differs.
    let held_profile = first.color_profile.clone();
    let held_payload = first.payload.clone();
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"mutate-again\n".to_vec(),
            50,
        )
        .expect("second OSC mutation input");
    let second = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-mutated-again",
        51,
        |profile| {
            profile.colors.get(&3)
                == Some(&Rgb {
                    r: 0x44,
                    g: 0x55,
                    b: 0x66,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0x12,
                        g: 0x34,
                        b: 0x56,
                    })
        },
    );
    assert_eq!(
        held_profile, first.color_profile,
        "held atomic color pair must not mutate with live terminal"
    );
    assert_eq!(
        held_payload, first.payload,
        "held atomic snapshot pair must not mutate with live terminal"
    );
    assert_ne!(
        second.color_profile.colors.get(&3),
        held_profile.colors.get(&3),
        "live re-capture must observe post-mutation palette"
    );
    assert_ne!(
        second.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        held_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        "live re-capture must observe post-mutation foreground"
    );
    let second_restored = ghostty_snapshot_color_profile(&second.payload);
    assert_eq!(
        second_restored.colors.get(&3),
        second.color_profile.colors.get(&3)
    );
    assert_eq!(
        second_restored.colors.get(&COLOR_INDEX_FOREGROUND),
        second.color_profile.colors.get(&COLOR_INDEX_FOREGROUND)
    );
    assert_eq!(
        second_restored, second.color_profile,
        "second capture GHOSTSNP full profile must equal its paired atomic profile"
    );

    // Drain-before-read egress retention parity: next explicit drain returns
    // client egress retained from internal drains exactly once for markers.
    let drained = daemon
        .drain(&session_id, 80)
        .expect("drain after atomic captures should succeed");
    let output = terminal_output(&drained.client_egress);
    assert!(
        output.contains("echo:color-mutated") || output.contains("echo:color-mutated-again"),
        "internal atomic capture drain must retain client egress: {output:?}"
    );

    daemon.shutdown(Some(session_id), 90).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn natural_exit_capture_color_and_snapshot_freezes_repeatable_pair() {
    // Direct retained-path proof for the new public dual-return: first
    // reconciling read after natural exit is capture_color_and_snapshot.
    let data_dir = temp_data_dir("color-snap-natural-exit");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("color-snap-exit-session".to_string());
    let client_id = ClientId("color-snap-exit-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = [
        "printf ready; ",
        "printf '\\033]4;5;rgb:5555/6666/7777\\033\\\\'; ",
        "printf '\\033]10;rgb:1010/2020/3030\\033\\\\'; ",
        "printf '\\033]11;rgb:4040/5050/6060\\033\\\\'; ",
        "printf '\\033]12;rgb:7070/8080/9090\\033\\\\'; ",
        "printf 'echo:color-exit-ready\\n'; ",
        "IFS= read -r line; ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "exit 0",
    ]
    .concat();

    daemon
        .spawn(request, 10)
        .expect("natural-exit color session should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("color-snap-exit-sub".to_string()),
            11,
        )
        .expect("natural-exit color session should attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    // Ensure OSC mutations reach the Ghostty shadow while the session is live.
    let _ = capture_color_and_snapshot_until(
        &mut daemon,
        &session_id,
        "echo:color-exit-ready",
        12,
        |profile| {
            profile.colors.get(&5)
                == Some(&Rgb {
                    r: 0x55,
                    g: 0x66,
                    b: 0x77,
                })
                && profile.colors.get(&COLOR_INDEX_FOREGROUND)
                    == Some(&Rgb {
                        r: 0x10,
                        g: 0x20,
                        b: 0x30,
                    })
        },
    );

    // Capture worker identity before exit so we can wait for exact process and
    // control-socket death without sleeping past a race into the live path.
    let (_, pty_child_pid, socket_path) = worker_process_evidence(&daemon, &session_id);
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"color-exit\n".to_vec(),
            20,
        )
        .expect("natural-exit trigger input should write");
    wait_for_condition("worker process and control route completion", || {
        !process_exists(pty_child_pid) && UnixStream::connect(&socket_path).is_err()
    });
    // Process is gone, but the daemon has not yet reconciled lifecycle.
    assert_eq!(
        daemon.list().expect("list unreconciled session")[0].registry_state,
        RegistrySessionState::Running,
        "registry must still report Running before the first reconciling capture"
    );

    // First daemon lifecycle reconciliation after process exit: freeze retained
    // color+snapshot pair through capture_color_and_snapshot (not drain/read_screen).
    // resolve_readback drains the dead worker, freezes retained terminal state, and
    // returns that frozen pair — the live dual-return branch is not used.
    let first = daemon
        .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
            request_id: RequestId("natural-exit-color-1".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        })
        .expect("first natural-exit capture_color_and_snapshot should freeze retained truth");

    // Public lifecycle discriminator: after the first reconciling capture, mutable
    // session paths must reject with SessionNotReadable before any later readback.
    // This proves capture_color_and_snapshot reconciled exit without relying only
    // on a later drain.
    assert!(
        matches!(
            daemon.input(
                client_id,
                session_id.clone(),
                b"should-not-write\n".to_vec(),
                21,
            ),
            Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
        ),
        "first reconciling capture must make subsequent input SessionNotReadable"
    );

    // Second call serves the frozen pair without re-entering a live terminal.
    // This is the retained branch: process/socket are already dead, so a live
    // re-capture could not produce a fresh authoritative Ghostty pair.
    let second = daemon
        .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
            request_id: RequestId("natural-exit-color-2".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("retained capture_color_and_snapshot should be repeatable");

    assert_eq!(
        first.color_profile, second.color_profile,
        "retained freeze must serve the same color profile"
    );
    assert_eq!(
        first.payload, second.payload,
        "retained freeze must serve the same GHOSTSNP payload"
    );
    assert_ne!(first.snapshot.request_id, second.snapshot.request_id);
    // Symmetric retained snapshot path must agree with the dual-return freeze.
    let retained_snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("natural-exit-snapshot-from-retained".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        })
        .expect("capture_snapshot should serve the same retained freeze");
    assert_eq!(
        retained_snapshot.payload, first.payload,
        "retained snapshot projection must match the frozen dual-return pair"
    );
    assert_eq!(
        first.snapshot.request_id,
        RequestId("natural-exit-color-1".to_string())
    );
    assert_eq!(
        second.snapshot.request_id,
        RequestId("natural-exit-color-2".to_string())
    );
    assert_ghostty_snapshot_authority(&first.payload);
    assert!(first.payload.bytes.starts_with(GHOSTSNP_MAGIC));
    assert_color_profile_authority(&first.color_profile);
    assert_eq!(
        first.color_profile.colors.get(&5),
        Some(&Rgb {
            r: 0x55,
            g: 0x66,
            b: 0x77
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        Some(&Rgb {
            r: 0x10,
            g: 0x20,
            b: 0x30
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        Some(&Rgb {
            r: 0x40,
            g: 0x50,
            b: 0x60
        })
    );
    assert_eq!(
        first.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        Some(&Rgb {
            r: 0x70,
            g: 0x80,
            b: 0x90
        })
    );

    let restored = ghostty_snapshot_color_profile(&first.payload);
    assert_eq!(
        restored, first.color_profile,
        "retained GHOSTSNP full profile must equal the frozen atomic pair"
    );

    let exit_drain = daemon
        .drain(&session_id, 23)
        .expect("drain after retained color capture should return final output");
    assert_retained_exit_output(&exit_drain, &session_id, "echo:color-exit");
    let second_drain = daemon
        .drain(&session_id, 24)
        .expect("second drain after retained color capture should succeed");
    assert_no_duplicate_exit_output(&second_drain, "echo:color-exit");

    daemon
        .shutdown(Some(session_id), 25)
        .expect("shutdown after retained color capture");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_kitty_and_mouse_input_reaches_child_pty() {
    let data_dir = temp_data_dir("kitty-mouse-input-pty");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("kitty-mouse-input-session".to_string());
    let client_id = ClientId("kitty-mouse-input-client".to_string());

    // Child hex-encodes exact raw stdin bytes so the production input path can
    // prove Kitty and mouse encodings reach the PTY unchanged.
    let kitty_bytes = b"\x1b[27u".to_vec();
    let mouse_bytes = b"\x1b[<0;10;20M".to_vec();
    let mut expected = kitty_bytes.clone();
    expected.extend_from_slice(&mouse_bytes);
    let expected_len = expected.len();

    let mut request = spawn_request(&session_id);
    // Use dd for an exact-length raw read and od for hex so we prove the
    // production CoreDaemon::input path without depending on Python startup.
    request.request.arguments[1] = format!(
        "stty -echo raw 2>/dev/null; printf ready; dd bs=1 count={len} 2>/dev/null | od -An -tx1 | tr -d ' \n'; printf '\n'",
        len = expected_len
    );

    daemon
        .spawn(request, 10)
        .expect("spawn input-proof session");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("kitty-mouse-sub".to_string()),
            11,
        )
        .expect("attach input-proof session");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    daemon
        .input(client_id.clone(), session_id.clone(), kitty_bytes, 12)
        .expect("kitty input should write through CoreDaemon::input");
    daemon
        .input(client_id.clone(), session_id.clone(), mouse_bytes, 13)
        .expect("mouse input should write through CoreDaemon::input");

    let mut expected_hex = String::new();
    for byte in &expected {
        expected_hex.push_str(&format!("{byte:02x}"));
    }
    let screen = read_screen_until(&mut daemon, &session_id, &expected_hex, 14);
    assert!(
        screen.screen.text.contains(&expected_hex),
        "child PTY must receive exact Kitty+mouse input bytes; text={}",
        screen.screen.text
    );

    daemon
        .shutdown(Some(session_id), 15)
        .expect("shutdown input-proof session");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_flags_include_kitty_and_mouse_from_ghostty_authority() {
    let data_dir = temp_data_dir("mode-flags-full-authority");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("mode-flags-full-session".to_string());
    let client_id = ClientId("mode-flags-full-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-modes ]; then ",
        "printf '\\033[?1000h\\033[?1006h\\033[=1;1u\\033[?2004h\\033[?1004h\\033[?1h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("mode-full-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id,
            session_id.clone(),
            b"enable-modes\n".to_vec(),
            12,
        )
        .expect("enable modes");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-modes", 13);

    let mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-full".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("read production mode flags");
    assert_mode_flags_authority(
        &mode_flags.mode_flags.mode_flags,
        ModeFlags {
            kitty_enabled: true,
            mouse_mode: 9,
            bracketed_paste: true,
            focus_reporting: true,
            application_cursor: true,
            ..ModeFlags::default()
        },
    );

    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("mode-full-snap".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("capture snapshot");
    assert_ghostty_snapshot_authority(&snapshot.payload);
    assert!(snapshot.payload.bytes.starts_with(GHOSTSNP_MAGIC));

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_input_admits_matching_token_and_rejects_stale() {
    let data_dir = temp_data_dir("mode-gated-admit");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("mode-gated-session".to_string());
    let client_id = ClientId("mode-gated-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-modes ]; then ",
        "printf '\\033[?1000h\\033[?1006h\\033[=1;1u'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("mode-gated-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-gated-baseline".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline modes");
    let baseline_token = baseline.mode_flags.mode_freshness;
    assert_ne!(baseline_token.mode_generation, 0);

    let admitted = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"enable-modes\n".to_vec(),
            Some(baseline_token),
            13,
        )
        .expect("matching gated input");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-modes", 14);

    let after_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("mode-gated-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("modes after enable");
    assert!(after_modes.mode_flags.mode_flags.kitty_enabled);
    assert_eq!(after_modes.mode_flags.mode_flags.mouse_mode, 9);

    let stale = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"stale-input\n".to_vec(),
            Some(baseline_token),
            16,
        )
        .expect("stale gated input returns typed result");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "stale token must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    thread::sleep(Duration::from_millis(100));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("mode-gated-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 17,
        })
        .expect("screen after stale reject");
    assert!(
        !screen.screen.text.contains("echo:stale-input"),
        "stale gated input must write zero PTY bytes; screen={}",
        screen.screen.text
    );

    let current = after_modes.mode_flags.mode_freshness;
    let again = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"fresh-input\n".to_vec(),
            Some(current),
            18,
        )
        .expect("fresh gated input");
    match again {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated outcome"),
    }
    let _ = read_screen_until(&mut daemon, &session_id, "echo:fresh-input", 19);

    daemon
        .mode_gated_input(
            ClientId("mode-gated-client".to_string()),
            session_id.clone(),
            b"plain-path\n".to_vec(),
            None,
            20,
        )
        .expect("plain path");
    let _ = read_screen_until(&mut daemon, &session_id, "echo:plain-path", 21);

    daemon.shutdown(Some(session_id), 22).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_race_a_queued_before_probe_rejects() {
    // Race (a): mode-changing output is available before the probe forms a
    // reply but must not be admitted against a token that missed that output.
    // Hold delays the worker barrier drain so a concurrent flip is applied only
    // at admit time under the reader fence.
    let data_dir = temp_data_dir("mode-gated-race-a");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(200)),
    );
    let session_id = SessionId("mode-gated-race-a".to_string());
    let client_id = ClientId("mode-gated-race-a-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("race-a-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("race-a-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    // Emit mode-changing output after probe, then admit under hold so the
    // worker applies that output after the hold but before the write.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-race-a\n".to_vec(),
            Some(token),
            14,
        )
        .expect("race-a gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "race (a) must reject stale token");
            assert!(
                result.mode_freshness != token || result.mode_flags.mouse_mode != 0,
                "worker must surface post-flip mode state"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(150));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("race-a-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-race-a"),
        "race (a) must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_race_b_after_probe_before_admit_rejects() {
    // Race (b): mode change after probe reply, before admission. Do not wait for
    // parent screen echo before admitting — the worker fence must catch it.
    let data_dir = temp_data_dir("mode-gated-race-b");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(150)),
    );
    let session_id = SessionId("mode-gated-race-b".to_string());
    let client_id = ClientId("mode-gated-race-b-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("race-b-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("race-b-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip modes");
    // Immediately admit with pre-flip token while hold keeps the fence open.
    let stale = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-race-b\n".to_vec(),
            Some(token),
            14,
        )
        .expect("race-b gated");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "race (b) must reject stale token");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(150));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("race-b-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-race-b"),
        "race (b) must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_post_final_drain_hold_rejects() {
    // Hold after the initial queue drain under the reader fence, then release
    // mode-changing output into the OS PTY buffer before the final drain/write.
    let data_dir = temp_data_dir("mode-gated-hold");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(200)),
    );
    let session_id = SessionId("mode-gated-hold".to_string());
    let client_id = ClientId("mode-gated-hold-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();

    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("hold-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");

    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("hold-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"held-stale\n".to_vec(),
            Some(token),
            14,
        )
        .expect("held gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "post-final-drain hold must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(200));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("hold-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:held-stale"),
        "held race must write zero stale PTY bytes"
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_timeout_fails_closed_without_late_write() {
    // Parent wait is write-timeout + 1s reply grace. Hold past that so the
    // parent clears the wait slot as a true timeout (not a correlated deadline
    // result). The worker still must not write after its wall-clock deadline.
    let data_dir = temp_data_dir("mode-gated-timeout");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(Duration::from_millis(120))
            .with_test_mode_gated_hold_ms(Some(2_000)),
    );
    let session_id = SessionId(format!(
        "mode-gated-timeout-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let client_id = ClientId("mode-gated-timeout-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("timeout-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("timeout-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let error = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"timeout-bytes\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect_err("short timeout must fail closed");
    let message = error.to_string();
    assert!(
        message.contains("timed out") || message.contains("timeout"),
        "expected timeout failure, got {message}"
    );

    // Wait past the worker hold so a non-fail-closed worker would write.
    thread::sleep(Duration::from_millis(1_500));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("timeout-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("screen after timeout");
    assert!(
        !screen.screen.text.contains("echo:timeout-bytes"),
        "timeout must not allow late PTY write; screen={}",
        screen.screen.text
    );

    daemon.shutdown(Some(session_id), 15).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_interleaved_output_during_wait() {
    let data_dir = temp_data_dir("mode-gated-interleave");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_mode_gated_hold_ms(Some(250)),
    );
    let session_id = SessionId("mode-gated-interleave".to_string());
    let client_id = ClientId("mode-gated-interleave-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("interleave-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("interleave-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    // Fire plain input that produces output while a gated wait is held.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"interleaved\n".to_vec(),
            13,
        )
        .expect("interleaved input");
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"after-interleave\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            14,
        )
        .expect("gated during interleave");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => assert!(result.admitted),
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    let screen = read_screen_until(&mut daemon, &session_id, "echo:after-interleave", 15);
    assert!(
        screen.screen.text.contains("echo:interleaved"),
        "interleaved output must still demux to parent; screen={}",
        screen.screen.text
    );

    daemon.shutdown(Some(session_id), 16).expect("shutdown");
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_unpublished_reader_chunk_window_rejects() {
    // Hold after the background reader captures mode bytes and before fence
    // enqueue onto the single fence-owned queue. The barrier must wait for
    // publication, apply the modes, and reject the stale token — not write while
    // the chunk is unpublished.
    let data_dir = temp_data_dir("mode-gated-unpub-chunk");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_test_hold_after_read_ms(Some(250)),
    );
    let session_id = SessionId("mode-gated-unpub".to_string());
    let client_id = ClientId("mode-gated-unpub-client".to_string());

    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = flip ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("unpub-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("unpub-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");
    let token = probe.mode_flags.mode_freshness;

    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flip\n".to_vec(),
            13,
        )
        .expect("flip");
    // Give the reader time to capture mode output into the after-read hold.
    thread::sleep(Duration::from_millis(50));
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-unpub\n".to_vec(),
            Some(token),
            14,
        )
        .expect("gated");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "unpublished chunk window must reject");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    thread::sleep(Duration::from_millis(300));
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("unpub-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:stale-unpub"),
        "must write zero stale bytes; screen={}",
        screen.screen.text
    );
    daemon.shutdown(Some(session_id), 16).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_write_deadline_bounds_complete_write() {
    // Force write WouldBlock past the worker write deadline so the worker must
    // return a correlated deadline result without delivering the payload. The
    // parent wait includes reply grace so this test must observe that result
    // (parent timeout is not accepted). After the block lifts, re-check screen
    // so a non-enforcing worker cannot pass by writing late.
    let data_dir = temp_data_dir("mode-gated-write-deadline");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    // Block long enough for spawn/probe/scheduling plus the 1s write deadline.
    // Keep the window short enough that waiting past it is practical in CI.
    let write_timeout = Duration::from_secs(1);
    let block_until = now_ms + 8_000;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(write_timeout)
            .with_test_write_block_until_unix_ms(Some(block_until)),
    );
    let session_id = SessionId(format!("mode-gated-wdeadline-{now_ms}"));
    let client_id = ClientId("mode-gated-wdeadline-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("wdeadline-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("wdeadline-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"deadline-bytes\n".to_vec(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect("must receive correlated worker gated result (not parent timeout)");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "deadline must not admit");
            assert_eq!(result.bytes_written, 0, "deadline must write zero bytes");
            let kind = result.error_kind.unwrap_or_default();
            assert!(
                kind.contains("deadline"),
                "expected deadline error_kind from worker, got {kind}"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated fail-closed outcome"),
    }

    // Wait until the forced WouldBlock window has ended so a worker that
    // ignored the deadline would complete the write and echo the payload.
    let after_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    if after_ms < block_until + 250 {
        thread::sleep(Duration::from_millis(block_until + 250 - after_ms));
    }
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("wdeadline-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("screen");
    assert!(
        !screen.screen.text.contains("echo:deadline-bytes"),
        "deadline-bounded write must deliver zero payload bytes even after block lifts; screen={}",
        screen.screen.text
    );
    daemon.shutdown(Some(session_id), 15).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_fence_queue_preserves_mode_order() {
    // Capacity-1 fence queue + hold-after-read: opposite mode transitions are
    // enqueued in order on the single ownership queue. Barrier/probe drain must
    // keep FIFO enable then disable → mouse off. Reversed order leaves mouse on.
    let data_dir = temp_data_dir("mode-gated-fence-order");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_mode_gated_input_timeout(Duration::from_secs(5))
            .with_test_hold_after_read_ms(Some(60)),
    );
    let session_id = SessionId("mode-gated-fence-order".to_string());
    let client_id = ClientId("mode-gated-fence-order-client".to_string());
    let mut request = spawn_request(&session_id);
    // Separate writes + sleep so the single fence queue captures enable then
    // disable as distinct chunks under hold-after-read.
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = seq ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "sleep 0.05; ",
        "printf '\\033[?1000l\\033[?1006l'; ",
        "printf 'seq-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("fence-order-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-base".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline");
    let baseline_token = baseline.mode_flags.mode_freshness;
    assert_eq!(baseline.mode_flags.mode_flags.mouse_mode, 0);

    daemon
        .input(client_id.clone(), session_id.clone(), b"seq\n".to_vec(), 13)
        .expect("seq");
    // Wait for enable hold (~60ms) + disable capture on the single fence queue,
    // then drain via probe (barrier path).
    thread::sleep(Duration::from_millis(120));
    let start = Instant::now();
    let after_seq = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("probe must drain fence queue without hang");
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "mode probe must not hang when the fence queue holds mode bytes"
    );
    assert_eq!(
        after_seq.mode_flags.mode_flags.mouse_mode, 0,
        "FIFO enable-then-disable must leave mouse off; reversed drain leaves mouse on"
    );

    // Wait for shell marker so both transitions are definitely applied, then
    // re-probe for a stable token used in admit/reject assertions.
    let _ = drain_until(&mut daemon, &session_id, "seq-done");
    let final_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("fence-order-final".to_string()),
            session_id: session_id.clone(),
            now_seconds: 15,
        })
        .expect("final modes");
    assert_eq!(
        final_modes.mode_flags.mode_flags.mouse_mode, 0,
        "final mouse mode must remain off after ordered sequence"
    );

    let stale = daemon
        .mode_gated_input(
            client_id.clone(),
            session_id.clone(),
            b"order-stale\n".to_vec(),
            Some(baseline_token),
            16,
        )
        .expect("stale gated");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            // If the sequence advanced modes, baseline is stale. If timing
            // applied both transitions with a net-zero mode delta before any
            // revision bump observation, admit is still safe only with final
            // mouse-off (asserted above); require reject when revision moved.
            if final_modes.mode_flags.mode_freshness != baseline_token {
                assert!(!result.admitted, "stale baseline token must reject");
            }
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    let admitted = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"fresh-after-order\n".to_vec(),
            Some(final_modes.mode_flags.mode_freshness),
            17,
        )
        .expect("current token");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(
                result.admitted,
                "current token after ordered apply must admit"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 17).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_ordinary_pressure_stays_lossless() {
    // Tiny public capacity + tiny fence pending + flood: ordinary pressure must
    // wait/backpressure, not latch sticky mode-authority failure. After the
    // flood drains, probes and current-token gated admit still succeed.
    // True-loss sticky fail-closed is covered by internal unit seams
    // (set_overflow_error / Failed event) without a public production flag.
    let data_dir = temp_data_dir("mode-gated-pressure-lossless");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_test_pending_capacity(Some(1))
            .with_mode_gated_input_timeout(Duration::from_secs(5)),
    );
    let session_id = SessionId("mode-gated-pressure-lossless".to_string());
    let client_id = ClientId("mode-gated-pressure-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = flood ]; then ",
        "i=0; while [ $i -lt 40 ]; do ",
        "printf 'flood-%s:%04000d\\n' \"$i\" 0; ",
        "i=$((i+1)); ",
        "done; ",
        "printf 'flood-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("pressure-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"flood\n".to_vec(),
            13,
        )
        .expect("flood");
    let _ = drain_until(&mut daemon, &session_id, "flood-done");
    let after = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("pressure-after".to_string()),
            session_id: session_id.clone(),
            now_seconds: 14,
        })
        .expect("probe must succeed after ordinary pressure (not sticky authority fail)");
    let admitted = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"after-pressure\n".to_vec(),
            Some(after.mode_flags.mode_freshness),
            15,
        )
        .expect("current token after pressure");
    match admitted {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(
                result.admitted,
                "ordinary pressure must not latch sticky authority failure"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 16).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_normal_drain_preserves_mode_fifo() {
    // hold_after_enqueue keeps the reader critical while opposite mode chunks
    // sit on the single fence queue. Normal (non-barrier) drain via the worker
    // control loop + parent drain, then a later probe, must keep enable→disable
    // FIFO (mouse off) after both mode transitions apply. Complements the
    // dual-buffer source guard in local_process unit tests (that guard fails on
    // df38c218; this path proves production normal-drain + Ghostty apply order).
    //
    // Deterministic producer/worker-application boundary: the child emits
    // enable + an "enabled" marker, then blocks until the parent releases
    // disable. The test observes worker-applied mouse-on (and a revision bump)
    // before releasing disable. Without that boundary, enable+disable can
    // coalesce into one PTY read; the worker samples ModeFlags once per chunk
    // and net-zero final modes leave mode_revision unchanged (production
    // contract), which made the old sleep-separated form flake.
    let data_dir = temp_data_dir("mode-gated-normal-drain-fifo");
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_pty_reader_chunk_capacity(Some(1))
            .with_test_hold_after_enqueue_ms(Some(80))
            .with_mode_gated_input_timeout(Duration::from_secs(5)),
    );
    let session_id = SessionId("mode-gated-normal-drain".to_string());
    let client_id = ClientId("mode-gated-normal-drain-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] = concat!(
        "printf ready; ",
        "while IFS= read -r line; do ",
        "if [ \"$line\" = seq ]; then ",
        "printf '\\033[?1000h\\033[?1006h'; ",
        "printf 'enabled\\n'; ",
        "IFS= read -r _release; ",
        "printf '\\033[?1000l\\033[?1006l'; ",
        "printf 'seq-done\\n'; ",
        "else printf \"echo:%s\\n\" \"$line\"; ",
        "fi; ",
        "done"
    )
    .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("normal-drain-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let baseline = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("normal-drain-base".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("baseline");
    assert_eq!(
        baseline.mode_flags.mode_flags.mouse_mode, 0,
        "baseline mouse must be off before the sequence"
    );
    daemon
        .input(client_id.clone(), session_id.clone(), b"seq\n".to_vec(), 13)
        .expect("seq");
    // Normal-path drains while enqueue holds may still be active (no fixed sleep).
    let start = Instant::now();
    let _ = daemon
        .drain(&session_id, 14)
        .expect("normal drain during hold");
    let _ = drain_until(&mut daemon, &session_id, "enabled");

    // Require worker application of enable before the child may emit disable.
    // Polling is condition-driven; mode sampling (not wall-clock) is the barrier.
    let mut enable_modes = None;
    for tick in 0..200u64 {
        let probe = daemon
            .read_mode_flags(ReadModeFlagsRequest {
                request_id: RequestId(format!("normal-drain-enable-{tick}")),
                session_id: session_id.clone(),
                now_seconds: 15 + tick,
            })
            .expect("enable probe");
        if probe.mode_flags.mode_flags.mouse_mode != 0 {
            enable_modes = Some(probe);
            break;
        }
        let _ = daemon
            .drain(&session_id, 200 + tick)
            .expect("normal drain while waiting for enable apply");
        thread::sleep(Duration::from_millis(10));
    }
    let enable_modes = enable_modes
        .expect("worker must apply enable (mouse on) before release; missing application boundary");
    assert!(
        enable_modes.mode_flags.mode_freshness.mode_revision
            >= baseline
                .mode_flags
                .mode_freshness
                .mode_revision
                .saturating_add(1),
        "enable application must advance revision; baseline={:?} enable={:?}",
        baseline.mode_flags.mode_freshness,
        enable_modes.mode_flags.mode_freshness
    );

    // Release disable only after enable was observed as its own mode sample.
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"release\n".to_vec(),
            16,
        )
        .expect("release");
    let _ = daemon
        .drain(&session_id, 17)
        .expect("normal drain during second transition");
    let _ = drain_until(&mut daemon, &session_id, "seq-done");
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "normal drain path must not hang under enqueue hold"
    );
    let final_modes = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("normal-drain-final".to_string()),
            session_id: session_id.clone(),
            now_seconds: 18,
        })
        .expect("final");
    assert_eq!(
        final_modes.mode_flags.mode_flags.mouse_mode, 0,
        "normal-drain FIFO must keep enable→disable (mouse off)"
    );
    // Both transitions applied as distinct observations: enable then disable
    // advances revision by ≥2. Mouse-off alone is insufficient if only disable
    // (or neither) applied. Valid because enable was observed before release.
    assert!(
        final_modes.mode_flags.mode_freshness.mode_revision
            >= baseline
                .mode_flags
                .mode_freshness
                .mode_revision
                .saturating_add(2),
        "expected ≥2 mode revisions (enable then disable); baseline={:?} enable={:?} final={:?}",
        baseline.mode_flags.mode_freshness,
        enable_modes.mode_flags.mode_freshness,
        final_modes.mode_flags.mode_freshness
    );
    assert_ne!(
        final_modes.mode_flags.mode_freshness, baseline.mode_flags.mode_freshness,
        "mode sequence must change freshness so stale admit is meaningful"
    );
    let stale = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            b"stale-normal-drain\n".to_vec(),
            Some(baseline.mode_flags.mode_freshness),
            19,
        )
        .expect("stale");
    match stale {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "stale must reject after mode sequence");
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated"),
    }
    daemon.shutdown(Some(session_id), 20).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_backed_mode_gated_partial_write_reports_bytes_written() {
    // First write chunk succeeds (1 byte), then force WouldBlock past the
    // deadline. Public outcome must be admitted=false with nonzero
    // bytes_written and partial_write error_kind — never a clean reject.
    let data_dir = temp_data_dir("mode-gated-partial-write");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    let block_until = now_ms + 30_000;
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_mode_gated_input_timeout(Duration::from_millis(800))
            .with_test_write_max_chunk(Some(1))
            .with_test_write_block_until_unix_ms(Some(block_until)),
    );
    let session_id = SessionId(format!("mode-gated-partial-{now_ms}"));
    let client_id = ClientId("mode-gated-partial-client".to_string());
    let mut request = spawn_request(&session_id);
    request.request.arguments[1] =
        "printf ready; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done"
            .to_string();
    daemon.spawn(request, 10).expect("spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            SubscriptionId("partial-sub".to_string()),
            11,
        )
        .expect("attach");
    let _ = drain_until(&mut daemon, &session_id, "ready");
    let probe = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("partial-probe".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("probe");

    let payload = b"partial-payload\n".to_vec();
    let outcome = daemon
        .mode_gated_input(
            client_id,
            session_id.clone(),
            payload.clone(),
            Some(probe.mode_flags.mode_freshness),
            13,
        )
        .expect("partial path returns outcome");
    match outcome {
        ModeGatedInputOutcome::Gated(result) => {
            assert!(!result.admitted, "partial must not report full admit");
            assert!(
                result.bytes_written > 0,
                "partial must report nonzero bytes_written, got {}",
                result.bytes_written
            );
            assert!(
                result.bytes_written < payload.len(),
                "partial bytes_written must be less than payload; written={} len={}",
                result.bytes_written,
                payload.len()
            );
            let kind = result.error_kind.unwrap_or_default();
            assert!(
                kind.contains("partial_write") || kind.contains("deadline"),
                "expected partial_write/deadline error_kind, got {kind}"
            );
        }
        ModeGatedInputOutcome::PlainWritten => panic!("expected gated partial outcome"),
    }
    daemon.shutdown(Some(session_id), 14).ok();
    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn worker_binary_is_hosted_by_daemon_package_not_core() {
    // Packaging proof: session worker builds from botster-core-daemon and
    // botster-core does not depend on botster-terminal-ghostty.
    let daemon_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let core_toml = fs::read_to_string(daemon_manifest.join("../botster-core/Cargo.toml"))
        .expect("read core cargo");
    assert!(
        !core_toml.contains("botster-terminal-ghostty"),
        "botster-core must remain Ghostty-free"
    );
    assert!(
        !core_toml.contains("name = \"botster-session-worker\""),
        "session-worker binary must not remain in botster-core"
    );
    let daemon_toml =
        fs::read_to_string(daemon_manifest.join("Cargo.toml")).expect("read daemon cargo");
    assert!(
        daemon_toml.contains("name = \"botster-session-worker\""),
        "daemon package must host botster-session-worker"
    );
    assert!(
        daemon_toml.contains("botster-terminal-ghostty"),
        "daemon package hosts Ghostty for the worker binary"
    );
    let path = worker_path();
    assert!(
        path.exists(),
        "daemon-hosted worker binary must resolve at {}",
        path.display()
    );
}

#[cfg(unix)]
#[test]
fn worker_backed_osc_color_queries_receive_session_side_write_pty_replies() {
    let data_dir = temp_data_dir("osc-color-write-pty");
    // Host outside Core supplies presentation policy through the policy-free
    // CoreDaemonConfig seam. CoreDaemon itself invents no color defaults.
    let host_color_profile = host_test_color_profile();
    let mut daemon = CoreDaemon::new(
        CoreDaemonConfig::new(&data_dir)
            .with_worker_path(worker_path())
            .with_terminal_color_profile(host_color_profile),
    );
    let session_id = SessionId("osc-color-session".to_string());

    // No client attaches. Child emits OSC 10/11/12 queries; session Ghostty must
    // answer via write_pty using the host-supplied color profile. Replies are
    // injected into the child PTY and appear on the production screen path
    // (line-discipline echo of PTY input), proving pre-attach session-side
    // ownership of OSC 10/11/12 for a supplied profile.
    let script_path = data_dir.join("osc_query_child.py");
    fs::create_dir_all(&data_dir).expect("create data dir for script");
    fs::write(
        &script_path,
        r#"#!/usr/bin/env python3
import sys
import time

sys.stdout.write("ready\n")
sys.stdout.flush()
sys.stdout.buffer.write(b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\")
sys.stdout.flush()
# Stay alive long enough for the parent to drain PTY output, run write_pty,
# and inject replies into this process's PTY input.
time.sleep(2)
sys.stdout.write("done\n")
sys.stdout.flush()
"#,
    )
    .expect("write osc child script");

    let mut request = spawn_request(&session_id);
    request.request.executable = "python3".to_string();
    request.request.arguments = vec![script_path.to_string_lossy().into_owned()];

    daemon
        .spawn(request, 10)
        .expect("spawn color query session");
    // Wait until all three host-supplied OSC replies are visible on the session
    // path without any client attach.
    let screen = read_screen_until(&mut daemon, &session_id, "]12;", 11);
    let text = screen.screen.text;
    // Host test profile:
    // FG 0xdd/0xdd/0xdd, BG 0x1e/0x1e/0x2e, cursor 0xf5/0xe0/0xdc
    let seq10 = assert_osc_color_reply_sequence(&text, 10, 0xdd, 0xdd, 0xdd);
    let seq11 = assert_osc_color_reply_sequence(&text, 11, 0x1e, 0x1e, 0x2e);
    let seq12 = assert_osc_color_reply_sequence(&text, 12, 0xf5, 0xe0, 0xdc);
    assert!(
        seq10 < seq11 && seq11 < seq12,
        "expected OSC 10,11,12 bound reply sequences in order; text={text}"
    );

    daemon.shutdown(Some(session_id), 12).ok();
    let _ = fs::remove_dir_all(data_dir);
}

fn mode_flags_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = concat!(
        "printf ready; while IFS= read -r line; do ",
        "printf \"echo:%s\\n\" \"$line\"; ",
        "if [ \"$line\" = enable-mouse ]; then printf '\\033[?1000h\\033[?1006h'; fi; ",
        "done"
    )
    .to_string();
    request
}

fn retained_history_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    let mut request = spawn_request(session_id);
    request.request.arguments[1] = "printf 'echo:dwgrh-prior-marker\nready'; while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string();
    request
}

fn self_exit_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "printf ready; IFS= read -r line; printf \"echo:%s\\n\" \"$line\"; exit 0"
                    .to_string(),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

#[cfg(unix)]
fn controlled_exit_spawn_request(
    session_id: &SessionId,
    marker_path: &std::path::Path,
    release_path: &std::path::Path,
) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                concat!(
                    "printf ready; IFS= read -r line; ",
                    "printf 'botster-core-natural-exit-marker:%s\\n' \"$line\"; ",
                    "printf observed > \"$1\"; ",
                    "while [ ! -e \"$2\" ]; do sleep 0.01; done; ",
                    "printf 'botster-core-natural-exit-tail\\n'; exit 0"
                )
                .to_string(),
                "botster-controlled-exit".to_string(),
                marker_path.to_string_lossy().into_owned(),
                release_path.to_string_lossy().into_owned(),
            ],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

fn notification_session_target(id: &str) -> NotificationTarget {
    NotificationTarget::Session(SessionId(id.to_string()))
}

fn notification_client_target(id: &str) -> NotificationTarget {
    NotificationTarget::Client(ClientId(id.to_string()))
}

fn notification(id: &str, target: NotificationTarget, created_at: u64) -> NotificationItem {
    NotificationItem::message(
        NotificationId(id.to_string()),
        target,
        NotificationSeverity::Info,
        NotificationSource {
            label: "daemon-test".to_string(),
            plugin_key: None,
        },
        NotificationContent {
            title: format!("Notification {id}"),
            body: Some("Synthetic daemon test notification.".to_string()),
            extension: None,
        },
        NotificationTimestamp(created_at),
    )
}

fn envelope_endpoint(id: &str) -> EnvelopeTarget {
    EnvelopeTarget::Endpoint {
        endpoint_id: EndpointId(id.to_string()),
    }
}

fn envelope(id: &str, targets: Vec<EnvelopeTarget>) -> RoutedEnvelope {
    RoutedEnvelope::new(
        EnvelopeId(id.to_string()),
        EndpointId("daemon-test-source".to_string()),
        targets,
        RoutedEnvelopePayload {
            content_type: "application/octet-stream".to_string(),
            body: format!("payload:{id}").into_bytes(),
            extension: None,
        },
        10,
    )
}

fn adoptable_record(
    session_id: &SessionId,
    now_seconds: u64,
) -> botster_core_daemon::RegistryRecord {
    let mut record = botster_core_daemon::RegistryRecord::running(
        session_id.clone(),
        Some(botster_core::ProcessIdentity {
            pid: Some(42),
            runtime_id: Some(format!("{}-runtime", session_id.0)),
        }),
        ResizePayload { rows: 24, cols: 80 },
        "sh".to_string(),
        now_seconds,
    );
    record.observe_restart_contract(
        serde_json::json!({"session": session_id.0}),
        now_seconds + 1,
    );
    record
}

fn serialized_lifecycle_baseline(baseline: &SessionLifecycleBaseline) -> SessionLifecycleBaseline {
    serde_json::from_slice(
        &serde_json::to_vec(baseline).expect("serialize lifecycle baseline for consumer"),
    )
    .expect("deserialize lifecycle baseline for consumer")
}

fn serialized_lifecycle_changes(changes: &SessionLifecycleChanges) -> SessionLifecycleChanges {
    serde_json::from_slice(
        &serde_json::to_vec(changes).expect("serialize lifecycle changes for consumer"),
    )
    .expect("deserialize lifecycle changes for consumer")
}

fn replace_lifecycle_projection(
    projection: &mut BTreeMap<String, SessionLifecycleRecord>,
    baseline: &SessionLifecycleBaseline,
) {
    projection.clear();
    projection.extend(
        baseline
            .sessions
            .iter()
            .cloned()
            .map(|record| (record.session.session_id.0.clone(), record)),
    );
}

fn apply_lifecycle_changes(
    projection: &mut BTreeMap<String, SessionLifecycleRecord>,
    changes: &SessionLifecycleChanges,
) {
    for change in &changes.changes {
        match &change.kind {
            SessionLifecycleChangeKind::Upsert { record } => {
                projection.insert(record.session.session_id.0.clone(), record.clone());
            }
            SessionLifecycleChangeKind::Removed { session_id } => {
                projection.remove(&session_id.0);
            }
            _ => {}
        }
    }
}

fn drain_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..100 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if terminal_output(&aggregate.client_egress).contains(expected) {
            return aggregate;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    aggregate
}

fn wait_for_file(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("child readiness file did not appear: {}", path.display());
}

fn drain_until_attached(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..10_000 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon attach drain should succeed");
        let attached = drained.client_egress.iter().any(|(target, frame)| {
            target == client_id
                && matches!(
                    frame,
                    TransportEgress::AttachState {
                        state: TerminalAttachState::Attached,
                        ..
                    }
                )
        });
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if attached {
            return aggregate;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker attach did not reach Attached");
}

fn drain_until_snapshot(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
) -> botster_core_daemon::DrainResult {
    let mut aggregate = botster_core_daemon::DrainResult::default();
    for tick in 0..10_000 {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon snapshot drain should succeed");
        let snapshot = drained.client_egress.iter().any(|(target, frame)| {
            target == client_id && matches!(frame, TransportEgress::Snapshot { .. })
        });
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        if snapshot {
            return aggregate;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker attach did not emit READY");
}

fn viewport_contains_marker(
    projection: &botster_terminal_ghostty::ViewportProjection,
    marker: &str,
) -> bool {
    let mut row = String::new();
    for (index, cell) in projection.cells.iter().enumerate() {
        if index > 0 && index % projection.cols as usize == 0 {
            if row.contains(marker) {
                return true;
            }
            row.clear();
        }
        row.push_str(&cell.grapheme);
    }
    row.contains(marker)
}

fn drain_until_for_client(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
    expected: &str,
) -> botster_core_daemon::DrainResult {
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_output_length = 0;
    let mut tick = 0;
    let mut aggregate = botster_core_daemon::DrainResult::default();
    loop {
        let drained = daemon
            .drain(session_id, 20 + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = renderable_output_for_client(&aggregate.client_egress, client_id);
        if output.contains(expected) {
            return aggregate;
        }

        let now = Instant::now();
        if output.len() != last_output_length {
            last_progress = now;
            last_output_length = output.len();
        }
        assert!(
            now.duration_since(last_progress) < REAL_WORKER_IDLE_TIMEOUT
                && now.duration_since(started) < REAL_WORKER_COMPLETION_TIMEOUT,
            "queued client output never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last output: {output:?}"
        );
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn read_screen_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> botster_core_daemon::ReadScreenResult {
    let request_id = RequestId("read-screen-marker".to_string());
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_text = String::new();
    let mut tick = 0;
    let last = loop {
        let read = daemon
            .read_screen(ReadScreenRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon read_screen should succeed");
        if read.screen.text.contains(expected) {
            return read;
        }
        let now = Instant::now();
        if read.screen.text != last_text {
            last_progress = now;
        }
        if now.duration_since(last_progress) >= REAL_WORKER_IDLE_TIMEOUT
            || now.duration_since(started) >= REAL_WORKER_COMPLETION_TIMEOUT
        {
            break read;
        }
        last_text.clone_from(&read.screen.text);
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    };
    panic!(
        "read_screen never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last text: {:?}",
        last.screen.text
    )
}

#[cfg(unix)]
fn drain_until_terminal_marker(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) {
    let started = Instant::now();
    let mut last_progress = started;
    let mut last_output_length = 0;
    let mut tick = 0;
    let mut aggregate = botster_core_daemon::DrainResult::default();
    loop {
        let drained = daemon
            .drain(session_id, start_tick + tick)
            .expect("daemon drain should succeed");
        aggregate.client_egress.extend(drained.client_egress);
        aggregate.observations.extend(drained.observations);
        aggregate.backpressure.extend(drained.backpressure);
        let output = terminal_output(&aggregate.client_egress);
        if output.contains(expected) {
            return;
        }

        let now = Instant::now();
        if output.len() != last_output_length {
            last_progress = now;
            last_output_length = output.len();
        }
        assert!(
            now.duration_since(last_progress) < REAL_WORKER_IDLE_TIMEOUT
                && now.duration_since(started) < REAL_WORKER_COMPLETION_TIMEOUT,
            "terminal output never observed {expected:?} within {REAL_WORKER_COMPLETION_TIMEOUT:?} or after {REAL_WORKER_IDLE_TIMEOUT:?} idle; last output: {output:?}"
        );
        tick += 1;
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn capture_color_and_snapshot_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected_marker: &str,
    start_tick: u64,
    profile_ready: impl Fn(&TerminalColorProfile) -> bool,
) -> botster_core_daemon::CaptureColorAndSnapshotResult {
    let request_id = RequestId("capture-color-and-snapshot".to_string());
    let mut last = None;
    for tick in 0..150 {
        let captured = daemon
            .capture_color_and_snapshot(CaptureColorAndSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_color_and_snapshot should succeed");
        let marker_ready = ghostty_snapshot_replays_marker(&captured.payload, expected_marker);
        if marker_ready && profile_ready(&captured.color_profile) {
            return captured;
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let last = last.expect("at least one atomic capture should complete");
    panic!(
        "capture_color_and_snapshot never observed marker {expected_marker:?} with expected colors; \
         palette[3]={:?} fg={:?} bg={:?} cursor={:?}; format={:?}",
        last.color_profile.colors.get(&3),
        last.color_profile.colors.get(&COLOR_INDEX_FOREGROUND),
        last.color_profile.colors.get(&COLOR_INDEX_BACKGROUND),
        last.color_profile.colors.get(&COLOR_INDEX_CURSOR),
        last.payload.format,
    );
}

fn ghostty_snapshot_color_profile(
    payload: &botster_core::TerminalSnapshotPayload,
) -> TerminalColorProfile {
    assert_snapshot_format(payload);
    let mut terminal = GhosttyTerminal::with_config(
        payload.size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty import terminal");
    terminal
        .import_snapshot(payload)
        .expect("test should import daemon Ghostty snapshot");
    terminal
        .read_color_profile()
        .expect("imported GHOSTSNP should expose a color profile")
}

fn capture_snapshot_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> botster_core_daemon::CaptureSnapshotResult {
    let request_id = RequestId("capture-snapshot-marker".to_string());
    let mut last = None;
    for tick in 0..100 {
        let captured = daemon
            .capture_snapshot(CaptureSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_snapshot should succeed");
        if ghostty_snapshot_replays_marker(&captured.payload, expected) {
            return captured;
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "capture_snapshot never observed {expected:?}; last bytes: {:?}",
        last.map(|captured| String::from_utf8_lossy(&captured.payload.bytes).to_string())
    )
}

fn capture_snapshot_and_retained_output_until(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    expected: &str,
    start_tick: u64,
) -> (botster_core_daemon::CaptureSnapshotResult, String) {
    let request_id = RequestId("capture-snapshot-marker".to_string());
    let mut aggregate_output = String::new();
    let mut last = None;
    for tick in 0..100 {
        let captured = daemon
            .capture_snapshot(CaptureSnapshotRequest {
                request_id: request_id.clone(),
                session_id: session_id.clone(),
                now_seconds: start_tick + tick,
            })
            .expect("daemon capture_snapshot should succeed");
        let drained = daemon
            .drain(session_id, start_tick + tick + 100)
            .expect("drain after capture_snapshot should succeed");
        aggregate_output.push_str(&terminal_output(&drained.client_egress));
        if aggregate_output.contains(expected)
            && ghostty_snapshot_replays_marker(&captured.payload, expected)
        {
            return (captured, aggregate_output);
        }
        last = Some(captured);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "capture_snapshot never retained {expected:?}; last format: {:?}; output: {:?}",
        last.and_then(|captured| captured.payload.format),
        aggregate_output
    )
}

/// Host-owned test color profile used only by daemon integration proofs.
///
/// Presentation policy is intentionally not hardcoded inside CoreDaemon.
fn host_test_color_profile() -> TerminalColorProfile {
    let mut colors = HashMap::new();
    colors.insert(
        COLOR_INDEX_FOREGROUND,
        Rgb {
            r: 0xdd,
            g: 0xdd,
            b: 0xdd,
        },
    );
    colors.insert(
        COLOR_INDEX_BACKGROUND,
        Rgb {
            r: 0x1e,
            g: 0x1e,
            b: 0x2e,
        },
    );
    colors.insert(
        COLOR_INDEX_CURSOR,
        Rgb {
            r: 0xf5,
            g: 0xe0,
            b: 0xdc,
        },
    );
    TerminalColorProfile { colors }
}

/// Assert one complete OSC identifier+value sequence and return its index.
///
/// Ghostty encodes OSC color replies as `]Ps;rgb:RRRR/GGGG/BBBB` with 16-bit
/// channels (high byte == low byte for 8-bit source colors). The identifier and
/// RGB value must appear as one bound sequence so mismatched OSC/RGB pairs fail.
fn assert_osc_color_reply_sequence(reply_text: &str, osc: u8, r: u8, g: u8, b: u8) -> usize {
    let rr = format!("{r:02x}{r:02x}");
    let gg = format!("{g:02x}{g:02x}");
    let bb = format!("{b:02x}{b:02x}");
    let expected = format!("]{osc};rgb:{rr}/{gg}/{bb}");
    let lowered = reply_text.to_ascii_lowercase();
    lowered
        .find(&expected)
        .unwrap_or_else(|| panic!("expected bound OSC sequence {expected} in reply {reply_text}"))
}

fn assert_snapshot_format(payload: &botster_core::TerminalSnapshotPayload) {
    assert_eq!(
        payload.format.as_deref(),
        Some(EXPECTED_SNAPSHOT_FORMAT),
        "snapshot format should match the active daemon terminal backend"
    );
}

fn assert_snapshot_payload_observed_marker_when_plain(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) {
    // Plain fallback snapshots are no longer a production path. Keep the helper
    // as a no-op so historical call sites stay readable next to Ghostty asserts.
    let _ = (payload, expected);
}

fn assert_ghostty_snapshot_replays_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    assert!(
        plain_text.contains(expected),
        "Ghostty snapshot replay should contain {expected:?}; replayed text length: {}",
        plain_text.len()
    );
    assert!(
        payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        payload.bytes.len()
    );
}

fn assert_ghostty_snapshot_replays_minimum_markers(
    payload: &botster_core::TerminalSnapshotPayload,
    marker_count: usize,
    minimum: usize,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    let retained_markers = retained_ghostty_scrollback_markers(&plain_text, marker_count);
    assert!(
        retained_markers.len() >= minimum,
        "Ghostty snapshot replay should retain at least {minimum} generated markers; retained markers: {}; replayed text length: {}",
        retained_markers.len(),
        plain_text.len()
    );
    assert!(
        payload.bytes.len() < EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING,
        "Ghostty snapshot payload should remain under the reviewed ceiling: {} bytes",
        payload.bytes.len()
    );
}

fn assert_ghostty_snapshot_does_not_replay_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    unexpected: &str,
) {
    let plain_text = ghostty_snapshot_plain_text(payload);
    assert!(
        !plain_text.contains(unexpected),
        "Ghostty snapshot replay should not contain {unexpected:?}; replayed text length: {}",
        plain_text.len()
    );
}

fn ghostty_snapshot_replays_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) -> bool {
    ghostty_snapshot_plain_text(payload).contains(expected)
}

fn ghostty_snapshot_plain_text(payload: &botster_core::TerminalSnapshotPayload) -> String {
    assert_snapshot_format(payload);
    let mut terminal = GhosttyTerminal::with_config(
        payload.size,
        GhosttyAdapterConfig::with_max_scrollback_bytes(DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES),
    )
    .expect("test should construct Ghostty replay terminal");
    terminal
        .import_snapshot(payload)
        .expect("test should import daemon Ghostty snapshot");
    terminal
        .plain_text()
        .expect("test should format replayed Ghostty snapshot")
}

fn retained_ghostty_scrollback_markers(plain_text: &str, marker_count: usize) -> Vec<usize> {
    (0..marker_count)
        .filter(|line| plain_text.contains(&format!("echo:scrollback-line-{line:05}")))
        .collect()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

fn assert_retained_exit_output(
    drained: &botster_core_daemon::DrainResult,
    session_id: &SessionId,
    expected: &str,
) {
    let output = terminal_output(&drained.client_egress);
    assert_eq!(
        count_occurrences(&output, expected),
        1,
        "retained final terminal output should be delivered exactly once: {output:?}"
    );
    assert!(
        drained.observations.iter().any(|observation| {
            matches!(
                observation,
                BotsterEngineObservation::SessionLifecycle {
                    session_id: observed_session,
                    state: SessionLifecycleState::Exited { .. },
                } if observed_session == session_id
            )
        }),
        "retained drain should include the process-exit lifecycle observation: {:?}",
        drained.observations
    );
}

fn assert_no_duplicate_exit_output(drained: &botster_core_daemon::DrainResult, expected: &str) {
    let output = terminal_output(&drained.client_egress);
    assert_eq!(count_occurrences(&output, expected), 0);
    assert!(drained.observations.iter().all(|observation| !matches!(
        observation,
        BotsterEngineObservation::SessionLifecycle {
            state: SessionLifecycleState::Exited { .. },
            ..
        }
    )));
}

fn terminal_output(frames: &[(ClientId, TransportEgress)]) -> String {
    frames
        .iter()
        .filter_map(|(_, frame)| match frame {
            TransportEgress::TerminalOutput { data, .. } => {
                Some(String::from_utf8_lossy(data).to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn first_snapshot_for_client(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
) -> Option<(usize, botster_core::TerminalSnapshotPayload)> {
    let index = frames
        .iter()
        .enumerate()
        .find_map(|(index, (target, frame))| {
            (target == client_id && matches!(frame, TransportEgress::Snapshot { .. }))
                .then_some(index)
        })?;
    let bytes = frames
        .iter()
        .filter_map(|(target, frame)| match frame {
            TransportEgress::Snapshot { data, .. } if target == client_id => Some(data.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    Some((
        index,
        botster_core::TerminalSnapshotPayload::new(
            bytes,
            TerminalScreenSize::new(24, 80),
            Some(EXPECTED_SNAPSHOT_FORMAT.to_string()),
        ),
    ))
}

fn first_snapshot_for_client_at_size(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
    size: TerminalScreenSize,
) -> Option<(usize, botster_core::TerminalSnapshotPayload)> {
    let index = frames
        .iter()
        .enumerate()
        .find_map(|(index, (target, frame))| {
            (target == client_id && matches!(frame, TransportEgress::Snapshot { .. }))
                .then_some(index)
        })?;
    let bytes = frames
        .iter()
        .filter_map(|(target, frame)| match frame {
            TransportEgress::Snapshot { data, .. } if target == client_id => Some(data.as_slice()),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    Some((
        index,
        botster_core::TerminalSnapshotPayload::new(
            bytes,
            size,
            Some(EXPECTED_SNAPSHOT_FORMAT.to_string()),
        ),
    ))
}

fn first_terminal_output_index_for_client_containing(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
    expected: &str,
) -> Option<usize> {
    frames
        .iter()
        .position(|(frame_client_id, frame)| match frame {
            TransportEgress::TerminalOutput { data, .. } if frame_client_id == client_id => {
                String::from_utf8_lossy(data).contains(expected)
            }
            _ => false,
        })
}

fn renderable_output(frames: &[(ClientId, TransportEgress)]) -> String {
    frames
        .iter()
        .filter_map(|(_, frame)| renderable_frame_data(frame))
        .collect::<Vec<_>>()
        .join("")
}

fn renderable_output_for_client(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
) -> String {
    frames
        .iter()
        .filter(|(frame_client_id, _)| frame_client_id == client_id)
        .filter_map(|(_, frame)| renderable_frame_data(frame))
        .collect::<Vec<_>>()
        .join("")
}

fn renderable_frame_data(frame: &TransportEgress) -> Option<String> {
    match frame {
        TransportEgress::TerminalOutput { data, .. }
        | TransportEgress::Snapshot { data, .. }
        | TransportEgress::Scrollback { data, .. } => {
            Some(String::from_utf8_lossy(data).to_string())
        }
        _ => None,
    }
}

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-daemon-{label}-{nanos}"))
}

fn short_temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::path::PathBuf::from("/tmp").join(format!("bcd-{label}-{nanos}"))
}

#[cfg(unix)]
fn worker_process_evidence(
    daemon: &CoreDaemon,
    session_id: &SessionId,
) -> (u32, u32, std::path::PathBuf) {
    let record = daemon
        .registry()
        .load(session_id)
        .expect("load worker registry record")
        .expect("worker registry record");
    let worker_pid = record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_pid"))
        .and_then(serde_json::Value::as_u64)
        .expect("worker pid in recovery identity") as u32;
    let pty_child_pid = record
        .process
        .and_then(|process| process.pid)
        .expect("PTY child pid in registry process identity");
    let socket_path = record
        .recovery_identity
        .as_ref()
        .and_then(|identity| identity.get("worker_control_socket"))
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
        .expect("worker socket in recovery identity");
    (worker_pid, pty_child_pid, socket_path)
}

#[cfg(unix)]
fn signal_process(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .expect("run process signal command");
    assert!(status.success(), "signal {signal} to process {pid}");
}

#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(unix)]
fn wait_for_condition(label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + REAL_WORKER_COMPLETION_TIMEOUT;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("{label} was not observed within {REAL_WORKER_COMPLETION_TIMEOUT:?}");
}

#[cfg(unix)]
struct StoppedProcess {
    pid: u32,
    resumed: bool,
}

#[cfg(unix)]
impl StoppedProcess {
    fn new(pid: u32) -> Self {
        signal_process(pid, "STOP");
        Self {
            pid,
            resumed: false,
        }
    }

    fn resume(&mut self) {
        if !self.resumed {
            signal_process(self.pid, "CONT");
            self.resumed = true;
        }
    }
}

#[cfg(unix)]
impl Drop for StoppedProcess {
    fn drop(&mut self) {
        if !self.resumed && process_exists(self.pid) {
            let _ = Command::new("kill")
                .arg("-CONT")
                .arg(self.pid.to_string())
                .status();
        }
    }
}

fn worker_path() -> std::path::PathBuf {
    static BUILD_WORKER: Once = Once::new();
    BUILD_WORKER.call_once(|| {
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "botster-core-daemon",
                "--bin",
                "botster-session-worker",
            ])
            .status()
            .expect("worker binary build command should run");
        assert!(
            status.success(),
            "worker binary should build for daemon restart test"
        );
    });

    let mut path = std::env::current_exe().expect("test executable path should resolve");
    while path.file_name().and_then(|name| name.to_str()) != Some("debug") {
        assert!(path.pop(), "test executable should live under target/debug");
    }
    path.join("botster-session-worker")
}
