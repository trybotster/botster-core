#![allow(missing_docs)]

use std::fs;
use std::process::Command;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

use botster_core::{
    BotsterEngineObservation, ClientId, CoreSessionMetadata, EndpointId, EnvelopeCursor,
    EnvelopeDeliveryStatus, EnvelopeId, EnvelopeTarget, ModeFlags, NotificationContent,
    NotificationDeliveryStatus, NotificationId, NotificationItem, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp, RequestId, ResizePayload,
    RoutedEnvelope, RoutedEnvelopeObservation, RoutedEnvelopePayload, RoutedEnvelopeQueueConfig,
    SessionId, SessionLifecycleState, SessionSpawnRequest, SessionWorkerHealthReason,
    SessionWorkerStaleReason, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    TransportEgress,
};
use botster_core_daemon::{
    AcknowledgeNotificationRequest, AcknowledgeRoutedEnvelopeRequest, CaptureSnapshotRequest,
    CoreDaemon, CoreDaemonConfig, CoreDaemonError, DrainNotificationsRequest,
    DrainRoutedEnvelopesRequest, GuardedWriteDecision, GuardedWriteDeliveryState,
    GuardedWriteRequest, PostNotificationRequest, PublishRoutedEnvelopeRequest, ReadScreenRequest,
    ReadinessEvidence, RegistrySessionState, SafeWriteIndicator, SessionAdoptionState,
    SpawnSessionRequest,
};

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
    let late_output = renderable_output_for_client(&late_drain.client_egress, &late_client);
    let history_index = late_output
        .find("echo:before-late-attach")
        .unwrap_or_else(|| panic!("late attach should replay prior marker: {late_output:?}"));
    let live_index = late_output
        .find("echo:after-late-attach")
        .unwrap_or_else(|| panic!("late client should receive later live output: {late_output:?}"));
    assert!(
        history_index < live_index,
        "late replay should precede later live output for the subscription: {late_output:?}"
    );

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

    let captured = capture_snapshot_until(&mut daemon, &session_id, "echo:snapshot-marker", 13);
    assert_eq!(
        captured.snapshot.request_id,
        RequestId("capture-snapshot-marker".to_string())
    );
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_eq!(captured.payload.format.as_deref(), Some("plain-opaque-v1"));
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert!(
        String::from_utf8_lossy(&captured.payload.bytes).contains("echo:snapshot-marker"),
        "capture_snapshot should internally drain before capturing"
    );

    let drained = daemon
        .drain(&session_id, 30)
        .expect("drain after capture_snapshot should succeed");
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
fn local_daemon_read_screen_and_capture_snapshot_use_in_process_engine_path() {
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
        "local daemon read_screen should use the in-process engine path"
    );

    let captured = capture_snapshot_until(&mut daemon, &session_id, "echo:local-marker", 14);
    assert_eq!(captured.snapshot.session_id, session_id);
    assert_eq!(captured.payload.format.as_deref(), Some("plain-opaque-v1"));
    assert_eq!(captured.snapshot.data, captured.payload.bytes);
    assert!(
        String::from_utf8_lossy(&captured.payload.bytes).contains("echo:local-marker"),
        "local daemon capture_snapshot should use the in-process engine path"
    );

    let _ = fs::remove_dir_all(data_dir);
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
            late_subscription,
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
    let late_output = renderable_output_for_client(&late_drain.client_egress, &late_client);
    let history_index = late_output
        .find("echo:worker-before-late")
        .unwrap_or_else(|| panic!("late attach history should remain pending: {late_output:?}"));
    let live_index = late_output
        .find("echo:worker-after-read")
        .unwrap_or_else(|| {
            panic!("read_screen internal drain should remain pending: {late_output:?}")
        });
    assert!(
        history_index < live_index,
        "attach pending drain should merge before read_screen pending drain: {late_output:?}"
    );

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn daemon_screen_and_snapshot_negative_paths_return_errors_without_panics() {
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
    daemon
        .spawn(spawn_request(&session_id), 11)
        .expect("worker-backed daemon should spawn");
    let screen = daemon
        .read_screen(ReadScreenRequest {
            request_id: RequestId("empty-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 12,
        })
        .expect("spawned never explicitly drained session should still read screen");
    assert_eq!(screen.screen.session_id, session_id);
    let snapshot = daemon
        .capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("empty-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 13,
        })
        .expect("spawned never explicitly drained session should still capture snapshot");
    assert_eq!(snapshot.snapshot.session_id, session_id);
    assert_eq!(snapshot.payload.format.as_deref(), Some("plain-opaque-v1"));

    daemon
        .shutdown(Some(session_id.clone()), 20)
        .expect("shutdown should succeed");
    assert!(matches!(
        daemon.read_screen(ReadScreenRequest {
            request_id: RequestId("shutdown-screen".to_string()),
            session_id: session_id.clone(),
            now_seconds: 21,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
    ));
    assert!(matches!(
        daemon.capture_snapshot(CaptureSnapshotRequest {
            request_id: RequestId("shutdown-snapshot".to_string()),
            session_id: session_id.clone(),
            now_seconds: 22,
        }),
        Err(CoreDaemonError::SessionNotReadable(session)) if session == session_id
    ));

    let _ = fs::remove_dir_all(data_dir);
}

#[cfg(unix)]
#[test]
fn natural_exit_read_screen_and_capture_snapshot_error_on_first_readback() {
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

    let screen_result = daemon.read_screen(ReadScreenRequest {
        request_id: RequestId("natural-exit-screen".to_string()),
        session_id: screen_session.clone(),
        now_seconds: 13,
    });
    assert!(
        matches!(
            screen_result,
            Err(CoreDaemonError::SessionNotReadable(ref session)) if session == &screen_session
        ),
        "first natural-exit read_screen should fail; got {screen_result:?}; sessions: {:?}",
        daemon.list()
    );
    let screen_drain = daemon
        .drain(&screen_session, 14)
        .expect("drain after refused read_screen should return retained final output");
    assert_retained_exit_output(&screen_drain, &screen_session, "echo:screen-exit");

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

    let snapshot_result = daemon.capture_snapshot(CaptureSnapshotRequest {
        request_id: RequestId("natural-exit-snapshot".to_string()),
        session_id: snapshot_session.clone(),
        now_seconds: 23,
    });
    assert!(
        matches!(
            snapshot_result,
            Err(CoreDaemonError::SessionNotReadable(ref session)) if session == &snapshot_session
        ),
        "first natural-exit capture_snapshot should fail; got {snapshot_result:?}; sessions: {:?}",
        daemon.list()
    );
    let snapshot_drain = daemon
        .drain(&snapshot_session, 24)
        .expect("drain after refused capture_snapshot should return retained final output");
    assert_retained_exit_output(&snapshot_drain, &snapshot_session, "echo:snapshot-exit");

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
    let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
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
    let mut last = None;
    for tick in 0..100 {
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
        last = Some(read);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!(
        "read_screen never observed {expected:?}; last text: {:?}",
        last.map(|read| read.screen.text)
    )
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
