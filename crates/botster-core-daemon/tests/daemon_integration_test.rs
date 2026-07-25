#![allow(missing_docs)]

use std::fs;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::sync::Once;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(feature = "ghostty-terminal")]
use botster_core::TerminalScreenSize;
use botster_core::{
    BotsterEngineObservation, ClientId, CoreSessionMetadata, EndpointId, EnvelopeCursor,
    EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget, ModeFlags, NotificationContent,
    NotificationDeliveryStatus, NotificationId, NotificationItem, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp, RequestId, ResizePayload,
    RoutedEnvelope, RoutedEnvelopeObservation, RoutedEnvelopePayload, RoutedEnvelopeQueueConfig,
    SessionId, SessionLifecycleState, SessionSpawnRequest, SessionWorkerHealthReason,
    SessionWorkerStaleReason, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TerminalAttachState, TransportEgress,
};
use botster_core_daemon::{
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, CaptureSnapshotRequest,
    CoreDaemon, CoreDaemonConfig, CoreDaemonError, DrainNotificationsRequest,
    DrainRoutedEnvelopesRequest, GuardedWriteDecision, GuardedWriteDeliveryState,
    GuardedWriteRequest, PostNotificationRequest, PublishRoutedEnvelopeRequest,
    ReadModeFlagsRequest, ReadScreenRequest, ReadinessEvidence, RegistrySessionState,
    SafeWriteIndicator, SessionAdoptionState, SessionLifecycleChangeKind,
    SessionLifecycleResyncReason, SpawnSessionRequest,
};
use botster_core_daemon::{
    DEFAULT_GHOSTTY_MAX_SCROLLBACK_BYTES, DEFAULT_LIFECYCLE_JOURNAL_CAPACITY,
};
#[cfg(feature = "ghostty-terminal")]
use botster_terminal_ghostty::{GhosttyAdapterConfig, GhosttyTerminal};

#[cfg(feature = "ghostty-terminal")]
const EXPECTED_SNAPSHOT_FORMAT: &str = "ghostty-terminal-snapshot-v1";
#[cfg(not(feature = "ghostty-terminal"))]
const EXPECTED_SNAPSHOT_FORMAT: &str = "plain-opaque-v1";
#[cfg(feature = "ghostty-terminal")]
const EXPECTED_GHOSTTY_SNAPSHOT_SIZE_CEILING: usize = 16 * 1024 * 1024;
#[cfg(feature = "ghostty-terminal")]
const EXPECTED_GHOSTTY_MIN_RETAINED_MARKERS: usize = 4_000;
#[cfg(feature = "ghostty-terminal")]
const EXPECTED_GHOSTTY_DROPPED_MARKER: &str = "echo:scrollback-line-00000";
#[cfg(feature = "ghostty-terminal")]
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

    daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription,
            13,
        )
        .expect("late attach should retain engine output for daemon drain");
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
    #[cfg(feature = "ghostty-terminal")]
    {
        let (snapshot_index, snapshot) =
            first_snapshot_for_client(&late_drain.client_egress, &late_client)
                .expect("late Ghostty attach should deliver an opaque snapshot replay");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:before-late-attach");
        let live_index = first_terminal_output_index_for_client_containing(
            &late_drain.client_egress,
            &late_client,
            "echo:after-late-attach",
        )
        .expect("late client should receive later live output");
        assert!(
            snapshot_index < live_index,
            "late Ghostty snapshot replay should precede later live output: {:?}",
            late_drain.client_egress
        );
    }
    #[cfg(not(feature = "ghostty-terminal"))]
    {
        let late_output = renderable_output_for_client(&late_drain.client_egress, &late_client);
        let history_index = late_output
            .find("echo:before-late-attach")
            .unwrap_or_else(|| panic!("late attach should replay prior marker: {late_output:?}"));
        let live_index = late_output
            .find("echo:after-late-attach")
            .unwrap_or_else(|| {
                panic!("late client should receive later live output: {late_output:?}")
            });
        assert!(
            history_index < live_index,
            "late replay should precede later live output for the subscription: {late_output:?}"
        );
    }

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

    #[cfg(feature = "ghostty-terminal")]
    let (captured, output) = capture_snapshot_and_retained_output_until(
        &mut daemon,
        &session_id,
        "echo:snapshot-marker",
        13,
    );
    #[cfg(not(feature = "ghostty-terminal"))]
    let captured = capture_snapshot_until(&mut daemon, &session_id, "echo:snapshot-marker", 13);
    assert_eq!(
        captured.snapshot.request_id,
        RequestId("capture-snapshot-marker".to_string())
    );
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_snapshot_format(&captured.payload);
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert_snapshot_payload_observed_marker_when_plain(&captured.payload, "echo:snapshot-marker");
    #[cfg(feature = "ghostty-terminal")]
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:snapshot-marker");

    #[cfg(not(feature = "ghostty-terminal"))]
    let drained = daemon
        .drain(&session_id, 30)
        .expect("drain after capture_snapshot should succeed");
    #[cfg(not(feature = "ghostty-terminal"))]
    let output = terminal_output(&drained.client_egress);
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
    #[cfg(feature = "ghostty-terminal")]
    assert_ghostty_snapshot_replays_marker(&captured.payload, "echo:local-marker");

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(all(unix, feature = "ghostty-terminal"))]
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

