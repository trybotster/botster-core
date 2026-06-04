//! Durable session-worker protocol contract tests.
//!
//! These tests prove representational completeness of public `botster_core`
//! contracts. They do not prove runtime scheduling, daemon restart, socket I/O,
//! or worker adoption behavior.

use botster_core::{
    BackpressureRoute, BackpressureSummary, ClientId, CoreSessionMetadata, DaemonCliOperation,
    DaemonControlOperation, DaemonControlOutcome, DeliveryLag, DurableRestartSemantics,
    DurableSessionProtocolVersion, GuardedSessionWriteDeferralReason, GuardedSessionWritePolicy,
    GuardedSessionWritePrimitive, GuardedSessionWriteRejectionReason, GuardedSessionWriteRequest,
    GuardedSessionWriteState, ModeFlags, NotificationDeliveryStatus, NotificationId, QueueSource,
    RequestId, RestartBoundary, RestartSurvival, SessionActivity, SessionId, SessionLifecycleState,
    SessionReadinessEvidence, SessionWorkerAdoptRequest, SessionWorkerAdoptionVerdict,
    SessionWorkerAttachRequest, SessionWorkerCapability, SessionWorkerDetached,
    SessionWorkerHealth, SessionWorkerHealthReason, SessionWorkerHeartbeat, SessionWorkerId,
    SessionWorkerIdentity, SessionWorkerOutputFrame, SessionWorkerProcessIdentity,
    SessionWorkerQueueLimits, SessionWorkerShutdownMode, SessionWorkerShutdownRequest,
    SessionWorkerSpawnRequest, SessionWorkerSpawned, SessionWorkerStaleReason,
    SlowConsumerBehavior, SnapshotHandoffStrategy, SubscriptionId, TerminalScreenSize,
    TerminalSnapshotPayload, DURABLE_SESSION_PROTOCOL_VERSION,
};

fn round_trip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialize contract value");
    serde_json::from_str(&json).expect("deserialize contract value")
}

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("session-durable-1".to_string())
}

fn worker_id() -> SessionWorkerId {
    SessionWorkerId("worker-durable-1".to_string())
}

