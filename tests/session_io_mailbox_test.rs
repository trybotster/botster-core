//! Session I/O mailbox semantic acceptance tests.

use std::time::Duration;

use botster_core::client::ClientId;
use botster_core::{
    BackpressureRoute, InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady,
    InitialSnapshotRequest, MailboxSendFailure, MailboxSendFailureReason, ModeFlags,
    ModeFlagsReady, NotificationPayload, PasteFileErrorReason, PasteFileFailed, PasteFileRequest,
    PasteFileWritten, PreparedSnapshotReady, PreparedSnapshotRequest, ProcessExitedPayload,
    PromptMarkPayload, QueueSource, RequestId, ScreenReady, SessionId, SessionIoCoalescingPolicy,
    SessionIoEvent, SessionIoOrderedEvent, SessionIoRequest, SubscriptionId, TerminalColorProfile,
    SESSION_IO_MAX_COALESCED_BYTES, SESSION_IO_MAX_COALESCED_FRAMES,
    SESSION_IO_MAX_COALESCED_WINDOW,
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

struct FakeSessionIoHarness {
    capacity: usize,
    closed: bool,
    requests: Vec<SessionIoRequest>,
    events: Vec<SessionIoEvent>,
    pending_output: Vec<Vec<u8>>,
}

impl FakeSessionIoHarness {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            closed: false,
            requests: Vec::new(),
            events: Vec::new(),
            pending_output: Vec::new(),
        }
    }

    fn send(&mut self, request: SessionIoRequest) -> Result<(), MailboxSendFailure> {
        if self.closed {
            return Err(MailboxSendFailure {
                source: QueueSource::SessionIo,
                route: route(),
                reason: MailboxSendFailureReason::QueueClosed,
            });
        }

        if self.requests.len() >= self.capacity {
            return Err(MailboxSendFailure {
                source: QueueSource::SessionIo,
                route: route(),
                reason: MailboxSendFailureReason::QueueFull,
            });
        }

        self.requests.push(request);
        Ok(())
    }

    fn handle(&mut self, request: SessionIoRequest) {
        match request {
            SessionIoRequest::GetSnapshot {
                request_id,
                session_id,
            } => self
                .events
                .push(SessionIoEvent::SnapshotReady(botster_core::SnapshotReady {
                    request_id,
                    session_id,
                    data: b"snapshot".to_vec(),
                    rows: 24,
                    cols: 80,
                })),
            SessionIoRequest::PasteFile(request) => {
                self.events
                    .push(SessionIoEvent::PasteFileWritten(PasteFileWritten {
                        request_id: request.request_id,
                        session_id: request.session_id,
                        bytes: request.data.len(),
                        storage_ref: Some("opaque-paste-1".to_string()),
                    }));
            }
            SessionIoRequest::PrepareSnapshot(request) => {
                self.events.push(SessionIoEvent::PreparedSnapshotReady(
                    PreparedSnapshotReady {
                        request_id: request.request_id,
                        session_id: request.session_id,
                        uncompressed_len: request.snapshot.len(),
                        payload: request.snapshot,
                        recovery: request.recovery,
                    },
                ));
            }
            SessionIoRequest::GetModeFlags {
                request_id,
                session_id,
            } => self
                .events
                .push(SessionIoEvent::ModeFlagsReady(ModeFlagsReady {
                    request_id,
                    session_id,
                    mode_flags: ModeFlags {
                        cursor_visible: true,
                        ..ModeFlags::default()
                    },
                })),
            SessionIoRequest::GetScreen {
                request_id,
                session_id,
            } => self.events.push(SessionIoEvent::ScreenReady(ScreenReady {
                request_id,
                session_id,
                text: "screen".to_string(),
            })),
            SessionIoRequest::Shutdown { session_id, reason } => self
                .events
                .push(SessionIoEvent::Shutdown { session_id, reason }),
            _ => {}
        }
    }

    fn push_output_before_ordered_event(
        &mut self,
        output: Vec<u8>,
        ordered_event: SessionIoOrderedEvent,
    ) {
        self.pending_output.push(output);
        if ordered_event.requires_output_flush() {
            self.events
                .extend(
                    self.pending_output
                        .drain(..)
                        .map(|data| SessionIoEvent::TerminalBytes {
                            session_id: session_id(),
                            data,
                        }),
                );
        }
    }
}

#[test]
fn session_io_harness_routes_pty_input_resize_color_and_shutdown() {
    let mut harness = FakeSessionIoHarness::new(8);

    harness
        .send(SessionIoRequest::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        })
        .expect("input request queues");
    harness
        .send(SessionIoRequest::Resize {
            session_id: session_id(),
            rows: 40,
            cols: 120,
        })
        .expect("resize request queues");
    harness
        .send(SessionIoRequest::SetColorProfile {
            session_id: session_id(),
            color_profile: TerminalColorProfile::default(),
        })
        .expect("color profile request queues");
    harness
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "test".to_string(),
        })
        .expect("shutdown request queues");

    assert!(matches!(
        harness.requests[0],
        SessionIoRequest::PtyInput { .. }
    ));
    assert!(matches!(
        harness.requests[1],
        SessionIoRequest::Resize { .. }
    ));
    assert!(matches!(
        harness.requests[2],
        SessionIoRequest::SetColorProfile { .. }
    ));
    assert!(matches!(
        harness.requests[3],
        SessionIoRequest::Shutdown { .. }
    ));
}

