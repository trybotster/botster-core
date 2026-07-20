//! Session worker engine behavior tests.

use botster_core::client::ClientId;
use botster_core::{
    BackpressureRoute, InitialSnapshotReady, InitialSnapshotRequest, MailboxSendFailureReason,
    ModeFlags, NotificationPayload, PreparedSnapshotRequest, ProcessExitedPayload,
    PromptMarkPayload, QueueSource, RequestId, SendFileRequest, SessionId, SessionIoEvent,
    SessionIoRequest, SessionWorkerEngine, SessionWorkerRuntimeEvent, SubscriptionId,
};
use botster_core_test_support::fake::{
    FakeSessionIoMailbox, FakeSessionWorkerRuntime, RuntimeCommand,
};

fn request_id(value: &str) -> RequestId {
    RequestId(value.to_string())
}

fn session_id() -> SessionId {
    SessionId("session-1".to_string())
}

fn client_id() -> ClientId {
    ClientId("client-1".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("sub-1".to_string())
}

fn route() -> BackpressureRoute {
    BackpressureRoute {
        session_id: Some(session_id()),
        client_id: Some(client_id()),
        subscription_id: Some(subscription_id()),
        plugin_key: None,
    }
}

fn engine() -> SessionWorkerEngine<FakeSessionWorkerRuntime> {
    SessionWorkerEngine::new(FakeSessionWorkerRuntime::new())
}

#[test]
fn session_worker_routes_input_writes() {
    let mut engine = engine();

    let outcome = engine
        .handle_request(SessionIoRequest::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        })
        .expect("input request succeeds");

    assert!(outcome.events.is_empty());
    assert_eq!(
        engine.runtime().commands(),
        &vec![RuntimeCommand::WriteInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        }]
    );
}

#[test]
fn session_worker_routes_resize_before_snapshot() {
    let mut engine = engine();

    engine
        .handle_request(SessionIoRequest::Resize {
            session_id: session_id(),
            rows: 40,
            cols: 120,
        })
        .expect("resize request succeeds");
    let outcome = engine
        .handle_request(SessionIoRequest::GetSnapshot {
            request_id: request_id("snapshot-1"),
            session_id: session_id(),
        })
        .expect("snapshot request succeeds");

    assert_eq!(
        engine.runtime().commands()[0],
        RuntimeCommand::Resize {
            session_id: session_id(),
            rows: 40,
            cols: 120,
        }
    );
    assert_eq!(
        outcome.events,
        vec![SessionIoEvent::SnapshotReady(botster_core::SnapshotReady {
            request_id: request_id("snapshot-1"),
            session_id: session_id(),
            data: b"snapshot".to_vec(),
            rows: 40,
            cols: 120,
        })]
    );
}

#[test]
fn session_worker_routes_send_file_prepare_mode_and_screen_requests() {
    let mut engine =
        SessionWorkerEngine::new(FakeSessionWorkerRuntime::new().with_mode_flags(ModeFlags {
            cursor_visible: true,
            ..ModeFlags::default()
        }));

    let send_file = engine
        .handle_request(SessionIoRequest::SendFile(SendFileRequest {
            request_id: request_id("send-file-1"),
            session_id: session_id(),
            filename: "send-file.txt".to_string(),
            data: b"send-file".to_vec(),
        }))
        .expect("send file request succeeds");
    let prepared = engine
        .handle_request(SessionIoRequest::PrepareSnapshot(PreparedSnapshotRequest {
            request_id: request_id("prepared-1"),
            session_id: session_id(),
            snapshot: b"prepared".to_vec(),
            recovery: true,
        }))
        .expect("prepare snapshot request succeeds");
    let mode = engine
        .handle_request(SessionIoRequest::GetModeFlags {
            request_id: request_id("mode-1"),
            session_id: session_id(),
        })
        .expect("mode flags request succeeds");
    let screen = engine
        .handle_request(SessionIoRequest::GetScreen {
            request_id: request_id("screen-1"),
            session_id: session_id(),
        })
        .expect("screen request succeeds");

    assert!(matches!(
        send_file.events[0],
        SessionIoEvent::SendFileWritten(_)
    ));
    assert!(matches!(
        prepared.events[0],
        SessionIoEvent::PreparedSnapshotReady(_)
    ));
    assert!(matches!(mode.events[0], SessionIoEvent::ModeFlagsReady(_)));
    assert!(matches!(screen.events[0], SessionIoEvent::ScreenReady(_)));
}

