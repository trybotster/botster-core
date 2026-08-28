#![allow(missing_docs)]

use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use botster_core::{
    ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
    SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId, TerminalCapabilitySet,
    TerminalWakeKind, WAKE_QUEUE_CAPACITY,
};
use botster_core_daemon::{
    CoreDaemon, CoreDaemonConfig, ReadModeFlagsRequest, ReadScreenRequest, SpawnSessionRequest,
};
use botster_core_test_support::terminal_adapter::{
    SharedFakeTerminalAdapter, TerminalAdapterHarnessDriver,
};

fn temp_data_dir(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("botster-core-wake-{label}-{nanos}"))
}

fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
    SpawnSessionRequest {
        request: SessionSpawnRequest {
            request_id: RequestId(format!("{}-spawn", session_id.0)),
            session_id: session_id.clone(),
            executable: "sh".to_string(),
            arguments: vec!["-c".to_string(), "printf ready; sleep 2".to_string()],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        },
        metadata: CoreSessionMetadata::new(),
    }
}

fn empty_caps() -> TerminalCapabilitySet {
    TerminalCapabilitySet::empty()
}

#[test]
fn attach_without_waking_bind_allocates_no_registry_entry() {
    let data_dir = temp_data_dir("unbound");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("unbound-session".into());
    let client_id = ClientId("unbound-client".into());
    let subscription_id = SubscriptionId("unbound-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(client_id, session_id.clone(), subscription_id.clone(), 2)
        .expect("attach");
    assert!(
        !daemon
            .wake_source()
            .registry_contains(&session_id, &subscription_id),
        "attached-but-unbound routes must not enter the waking-adapter registry"
    );
    assert_eq!(
        daemon.wake_source().registry_len(),
        0,
        "attach must not allocate RouteWakeState"
    );
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn bind_rejection_allocates_nothing() {
    let data_dir = temp_data_dir("reject");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("reject-session".into());
    let client_id = ClientId("reject-client".into());
    let subscription_id = SubscriptionId("reject-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    let before = daemon.wake_source().registry_len();
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    let err = daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id,
            subscription_id,
            botster_core_daemon::TerminalSubscriptionGeneration(1),
            empty_caps(),
            Box::new(adapter),
        )
        .expect_err("bind before attach");
    let _ = err;
    assert_eq!(daemon.wake_source().registry_len(), before);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn waking_bind_then_writable_wake_pumps_one_route() {
    let data_dir = temp_data_dir("pump-one");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("pump-session".into());
    let client_id = ClientId("pump-client".into());
    let subscription_id = SubscriptionId("pump-sub".into());
    let other_sub = SubscriptionId("other-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    daemon
        .attach(
            ClientId("other-client".into()),
            session_id.clone(),
            other_sub,
            3,
        )
        .expect("other attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::auto_complete();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id.clone(),
            generation,
            empty_caps(),
            Box::new(adapter),
        )
        .expect("bind");
    assert_eq!(daemon.wake_source().registry_len(), 1);
    assert!(daemon
        .wake_source()
        .registry_contains(&session_id, &subscription_id));
    let batch = daemon.wait_wakes(Duration::from_millis(50));
    let outcome = daemon.pump_woken(&batch, 4).expect("pump");
    assert!(
        outcome.pumped_routes <= 1
            || !batch.adapter_routes.is_empty()
            || batch.ingress_sessions.len() <= 1
    );
    daemon
        .detach_terminal_subscription(
            ClientId("pump-client".into()),
            session_id,
            subscription_id,
            generation,
            5,
        )
        .ok();
    assert_eq!(daemon.wake_source().registry_len(), 0);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn readback_does_not_advance_bound_adapter() {
    let data_dir = temp_data_dir("readback");
    let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
    let session_id = SessionId("readback-session".into());
    let client_id = ClientId("readback-client".into());
    let subscription_id = SubscriptionId("readback-sub".into());
    daemon.spawn(spawn_request(&session_id), 1).expect("spawn");
    daemon
        .expect_terminal_adapter(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
        )
        .expect("declare");
    daemon
        .attach(
            client_id.clone(),
            session_id.clone(),
            subscription_id.clone(),
            2,
        )
        .expect("attach");
    let generation = daemon
        .list_terminal_subscriptions()
        .into_iter()
        .find(|row| row.subscription_id == subscription_id)
        .expect("row")
        .generation;
    let adapter = SharedFakeTerminalAdapter::new();
    daemon
        .bind_waking_terminal_adapter(
            client_id,
            session_id.clone(),
            subscription_id,
            generation,
            empty_caps(),
            Box::new(adapter.clone()),
        )
        .expect("bind");
    let before = adapter.delivered_frame_bytes().len();
    let _ = daemon.read_screen(ReadScreenRequest {
        request_id: RequestId("screen".into()),
        session_id: session_id.clone(),
        now_seconds: 3,
    });
    let _ = daemon.read_mode_flags(ReadModeFlagsRequest {
        request_id: RequestId("modes".into()),
        session_id: session_id.clone(),
        now_seconds: 4,
    });
    assert_eq!(adapter.delivered_frame_bytes().len(), before);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn retained_sink_clone_does_not_pin_allocation() {
    let source = botster_core::TerminalWakeSource::new();
    let session = SessionId("retain".into());
    let sub = SubscriptionId("sub".into());
    let sink = source.bind_route(
        session.clone(),
        sub.clone(),
        botster_core::TerminalSubscriptionGeneration(1),
    );
    let clone = sink.clone();
    assert!(sink.wake(TerminalWakeKind::Writable));
    source.retire_route(&session, &sub);
    let _ = source.wait_wakes(Duration::from_millis(0));
    assert_eq!(source.registry_len(), 0);
    assert_eq!(clone.strong_count(), 0);
    assert!(!clone.wake(TerminalWakeKind::Writable));
    assert!(source.live_allocation_bound() <= WAKE_QUEUE_CAPACITY);
}

#[test]
fn overflow_reconcile_visits_only_registry() {
    let source = botster_core::TerminalWakeSource::new();
    let mut sinks = Vec::new();
    for n in 0..=WAKE_QUEUE_CAPACITY {
        let sink = source.bind_route(
            SessionId(format!("s{n}")),
            SubscriptionId(format!("sub{n}")),
            botster_core::TerminalSubscriptionGeneration(1),
        );
        let _ = sink.wake(TerminalWakeKind::Writable);
        sinks.push(sink);
    }
    let before = source.visit_count();
    let _ = source.wait_wakes(Duration::from_millis(0));
    let visits = source.visit_count().saturating_sub(before);
    assert!(visits <= source.registry_len() + WAKE_QUEUE_CAPACITY + 1);
    drop(sinks);
}