#[test]
fn snapshot_request_routes_result_by_request_id() {
    let mut harness = FakeSessionIoHarness::new(1);
    harness.handle(SessionIoRequest::GetSnapshot {
        request_id: request_id("snapshot-1"),
        session_id: session_id(),
    });

    assert_eq!(
        harness.events,
        vec![SessionIoEvent::SnapshotReady(botster_core::SnapshotReady {
            request_id: request_id("snapshot-1"),
            session_id: session_id(),
            data: b"snapshot".to_vec(),
            rows: 24,
            cols: 80,
        })]
    );
}

#[test]
fn initial_snapshot_barrier_delivers_snapshot_before_live_output() {
    let mut barrier = InitialSnapshotBarrier::new();

    assert_eq!(barrier.phase(), InitialSnapshotPhase::WaitingForSnapshot);
    assert_eq!(barrier.push_live_output(b"live-1".to_vec()), None);
    assert_eq!(barrier.push_live_output(b"live-2".to_vec()), None);

    let delivered = barrier.deliver_initial_snapshot(InitialSnapshotReady {
        request_id: request_id("initial-1"),
        session_id: session_id(),
        client_id: client_id(),
        subscription_id: subscription_id(),
        snapshot: b"initial".to_vec(),
        rows: 24,
        cols: 80,
    });

    assert_eq!(barrier.phase(), InitialSnapshotPhase::LiveOutputActive);
    assert!(matches!(
        delivered[0],
        SessionIoEvent::InitialSnapshotReady(_)
    ));
    assert_eq!(
        delivered[1],
        SessionIoEvent::TerminalBytes {
            session_id: session_id(),
            data: b"live-1".to_vec()
        }
    );
    assert_eq!(
        delivered[2],
        SessionIoEvent::TerminalBytes {
            session_id: session_id(),
            data: b"live-2".to_vec()
        }
    );
    assert_eq!(
        barrier.push_live_output(b"live-3".to_vec()),
        Some(b"live-3".to_vec())
    );
}

#[test]
fn paste_file_request_reports_written_and_failed_results() {
    let request = PasteFileRequest {
        request_id: request_id("paste-1"),
        session_id: session_id(),
        filename: "paste.txt".to_string(),
        data: b"paste".to_vec(),
    };
    let mut harness = FakeSessionIoHarness::new(1);

    harness.handle(SessionIoRequest::PasteFile(request));

    assert_eq!(
        harness.events[0],
        SessionIoEvent::PasteFileWritten(PasteFileWritten {
            request_id: request_id("paste-1"),
            session_id: session_id(),
            bytes: 5,
            storage_ref: Some("opaque-paste-1".to_string()),
        })
    );
    assert_eq!(
        SessionIoEvent::PasteFileFailed(PasteFileFailed {
            request_id: request_id("paste-2"),
            session_id: session_id(),
            reason: PasteFileErrorReason::StorageUnavailable,
            detail: Some("runtime unavailable".to_string()),
        }),
        serde_json::from_str(
            &serde_json::to_string(&SessionIoEvent::PasteFileFailed(PasteFileFailed {
                request_id: request_id("paste-2"),
                session_id: session_id(),
                reason: PasteFileErrorReason::StorageUnavailable,
                detail: Some("runtime unavailable".to_string()),
            }))
            .expect("serialize paste failure")
        )
        .expect("deserialize paste failure")
    );
}

#[test]
fn prepared_snapshot_request_reports_payload_metadata() {
    let mut harness = FakeSessionIoHarness::new(1);

    harness.handle(SessionIoRequest::PrepareSnapshot(PreparedSnapshotRequest {
        request_id: request_id("prepared-1"),
        session_id: session_id(),
        snapshot: b"prepared".to_vec(),
        recovery: true,
    }));

    assert_eq!(
        harness.events,
        vec![SessionIoEvent::PreparedSnapshotReady(
            PreparedSnapshotReady {
                request_id: request_id("prepared-1"),
                session_id: session_id(),
                uncompressed_len: 8,
                payload: b"prepared".to_vec(),
                recovery: true,
            }
        )]
    );
}

