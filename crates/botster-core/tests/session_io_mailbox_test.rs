//! Session I/O mailbox semantic acceptance tests.

use std::time::Duration;

use botster_core::client::ClientId;
use botster_core::{
    BackpressureRoute, InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady,
    InitialSnapshotRequest, MailboxSendFailureReason, NotificationPayload, ProcessExitedPayload,
    PromptMarkPayload, QueueSource, RequestId, SendFileErrorReason, SendFileFailed, SessionId,
    SessionIoCoalescingPolicy, SessionIoEvent, SessionIoOrderedEvent, SessionIoRequest,
    SubscriptionId, TerminalColorProfile, SESSION_IO_MAX_COALESCED_BYTES,
    SESSION_IO_MAX_COALESCED_FRAMES, SESSION_IO_MAX_COALESCED_WINDOW,
};
use botster_core_test_support::fake::FakeSessionIoMailbox;

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

struct OrderedFlushHarness {
    events: Vec<SessionIoEvent>,
    pending_output: Vec<Vec<u8>>,
}

impl OrderedFlushHarness {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            pending_output: Vec::new(),
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
fn session_io_mailbox_queues_pty_input_resize_color_and_shutdown() {
    let mut mailbox = FakeSessionIoMailbox::new(8, route());

    mailbox
        .send(SessionIoRequest::PtyInput {
            session_id: session_id(),
            data: b"ls\n".to_vec(),
        })
        .expect("input request queues");
    mailbox
        .send(SessionIoRequest::Resize {
            session_id: session_id(),
            rows: 40,
            cols: 120,
        })
        .expect("resize request queues");
    mailbox
        .send(SessionIoRequest::SetColorProfile {
            session_id: session_id(),
            color_profile: TerminalColorProfile::default(),
        })
        .expect("color profile request queues");
    mailbox
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "test".to_string(),
        })
        .expect("shutdown request queues");

    assert!(matches!(
        mailbox.requests()[0],
        SessionIoRequest::PtyInput { .. }
    ));
    assert!(matches!(
        mailbox.requests()[1],
        SessionIoRequest::Resize { .. }
    ));
    assert!(matches!(
        mailbox.requests()[2],
        SessionIoRequest::SetColorProfile { .. }
    ));
    assert!(matches!(
        mailbox.requests()[3],
        SessionIoRequest::Shutdown { .. }
    ));
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
fn send_file_failed_result_round_trips() {
    assert_eq!(
        SessionIoEvent::SendFileFailed(SendFileFailed {
            request_id: request_id("send-file-2"),
            session_id: session_id(),
            reason: SendFileErrorReason::StorageUnavailable,
            detail: Some("runtime unavailable".to_string()),
        }),
        serde_json::from_str(
            &serde_json::to_string(&SessionIoEvent::SendFileFailed(SendFileFailed {
                request_id: request_id("send-file-2"),
                session_id: session_id(),
                reason: SendFileErrorReason::StorageUnavailable,
                detail: Some("runtime unavailable".to_string()),
            }))
            .expect("serialize send-file failure")
        )
        .expect("deserialize send-file failure")
    );
}

#[test]
fn mailbox_send_failures_distinguish_full_from_closed() {
    let mut full = FakeSessionIoMailbox::new(0, route());
    let failure = full
        .send(SessionIoRequest::Shutdown {
            session_id: session_id(),
            reason: "full".to_string(),
        })
        .expect_err("queue is full");

    assert_eq!(failure.reason, MailboxSendFailureReason::QueueFull);
    assert_eq!(failure.source, QueueSource::SessionIo);

    let mut closed = FakeSessionIoMailbox::new(1, route());
    closed.close();
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

    let mut harness = OrderedFlushHarness::new();
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
    let title = SessionIoEvent::TitleChanged {
        session_id: session_id(),
        title: "Build".to_string(),
    };
    let cwd = SessionIoEvent::CwdChanged {
        session_id: session_id(),
        cwd: "/work/repo".to_string(),
    };
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

    assert!(matches!(title, SessionIoEvent::TitleChanged { .. }));
    assert!(matches!(cwd, SessionIoEvent::CwdChanged { .. }));
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
    let actor_source = std::fs::read_to_string("src/contract/actor.rs").expect("read actor source");
    let session_io_source = actor_source
        .split("/// Stable plugin identity.")
        .next()
        .expect("session I/O contract slice");

    for forbidden in [
        "HubRecovery",
        "AuthorizationPolicy",
        "WebRtc",
        "ActionCable",
        "PtyForwarder",
        "snapshot_and_subscribe",
    ] {
        assert!(
            !session_io_source.contains(forbidden),
            "session I/O mailbox core contract must not mention {forbidden}"
        );
    }
}