#[cfg(all(unix, feature = "ghostty-terminal"))]
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
    daemon
        .attach(
            reattached_client.clone(),
            session_id.clone(),
            reattached_subscription.clone(),
            17,
        )
        .expect("fresh client and subscription should reattach the running session");

    let reattach_drain = daemon
        .drain(&session_id, 18)
        .expect("reattach bootstrap should drain");
    let (snapshot_index, snapshot) =
        first_snapshot_for_client(&reattach_drain.client_egress, &reattached_client)
            .expect("reattach should deliver the authoritative Ghostty snapshot");
    let snapshot_subscription = match &reattach_drain.client_egress[snapshot_index] {
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

    let attaching_index = reattach_drain
        .client_egress
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
    let attached_index = reattach_drain
        .client_egress
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
        reattach_drain.client_egress
    );
    assert!(
        reattach_drain
            .client_egress
            .iter()
            .all(|(client_id, _)| client_id != &original_client),
        "detached client must not receive reattach-only delivery: {:?}",
        reattach_drain.client_egress
    );

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

#[cfg(all(unix, feature = "ghostty-terminal"))]
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

    daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-late-subscription".to_string()),
            101,
        )
        .expect("late attach should receive a scrollback snapshot");
    let late_drain = daemon
        .drain(&session_id, 102)
        .expect("late attach drain should succeed");
    let (_, snapshot) = first_snapshot_for_client(&late_drain.client_egress, &late_client)
        .expect("late Ghostty attach should include a snapshot frame");
    assert_ghostty_snapshot_replays_marker(&snapshot, "echo:scrollback-line-0000");

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(all(unix, feature = "ghostty-terminal"))]
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

    daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            SubscriptionId("dwgs-override-late-subscription".to_string()),
            101,
        )
        .expect("late attach should receive a scrollback snapshot");
    let late_drain = daemon
        .drain(&session_id, 102)
        .expect("late attach drain should succeed");
    let (_, snapshot) = first_snapshot_for_client(&late_drain.client_egress, &late_client)
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