#[test]
fn mode_and_screen_requests_round_trip_typed_results() {
    let mut harness = FakeSessionIoHarness::new(2);

    harness.handle(SessionIoRequest::GetModeFlags {
        request_id: request_id("mode-1"),
        session_id: session_id(),
    });
    harness.handle(SessionIoRequest::GetScreen {
        request_id: request_id("screen-1"),
        session_id: session_id(),
    });

    assert!(matches!(
        harness.events[0],
        SessionIoEvent::ModeFlagsReady(_)
    ));
    assert!(matches!(harness.events[1], SessionIoEvent::ScreenReady(_)));
}

#[test]
fn mailbox_send_failures_distinguish_full_from_closed() {
    let mut full = FakeSessionIoHarness::new(0);
    let failure = full
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "full".to_string(),
        })
        .expect_err("queue is full");

    assert_eq!(failure.reason, MailboxSendFailureReason::QueueFull);
    assert_eq!(failure.source, QueueSource::SessionIo);

    let mut closed = FakeSessionIoHarness::new(1);
    closed.closed = true;
    let failure = closed
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "closed".to_string(),
        })
        .expect_err("queue is closed");

    assert_eq!(failure.reason, MailboxSendFailureReason::QueueClosed);
}

#[test]
fn coalescing_policy_flushes_at_sixteen_frames_32k_and_four_ms() {
    let policy = SessionIoCoalescingPolicy::default();

    assert!(policy.should_flush_output(SESSION_IO_MAX_COALESCED_BYTES, 1, Duration::ZERO));
    assert!(policy.should_flush_output(1, SESSION_IO_MAX_COALESCED_FRAMES, Duration::ZERO));
    assert!(policy.should_flush_output(1, 1, SESSION_IO_MAX_COALESCED_WINDOW));
    assert!(policy.metadata_age_expired(SESSION_IO_MAX_COALESCED_WINDOW));
    assert!(!policy.should_flush_output(
        SESSION_IO_MAX_COALESCED_BYTES - 1,
        SESSION_IO_MAX_COALESCED_FRAMES - 1,
        SESSION_IO_MAX_COALESCED_WINDOW - Duration::from_millis(1),
    ));
}

#[test]
fn ordered_flush_helper_flushes_before_prompt_bell_notification_and_process_exit() {
    for event in [
        SessionIoOrderedEvent::PromptMark,
        SessionIoOrderedEvent::Bell,
        SessionIoOrderedEvent::Notification,
        SessionIoOrderedEvent::ProcessExited,
        SessionIoOrderedEvent::Eof,
        SessionIoOrderedEvent::Desynchronized,
        SessionIoOrderedEvent::Shutdown,
    ] {
        assert!(event.requires_output_flush(), "{event:?}");
    }

    let mut harness = FakeSessionIoHarness::new(1);
    harness.push_output_before_ordered_event(
        b"pending".to_vec(),
        SessionIoOrderedEvent::ProcessExited,
    );
    harness.events.push(SessionIoEvent::ProcessExited {
        session_id: session_id(),
        payload: ProcessExitedPayload {
            exit_code: Some(0),
            signal: None,
        },
    });

    assert!(matches!(
        harness.events[0],
        SessionIoEvent::TerminalBytes { .. }
    ));
    assert!(matches!(
        harness.events[1],
        SessionIoEvent::ProcessExited { .. }
    ));
}

#[test]
fn metadata_ordered_events_are_typed() {
    let prompt = SessionIoEvent::PromptMark {
        session_id: session_id(),
        payload: PromptMarkPayload {
            mark: "prompt".to_string(),
        },
    };
    let bell = SessionIoEvent::Bell {
        session_id: session_id(),
    };
    let notification = SessionIoEvent::Notification {
        session_id: session_id(),
        payload: NotificationPayload {
            title: "title".to_string(),
            body: "body".to_string(),
        },
    };

    assert!(matches!(prompt, SessionIoEvent::PromptMark { .. }));
    assert!(matches!(bell, SessionIoEvent::Bell { .. }));
    assert!(matches!(notification, SessionIoEvent::Notification { .. }));
}

#[test]
fn initial_snapshot_request_shape_includes_attach_routing() {
    let request = SessionIoRequest::GetInitialSnapshot(InitialSnapshotRequest {
        request_id: request_id("initial-1"),
        session_id: session_id(),
        client_id: client_id(),
        subscription_id: subscription_id(),
        rows: 24,
        cols: 80,
    });

    assert!(matches!(
        request,
        SessionIoRequest::GetInitialSnapshot(InitialSnapshotRequest {
            rows: 24,
            cols: 80,
            ..
        })
    ));
}

#[test]
fn session_io_mailbox_contract_excludes_hub_recovery_and_authorization_policy() {
    let actor_source = std::fs::read_to_string("src/actor.rs").expect("read actor source");

    for forbidden in [
        "HubRecovery",
        "AuthorizationPolicy",
        "WebRtc",
        "ActionCable",
        "PtyForwarder",
        "snapshot_and_subscribe",
    ] {
        assert!(
            !actor_source.contains(forbidden),
            "session I/O mailbox core contract must not mention {forbidden}"
        );
    }
}