fn client_id() -> ClientId {
    ClientId("client-durable-1".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-durable-1".to_string())
}

fn protocol() -> DurableSessionProtocolVersion {
    DurableSessionProtocolVersion {
        current: DURABLE_SESSION_PROTOCOL_VERSION,
        minimum_compatible: DURABLE_SESSION_PROTOCOL_VERSION,
        implementation: "botster-core-test".to_string(),
        capabilities: vec![
            SessionWorkerCapability::SnapshotHandoff,
            SessionWorkerCapability::ModeFlags,
            SessionWorkerCapability::DaemonRestartAdoption,
            SessionWorkerCapability::Heartbeat,
            SessionWorkerCapability::GuardedSessionWrites,
        ],
    }
}

fn worker_identity() -> SessionWorkerIdentity {
    SessionWorkerIdentity {
        worker_id: worker_id(),
        session_id: session_id(),
        process: SessionWorkerProcessIdentity {
            pid: Some(4242),
            process_generation: "process-generation-1".to_string(),
            born_at: 10,
        },
        protocol: protocol(),
        adoption_generation: 2,
    }
}

fn healthy_worker() -> SessionWorkerHealth {
    SessionWorkerHealth::Healthy {
        last_heartbeat_at: 20,
    }
}

fn readiness() -> SessionReadinessEvidence {
    SessionReadinessEvidence {
        session_id: session_id(),
        observed_at: 30,
        mode_flags: Some(ModeFlags {
            cursor_visible: false,
            ..ModeFlags::default()
        }),
        screen_text: Some("waiting for input".to_string()),
        waiting_for_answer: true,
        unsafe_to_interrupt: true,
        snapshot_pending: false,
        worker_health: healthy_worker(),
        activity: SessionActivity {
            last_input_at: Some(22),
            last_output_at: Some(24),
            last_declared_activity_at: None,
        },
        semantic_hints: vec!["agent_waiting_for_answer".to_string()],
    }
}

fn pressure() -> BackpressureSummary {
    BackpressureSummary {
        source: QueueSource::SessionIo,
        capacity: 512,
        depth: 511,
        route: BackpressureRoute {
            session_id: Some(session_id()),
            client_id: Some(client_id()),
            subscription_id: Some(subscription_id()),
            plugin_key: None,
        },
    }
}

#[test]
fn worker_protocol_version_metadata_round_trips() {
    let version = protocol();
    let older = DurableSessionProtocolVersion {
        current: 1,
        minimum_compatible: 1,
        implementation: "worker-test".to_string(),
        capabilities: vec![SessionWorkerCapability::Heartbeat],
    };

    assert_eq!(version, round_trip(&version));
    assert!(version.is_compatible_with(&older));
    assert!(version
        .capabilities
        .contains(&SessionWorkerCapability::DaemonRestartAdoption));
}

#[test]
fn worker_identity_binds_session_process_and_generation() {
    let identity = worker_identity();

    assert_eq!(identity, round_trip(&identity));
    assert_eq!(identity.session_id, session_id());
    assert_eq!(identity.process.pid, Some(4242));
    assert_eq!(identity.adoption_generation, 2);
}

#[test]
fn spawn_and_adopt_contracts_preserve_session_identity() {
    let spawn = SessionWorkerSpawnRequest {
        request_id: request_id("spawn-1"),
        session_id: session_id(),
        host_spawn_ref: "synthetic-spawn-ref".to_string(),
        initial_size: Some(TerminalScreenSize::new(40, 120)),
        metadata: CoreSessionMetadata::new(),
        protocol: protocol(),
    };
    let spawned = SessionWorkerSpawned {
        request_id: spawn.request_id.clone(),
        identity: worker_identity(),
    };
    let adopt = SessionWorkerAdoptRequest {
        request_id: request_id("adopt-1"),
        identity: spawned.identity.clone(),
        expected_session_id: session_id(),
        daemon_protocol: protocol(),
        last_heartbeat_at: Some(20),
    };
    let verdict = SessionWorkerAdoptionVerdict::Adopted {
        identity: spawned.identity.clone(),
    };

    assert_eq!(spawn, round_trip(&spawn));
    assert_eq!(spawned, round_trip(&spawned));
    assert_eq!(adopt, round_trip(&adopt));
    assert_eq!(verdict, round_trip(&verdict));
    assert_eq!(adopt.identity.session_id, adopt.expected_session_id);
}

#[test]
fn attach_and_detach_contracts_use_client_subscription_identity() {
    let attach = SessionWorkerAttachRequest {
        request_id: request_id("attach-1"),
        client_id: client_id(),
        session_id: session_id(),
        subscription_id: subscription_id(),
        size: TerminalScreenSize::new(24, 80),
        snapshot_strategy: SnapshotHandoffStrategy::SnapshotBeforeLiveOutput,
    };
    let detach = SessionWorkerDetached {
        session_id: session_id(),
        subscription_id: subscription_id(),
        worker_retained: true,
    };

    assert_eq!(attach, round_trip(&attach));
    assert_eq!(detach, round_trip(&detach));
    assert_eq!(
        attach.snapshot_strategy,
        SnapshotHandoffStrategy::SnapshotBeforeLiveOutput
    );
}

#[test]
fn heartbeat_and_health_distinguish_alive_unhealthy_and_stale() {
    let heartbeat = SessionWorkerHeartbeat {
        identity: worker_identity(),
        at: 40,
        lifecycle: SessionLifecycleState::Running,
        activity: SessionActivity::default(),
        pressure: vec![pressure()],
    };
    let unhealthy = SessionWorkerHealth::Unhealthy {
        last_heartbeat_at: 35,
        reason: SessionWorkerHealthReason::Backpressured,
    };
    let stale = SessionWorkerHealth::Stale {
        reason: SessionWorkerStaleReason::HeartbeatExpired,
    };

    assert_eq!(heartbeat, round_trip(&heartbeat));
    assert_eq!(healthy_worker(), round_trip(&healthy_worker()));
    assert_eq!(unhealthy, round_trip(&unhealthy));
    assert_eq!(stale, round_trip(&stale));
}

#[test]
fn restart_semantics_matrix_names_hub_core_and_worker_survival() {
    let semantics = DurableRestartSemantics::durable_worker_contract();

    assert_eq!(semantics, round_trip(&semantics));
    assert_eq!(
        semantics.survival_for(RestartBoundary::HubRestart),
        RestartSurvival::Survives
    );
    assert_eq!(
        semantics.survival_for(RestartBoundary::CoreDaemonRestartAdopted),
        RestartSurvival::Survives
    );
    assert_eq!(
        semantics.survival_for(RestartBoundary::CoreDaemonRestartAdoptionFailed),
        RestartSurvival::SurvivesDegraded
    );
    assert_eq!(
        semantics.survival_for(RestartBoundary::SessionWorkerDeath),
        RestartSurvival::Dies
    );
}

#[test]
fn guarded_pty_input_contract_accepts_without_claiming_delivery() {
    let write = GuardedSessionWriteRequest {
        request_id: request_id("write-1"),
        session_id: session_id(),
        primitive: GuardedSessionWritePrimitive::PtyInput {
            data: b"yes\n".to_vec(),
        },
        readiness: readiness(),
        host_policy: GuardedSessionWritePolicy::Allow,
        defer_until: None,
    };
    let accepted = GuardedSessionWriteState::Accepted {
        request_id: write.request_id.clone(),
    };

    assert_eq!(write, round_trip(&write));
    assert_eq!(accepted, round_trip(&accepted));
    assert_ne!(
        accepted,
        GuardedSessionWriteState::Written {
            request_id: request_id("write-1"),
            written_at: 31,
        }
    );
}

#[test]
fn guarded_annotation_contract_represents_deferred_rejected_written_and_acknowledged() {
    let deferred = GuardedSessionWriteState::Deferred {
        request_id: request_id("write-defer"),
        reason: GuardedSessionWriteDeferralReason::WaitingForAnswer,
    };
    let rejected = GuardedSessionWriteState::Rejected {
        request_id: request_id("write-reject"),
        reason: GuardedSessionWriteRejectionReason::HostPolicyRejected,
    };
    let queued = GuardedSessionWriteState::Queued {
        request_id: request_id("write-queued"),
        pressure: Some(pressure()),
    };
    let written = GuardedSessionWriteState::Written {
        request_id: request_id("write-written"),
        written_at: 50,
    };
    let acknowledged = GuardedSessionWriteState::Acknowledged {
        request_id: request_id("write-ack"),
        acknowledged_at: 55,
        notification_status: Some(NotificationDeliveryStatus::Acknowledged),
    };

    for state in [&deferred, &rejected, &queued, &written, &acknowledged] {
        assert_eq!(*state, round_trip(state));
    }
}

#[test]
fn readiness_evidence_contract_covers_answer_wait_cursor_and_prompt_safety() {
    let evidence = readiness();

    assert_eq!(evidence, round_trip(&evidence));
    assert!(evidence.waiting_for_answer);
    assert!(evidence.unsafe_to_interrupt);
    assert_eq!(
        evidence
            .mode_flags
            .as_ref()
            .map(|flags| flags.cursor_visible),
        Some(false)
    );
    assert!(!evidence.snapshot_pending);
}

#[test]
fn bounded_output_contract_preserves_pressure_and_slow_consumer_semantics() {
    let limits = SessionWorkerQueueLimits {
        source: QueueSource::SessionIo,
        output_frame_capacity: 16,
        output_byte_capacity: 32 * 1024,
        pressure: vec![pressure()],
        lag: vec![DeliveryLag {
            source: QueueSource::SessionIo,
            capacity: 512,
            depth: 16,
            route: BackpressureRoute::queue_only(),
        }],
        slow_consumer: SlowConsumerBehavior::PreserveOrderAndBackpressure,
    };

    assert_eq!(limits, round_trip(&limits));
    assert_eq!(limits.source, QueueSource::SessionIo);
    assert_eq!(
        limits.slow_consumer,
        SlowConsumerBehavior::PreserveOrderAndBackpressure
    );
}

#[test]
fn daemon_control_api_is_typed_not_cli_output() {
    let command = DaemonControlOperation::AttachStream {
        request_id: request_id("daemon-attach"),
        session_id: session_id(),
        client_id: client_id(),
        subscription_id: subscription_id(),
    };
    let outcome = DaemonControlOutcome::Health {
        request_id: request_id("daemon-health"),
        workers: vec![healthy_worker()],
    };
    let cli = DaemonCliOperation::SessionList;

    assert_eq!(command, round_trip(&command));
    assert_eq!(outcome, round_trip(&outcome));
    assert_eq!(cli, round_trip(&cli));
}

#[test]
fn worker_protocol_output_shutdown_and_failure_shapes_round_trip() {
    let output = SessionWorkerOutputFrame::Snapshot {
        session_id: session_id(),
        snapshot: TerminalSnapshotPayload::new(
            b"snapshot".to_vec(),
            TerminalScreenSize::new(24, 80),
            Some("synthetic".to_string()),
        ),
    };
    let shutdown = SessionWorkerShutdownRequest {
        request_id: request_id("shutdown-1"),
        session_id: session_id(),
        mode: SessionWorkerShutdownMode::Graceful,
        reason: "test shutdown".to_string(),
    };
    let failure = botster_core::SessionWorkerFailure {
        identity: Some(worker_identity()),
        session_id: session_id(),
        reason: SessionWorkerStaleReason::WorkerDied,
        durability: RestartSurvival::Dies,
    };

    assert_eq!(output, round_trip(&output));
    assert_eq!(shutdown, round_trip(&shutdown));
    assert_eq!(failure, round_trip(&failure));
}

#[test]
fn session_worker_protocol_examples_exclude_pii() {
    let encoded = serde_json::to_string(&vec![
        serde_json::to_value(worker_identity()).expect("identity json"),
        serde_json::to_value(readiness()).expect("readiness json"),
        serde_json::to_value(SessionWorkerSpawnRequest {
            request_id: request_id("spawn-pii-check"),
            session_id: session_id(),
            host_spawn_ref: "synthetic-spawn-ref".to_string(),
            initial_size: None,
            metadata: CoreSessionMetadata::new(),
            protocol: protocol(),
        })
        .expect("spawn json"),
        serde_json::to_value(GuardedSessionWritePrimitive::SessionNotification {
            notification_id: NotificationId("notification-durable-1".to_string()),
            body: "synthetic notice".to_string(),
        })
        .expect("notification json"),
    ])
    .expect("json");

    for forbidden in ["/Users/", "jason", "Rails", "Claude", "Codex", "customer"] {
        assert!(
            !encoded.contains(forbidden),
            "durable contract fixture contained forbidden example text: {forbidden}"
        );
    }
}