#[test]
fn initial_snapshot_precedes_live_output_through_engine() {
    let mut engine = engine();

    engine
        .handle_request(SessionIoRequest::GetInitialSnapshot(
            InitialSnapshotRequest {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                rows: 30,
                cols: 100,
            },
        ))
        .expect("initial snapshot request succeeds");
    let live = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"live-1".to_vec(),
        last_output_at: 10,
    });
    let initial = engine.handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
        InitialSnapshotReady {
            request_id: request_id("initial-1"),
            session_id: session_id(),
            client_id: client_id(),
            subscription_id: subscription_id(),
            snapshot: b"initial".to_vec(),
            rows: 30,
            cols: 100,
        },
    ));

    assert!(live.events.is_empty());
    assert_eq!(
        initial.events,
        vec![
            SessionIoEvent::InitialSnapshotReady(InitialSnapshotReady {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                snapshot: b"initial".to_vec(),
                rows: 30,
                cols: 100,
            }),
            SessionIoEvent::TerminalBytes {
                session_id: session_id(),
                data: b"live-1".to_vec(),
            },
        ]
    );
}

#[test]
fn runtime_metadata_events_become_session_io_events() {
    let mut engine = engine();

    assert!(matches!(
        engine
            .handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
                session_id: session_id(),
                data: b"live".to_vec(),
                last_output_at: 10,
            })
            .events[0],
        SessionIoEvent::TerminalBytes { .. }
    ));
    let title = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TitleChanged {
        session_id: session_id(),
        title: "Build".to_string(),
    });

    assert!(matches!(
        title.events[0],
        SessionIoEvent::TitleChanged { .. }
    ));

    assert!(matches!(
        engine
            .handle_runtime_event(SessionWorkerRuntimeEvent::CwdChanged {
                session_id: session_id(),
                cwd: "/work/repo".to_string(),
            })
            .events[0],
        SessionIoEvent::CwdChanged { .. }
    ));
    assert!(matches!(
        engine
            .handle_runtime_event(SessionWorkerRuntimeEvent::PromptMark {
                session_id: session_id(),
                payload: PromptMarkPayload {
                    mark: "A".to_string(),
                },
            })
            .events[0],
        SessionIoEvent::PromptMark { .. }
    ));
    assert!(matches!(
        engine
            .handle_runtime_event(SessionWorkerRuntimeEvent::Bell {
                session_id: session_id(),
            })
            .events[0],
        SessionIoEvent::Bell { .. }
    ));
    assert!(matches!(
        engine
            .handle_runtime_event(SessionWorkerRuntimeEvent::Notification {
                session_id: session_id(),
                payload: NotificationPayload {
                    title: "Notice".to_string(),
                    body: "Body".to_string(),
                },
            })
            .events[0],
        SessionIoEvent::Notification { .. }
    ));
}

#[test]
fn steady_state_live_output_emits_after_initial_snapshot() {
    let mut engine = engine();

    engine
        .handle_request(SessionIoRequest::GetInitialSnapshot(
            InitialSnapshotRequest {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                rows: 30,
                cols: 100,
            },
        ))
        .expect("initial snapshot request succeeds");
    engine.handle_runtime_event(SessionWorkerRuntimeEvent::InitialSnapshotReady(
        InitialSnapshotReady {
            request_id: request_id("initial-1"),
            session_id: session_id(),
            client_id: client_id(),
            subscription_id: subscription_id(),
            snapshot: b"initial".to_vec(),
            rows: 30,
            cols: 100,
        },
    ));
    let outcome = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"steady".to_vec(),
        last_output_at: 20,
    });

    assert_eq!(
        outcome.events,
        vec![SessionIoEvent::TerminalBytes {
            session_id: session_id(),
            data: b"steady".to_vec(),
        }]
    );
    assert_eq!(outcome.last_output_at, Some(20));
}