#[cfg(all(unix, feature = "ghostty-terminal"))]
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

    daemon
        .attach(
            late_client.clone(),
            session_id.clone(),
            late_subscription.clone(),
            13,
        )
        .expect("late attach should retain initial history");
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
    let attaching_index = late_drain
        .client_egress
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
    let snapshot_index = late_drain
        .client_egress
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
    let attached_index = late_drain
        .client_egress
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
    let live_index = late_drain
        .client_egress
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
        late_drain.client_egress
    );
    #[cfg(feature = "ghostty-terminal")]
    {
        let (snapshot_index, snapshot) =
            first_snapshot_for_client(&late_drain.client_egress, &late_client)
                .expect("late Ghostty attach should keep an opaque snapshot replay pending");
        assert_ghostty_snapshot_replays_marker(&snapshot, "echo:worker-before-late");
        let live_index = first_terminal_output_index_for_client_containing(
            &late_drain.client_egress,
            &late_client,
            "echo:worker-after-read",
        )
        .expect("read_screen internal drain should remain pending");
        assert!(
            snapshot_index < live_index,
            "attach snapshot replay should merge before read_screen pending drain: {:?}",
            late_drain.client_egress
        );
    }
    #[cfg(not(feature = "ghostty-terminal"))]
    {
        let late_output = renderable_output_for_client(&late_drain.client_egress, &late_client);
        let history_index = late_output
            .find("echo:worker-before-late")
            .unwrap_or_else(|| {
                panic!("late attach history should remain pending: {late_output:?}")
            });
        let live_index = late_output
            .find("echo:worker-after-read")
            .unwrap_or_else(|| {
                panic!("read_screen internal drain should remain pending: {late_output:?}")
            });
        assert!(
            history_index < live_index,
            "attach pending drain should merge before read_screen pending drain: {late_output:?}"
        );
    }

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(all(unix, not(feature = "ghostty-terminal")))]
#[test]
fn worker_backed_empty_initial_snapshot_attaches_before_live_output_without_history() {
    let data_dir = temp_data_dir("worker-empty-initial-snapshot");
    let mut daemon =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let session_id = SessionId("worker-empty-session".to_string());
    let client_id = ClientId("worker-empty-client".to_string());
    let subscription_id = SubscriptionId("worker-empty-subscription".to_string());

    daemon
        .spawn(silent_spawn_request(&session_id), 10)
        .expect("silent worker-backed daemon should spawn");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            11,
        )
        .expect("silent worker-backed attach should subscribe");
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"after-empty-snapshot\n".to_vec(),
            12,
        )
        .expect("live marker should write after empty snapshot request");

    let drained = drain_until_for_client(
        &mut daemon,
        &session_id,
        &client_id,
        "echo:after-empty-snapshot",
    );
    let client_frames = drained
        .client_egress
        .iter()
        .filter(|(frame_client_id, _)| frame_client_id == &client_id)
        .map(|(_, frame)| frame)
        .collect::<Vec<_>>();
    assert!(client_frames.iter().all(|frame| !matches!(
        frame,
        TransportEgress::Snapshot { .. } | TransportEgress::Scrollback { .. }
    )));
    let attaching_index = client_frames
        .iter()
        .position(|frame| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    subscription_id: frame_subscription_id,
                    state: TerminalAttachState::Attaching,
                    ..
                } if frame_subscription_id == &subscription_id
            )
        })
        .expect("empty worker-backed attach should emit Attaching");
    let attached_index = client_frames
        .iter()
        .position(|frame| {
            matches!(
                frame,
                TransportEgress::AttachState {
                    subscription_id: frame_subscription_id,
                    state: TerminalAttachState::Attached,
                    ..
                } if frame_subscription_id == &subscription_id
            )
        })
        .expect("empty InitialSnapshotReady should emit Attached");
    let live_index = client_frames
        .iter()
        .position(|frame| {
            matches!(
                frame,
                TransportEgress::TerminalOutput {
                    subscription_id: frame_subscription_id,
                    data,
                    ..
                } if frame_subscription_id == &subscription_id
                    && String::from_utf8_lossy(data).contains("echo:after-empty-snapshot")
            )
        })
        .expect("live output should flow after empty snapshot readiness");
    assert!(
        attaching_index < attached_index && attached_index < live_index,
        "empty worker-backed readiness must order Attaching before Attached before live output: {client_frames:?}"
    );

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
    #[cfg(feature = "ghostty-terminal")]
    daemon
        .input(
            client_id.clone(),
            session_id.clone(),
            b"enable-mouse\n".to_vec(),
            16,
        )
        .expect("mouse mode DECSET should write");
    #[cfg(feature = "ghostty-terminal")]
    let _ = read_screen_until(&mut daemon, &session_id, "echo:enable-mouse", 17);
    #[cfg(feature = "ghostty-terminal")]
    let live_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("live-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 18,
        })
        .expect("live mode flags should be authoritative");
    #[cfg(feature = "ghostty-terminal")]
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
    #[cfg(feature = "ghostty-terminal")]
    let retained_mode_flags = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("retained-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 25,
        })
        .expect("shutdown mode flags should serve retained terminal truth");

    assert!(first_screen.screen.text.contains("echo:shutdown-final"));
    #[cfg(feature = "ghostty-terminal")]
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
    #[cfg(feature = "ghostty-terminal")]
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