#[test]
fn process_exit_follows_prior_live_output_event() {
    let mut engine = engine();

    let output = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"tail".to_vec(),
        last_output_at: 20,
    });
    let exit = engine.handle_runtime_event(SessionWorkerRuntimeEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });

    assert!(matches!(
        output.events[0],
        SessionIoEvent::TerminalBytes { .. }
    ));
    assert!(matches!(
        exit.events[0],
        SessionIoEvent::ProcessExited { .. }
    ));
}

#[test]
fn process_exit_flushes_pre_snapshot_output_before_exit_event() {
    let mut engine = engine();

    engine
        .handle_request(SessionIoRequest::GetInitialSnapshot(
            InitialSnapshotRequest {
                request_id: request_id("initial-1"),
                session_id: session_id(),
                client_id: client_id(),
                subscription_id: subscription_id(),
                rows: 30,
                cols: 100,
            },
        ))
        .expect("initial snapshot request succeeds");
    engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"pending".to_vec(),
        last_output_at: 20,
    });
    let exit = engine.handle_runtime_event(SessionWorkerRuntimeEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });

    assert!(matches!(
        exit.events[0],
        SessionIoEvent::TerminalBytes { .. }
    ));
    assert!(matches!(
        exit.events[1],
        SessionIoEvent::ProcessExited { .. }
    ));
}

#[test]
fn shutdown_waits_for_final_runtime_output_and_process_exit_before_closing() {
    let mut engine = engine();

    let shutdown = engine
        .handle_request(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "test".to_string(),
        })
        .expect("shutdown request succeeds");
    let later = engine
        .handle_request(SessionIoRequest::PtyInput {
            session_id: session_id(),
            data: b"ignored".to_vec(),
        })
        .expect("closed worker request succeeds");

    assert!(matches!(
        shutdown.events[0],
        SessionIoEvent::Shutdown { .. }
    ));
    assert!(!engine.is_closed());
    assert!(later.events.is_empty());
    assert_eq!(engine.runtime().commands().len(), 1);

    let final_output = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"final".to_vec(),
        last_output_at: 9,
    });
    assert!(matches!(
        final_output.events.as_slice(),
        [SessionIoEvent::TerminalBytes { data, .. }] if data == b"final"
    ));
    assert!(!engine.is_closed());

    let exit = engine.handle_runtime_event(SessionWorkerRuntimeEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });
    assert!(matches!(
        exit.events.as_slice(),
        [SessionIoEvent::ProcessExited { .. }]
    ));
    assert!(engine.is_closed());
}

#[test]
fn mailbox_failures_report_queue_full_and_closed() {
    let mut full = FakeSessionIoMailbox::new(0, route());
    let failure = full
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "full".to_string(),
        })
        .expect_err("queue is full");

    assert_eq!(failure.source, QueueSource::SessionIo);
    assert_eq!(failure.reason, MailboxSendFailureReason::QueueFull);

    let mut closed = FakeSessionIoMailbox::new(1, route());
    closed.close();
    let failure = closed
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "closed".to_string(),
        })
        .expect_err("queue is closed");

    assert_eq!(failure.source, QueueSource::SessionIo);
    assert_eq!(failure.reason, MailboxSendFailureReason::QueueClosed);
}

#[test]
fn live_output_updates_activity_timestamp() {
    let mut engine = engine();

    let outcome = engine.handle_runtime_event(SessionWorkerRuntimeEvent::TerminalBytes {
        session_id: session_id(),
        data: b"output".to_vec(),
        last_output_at: 1234,
    });

    assert_eq!(outcome.last_output_at, Some(1234));
    assert_eq!(engine.last_output_at(), Some(1234));
}

#[test]
fn engine_contract_excludes_concrete_host_policy() {
    let source = std::fs::read_to_string("src/engine/session_worker.rs")
        .expect("read session worker source");
    for forbidden in [
        "WebRTC",
        "browser",
        "TUI",
        "ActionCable",
        "Rails",
        "Authorization",
        "Retention",
        "RestartStrategy",
        "CloudSync",
        "ProductConfig",
    ] {
        assert!(
            !source.contains(forbidden),
            "session worker engine must not mention {forbidden}"
        );
    }
}