#[cfg(not(feature = "ghostty-terminal"))]
#[test]
fn plain_backend_mode_flags_are_unsupported_instead_of_default_authority() {
    let data_dir = temp_data_dir("plain-mode-flags-unsupported");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("plain-mode-flags-session".to_string());
    daemon
        .spawn(spawn_request(&session_id), 10)
        .expect("spawn plain-backend session");

    let error = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("plain-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 11,
        })
        .expect_err("plain backend must not fabricate all-false authority");

    assert!(matches!(
        error,
        CoreDaemonError::Engine(
            botster_core::ManagedSessionRuntimeError::UnsupportedSessionRequest {
                request_kind: "mode_flags",
            }
        )
    ));
    daemon
        .shutdown(Some(session_id.clone()), 12)
        .expect("plain-backend shutdown should retain unsupported mode state");
    let retained_error = daemon
        .read_mode_flags(ReadModeFlagsRequest {
            request_id: RequestId("plain-retained-mode-flags".to_string()),
            session_id: session_id.clone(),
            now_seconds: 13,
        })
        .expect_err("retained plain mode state must remain unsupported");
    assert!(matches!(
        retained_error,
        CoreDaemonError::Engine(
            botster_core::ManagedSessionRuntimeError::UnsupportedSessionRequest {
                request_kind: "mode_flags",
            }
        )
    ));
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
            "worker-backed daemon should persist a reconnectable worker endpoint"
        );
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
    let old_cursor = {
        let mut daemon =
            CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
        let baseline = daemon
            .lifecycle_baseline()
            .expect("first-generation baseline");
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("first generation should spawn worker");
        let cursor = daemon.lifecycle_changes(&baseline.cursor).cursor;
        daemon.release_for_restart();
        cursor
    };

    let mut restarted =
        CoreDaemon::new(CoreDaemonConfig::new(&data_dir).with_worker_path(worker_path()));
    let restarted_baseline = restarted
        .lifecycle_baseline()
        .expect("restart baseline should expose durable registry truth");
    assert_eq!(restarted_baseline.sessions.len(), 1);
    assert_eq!(
        restarted_baseline.sessions[0].session.session_id,
        session_id
    );
    assert!(restarted_baseline.sessions[0].lifecycle.is_none());
    let foreign = restarted.lifecycle_changes(&old_cursor);
    assert!(foreign.changes.is_empty());
    assert_eq!(
        foreign.resync_required,
        Some(SessionLifecycleResyncReason::SourceChanged)
    );

    restarted
        .adopt_session(&session_id, 12)
        .expect("fresh daemon should adopt from real worker protocol evidence");
    let adopted = restarted.lifecycle_changes(&restarted_baseline.cursor);
    assert_eq!(adopted.changes.len(), 1);
    assert!(matches!(
        &adopted.changes[0].kind,
        SessionLifecycleChangeKind::Upsert { record }
            if record.session.session_id == session_id
                && record.session.registry_state == RegistrySessionState::Running
                && matches!(record.lifecycle, Some(SessionLifecycleState::Running))
    ));
    assert_eq!(
        restarted
            .lifecycle_baseline()
            .expect("post-adoption baseline")
            .sessions
            .len(),
        1,
        "adoption must not fabricate a duplicate session"
    );

    restarted
        .shutdown(Some(session_id.clone()), 20)
        .expect("adopted worker should shut down cleanly");
    assert!(restarted
        .remove_session(&session_id)
        .expect("adopted terminal worker should be removable"));
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

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(not(feature = "ghostty-terminal"))]
fn silent_spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "while IFS= read -r line; do printf \"echo:%s\\n\" \"$line\"; done".to_string(),
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

fn drain_until_for_client(
    daemon: &mut CoreDaemon,
    session_id: &SessionId,
    client_id: &ClientId,
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
        if renderable_output_for_client(&aggregate.client_egress, client_id).contains(expected) {
            return aggregate;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    aggregate
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

#[cfg(all(unix, feature = "ghostty-terminal"))]
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
        #[cfg(feature = "ghostty-terminal")]
        if ghostty_snapshot_replays_marker(&captured.payload, expected) {
            return captured;
        }
        #[cfg(not(feature = "ghostty-terminal"))]
        if String::from_utf8_lossy(&captured.payload.bytes).contains(expected) {
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

#[cfg(feature = "ghostty-terminal")]
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
    #[cfg(not(feature = "ghostty-terminal"))]
    assert!(
        String::from_utf8_lossy(&payload.bytes).contains(expected),
        "plain fallback snapshot should retain raw marker bytes"
    );
    #[cfg(feature = "ghostty-terminal")]
    let _ = (payload, expected);
}

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(feature = "ghostty-terminal")]
fn ghostty_snapshot_replays_marker(
    payload: &botster_core::TerminalSnapshotPayload,
    expected: &str,
) -> bool {
    ghostty_snapshot_plain_text(payload).contains(expected)
}

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(feature = "ghostty-terminal")]
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

#[cfg(feature = "ghostty-terminal")]
fn first_snapshot_for_client(
    frames: &[(ClientId, TransportEgress)],
    client_id: &ClientId,
) -> Option<(usize, botster_core::TerminalSnapshotPayload)> {
    frames
        .iter()
        .enumerate()
        .find_map(|(index, (frame_client_id, frame))| {
            if frame_client_id != client_id {
                return None;
            }
            match frame {
                TransportEgress::Snapshot { data, .. } => Some((
                    index,
                    botster_core::TerminalSnapshotPayload::new(
                        data.clone(),
                        TerminalScreenSize::new(24, 80),
                        Some(EXPECTED_SNAPSHOT_FORMAT.to_string()),
                    ),
                )),
                _ => None,
            }
        })
}

#[cfg(feature = "ghostty-terminal")]
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
                "botster-core",
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
