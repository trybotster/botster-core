//! Conformance assertions for downstream consumers.

use botster_core::client::ClientId;
use botster_core::session::{RequestId, SessionId, SubscriptionId};
use botster_core::session_protocol::{encode_frame, FrameDecoder, FRAME_PTY_OUTPUT};
use botster_core::transport::TransportEgress;
use botster_core::{
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, SessionIoEvent,
    TerminalScreenEngine, TerminalScreenHook, TerminalScreenRuntime, TerminalScreenSize,
    TerminalScreenState, TerminalSnapshotPayload,
};

/// Assert that terminal output chunks round-trip through public core contracts.
///
/// This exercises the session protocol frame encoder/decoder and the
/// transport-neutral terminal egress shape without depending on private runtime
/// implementation details.
pub fn assert_terminal_output_round_trips(
    session_id: SessionId,
    subscription_id: SubscriptionId,
    chunks: impl IntoIterator<Item = impl AsRef<[u8]>>,
) -> Vec<TransportEgress> {
    let chunks: Vec<Vec<u8>> = chunks
        .into_iter()
        .map(|chunk| chunk.as_ref().to_vec())
        .collect();

    let mut encoded = Vec::new();
    for chunk in &chunks {
        let frame = encode_frame(FRAME_PTY_OUTPUT, chunk)
            .expect("terminal output chunks should encode as PTY output frames");
        encoded.extend_from_slice(&frame);
    }

    let mut decoder = FrameDecoder::new();
    let frames = decoder
        .feed(&encoded)
        .expect("encoded terminal output frames should decode");

    assert_eq!(frames.len(), chunks.len());
    for (frame, chunk) in frames.iter().zip(&chunks) {
        assert_eq!(frame.frame_type, FRAME_PTY_OUTPUT);
        assert_eq!(&frame.payload, chunk);
    }

    let egress: Vec<TransportEgress> = chunks
        .into_iter()
        .map(|chunk| TransportEgress::TerminalOutput {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            data: chunk,
        })
        .collect();
    let json = serde_json::to_string(&egress)
        .expect("terminal output egress should serialize through public serde contract");
    let decoded: Vec<TransportEgress> = serde_json::from_str(&json)
        .expect("terminal output egress should deserialize through public serde contract");
    assert_eq!(decoded, egress);

    egress
}

/// Assert that a terminal backend preserves opaque snapshot bytes, dimensions,
/// and host-owned format labels through capture and replay.
pub fn assert_terminal_backend_snapshot_round_trips_opaque_state<R>(runtime: R)
where
    R: TerminalScreenRuntime,
{
    let snapshot_size = TerminalScreenSize::new(37, 111);
    let output_bytes = b"snapshot-state\x00\xff";
    let mut engine = TerminalScreenEngine::new(runtime);

    assert_eq!(
        engine.resize(snapshot_size).hooks,
        vec![TerminalScreenHook::Resized {
            size: snapshot_size
        }]
    );
    let output = engine.normalize_output(output_bytes);
    assert!(matches!(
        output.output,
        Some(output) if output.bytes == output_bytes
    ));

    let captured = captured_snapshot(engine.capture_snapshot().snapshot);
    assert_eq!(captured.bytes, output_bytes);
    assert_eq!(captured.size, snapshot_size);

    engine.normalize_output(b"mutated-after-capture");
    engine.resize(TerminalScreenSize::new(12, 44));

    let replayed = engine.replay_snapshot(captured.clone());
    assert_eq!(
        replayed.hooks,
        vec![TerminalScreenHook::SnapshotReplayed {
            size: snapshot_size
        }]
    );

    let restored = captured_snapshot(engine.capture_snapshot().snapshot);
    assert_eq!(restored, captured);
}

/// Assert that backend dimensions survive snapshot capture, resize mutation,
/// and restore through the public terminal screen engine.
pub fn assert_terminal_backend_resize_survives_snapshot_restore<R>(runtime: R)
where
    R: TerminalScreenRuntime,
{
    let restored_size = TerminalScreenSize::new(41, 132);
    let mutated_size = TerminalScreenSize::new(19, 70);
    let output_bytes = b"resize-before-snapshot";
    let mut engine = TerminalScreenEngine::new(runtime);

    engine.resize(restored_size);
    engine.normalize_output(output_bytes);
    let snapshot = captured_snapshot(engine.capture_snapshot().snapshot);
    assert_eq!(snapshot.size, restored_size);

    engine.resize(mutated_size);
    assert_eq!(
        screen_state(engine.screen_state().screen).size,
        mutated_size,
        "runtime should report the mutated size before restore"
    );

    engine.replay_snapshot(snapshot);
    let restored = screen_state(engine.screen_state().screen);
    assert_eq!(restored.size, restored_size);
    assert_eq!(restored.plain_text, String::from_utf8_lossy(output_bytes));
}

/// Assert that current screen state is readable after output and metadata setup.
pub fn assert_terminal_backend_screen_state_matches_output_and_metadata<R>(
    runtime: R,
    expected_state: TerminalScreenState,
) where
    R: TerminalScreenRuntime,
{
    let mut engine = TerminalScreenEngine::new(runtime);
    engine.resize(expected_state.size);
    engine.normalize_output(expected_state.plain_text.as_bytes());

    let outcome = engine.screen_state();
    assert_eq!(
        outcome.hooks,
        vec![TerminalScreenHook::ScreenRead {
            size: expected_state.size
        }]
    );
    assert_eq!(screen_state(outcome.screen), expected_state);
}

/// Assert that attaching live output is held until after the authoritative
/// initial snapshot event is delivered.
pub fn assert_initial_snapshot_precedes_live_output() -> Vec<SessionIoEvent> {
    let session_id = SessionId("session-initial-snapshot-contract".to_string());
    let live_before_snapshot = b"live-before-snapshot\xff".to_vec();
    let snapshot = InitialSnapshotReady {
        request_id: RequestId("req-initial-snapshot-contract".to_string()),
        session_id: session_id.clone(),
        client_id: ClientId("client-initial-snapshot-contract".to_string()),
        subscription_id: SubscriptionId("sub-initial-snapshot-contract".to_string()),
        snapshot: b"initial-snapshot\x00".to_vec(),
        rows: 45,
        cols: 120,
    };
    let mut barrier = InitialSnapshotBarrier::new();

    assert_eq!(barrier.phase(), InitialSnapshotPhase::WaitingForSnapshot);
    assert_eq!(barrier.push_live_output(live_before_snapshot.clone()), None);

    let events = barrier.deliver_initial_snapshot(snapshot.clone());
    assert_eq!(barrier.phase(), InitialSnapshotPhase::LiveOutputActive);
    assert_eq!(
        events,
        vec![
            SessionIoEvent::InitialSnapshotReady(snapshot),
            SessionIoEvent::TerminalBytes {
                session_id: session_id.clone(),
                data: live_before_snapshot,
            },
        ]
    );

    assert_eq!(
        barrier.push_live_output(b"live-after-snapshot".to_vec()),
        Some(b"live-after-snapshot".to_vec())
    );

    events
}

fn captured_snapshot(snapshot: Option<TerminalSnapshotPayload>) -> TerminalSnapshotPayload {
    match snapshot {
        Some(snapshot) => snapshot,
        None => panic!("terminal backend should capture a snapshot"),
    }
}

fn screen_state(screen: Option<TerminalScreenState>) -> TerminalScreenState {
    match screen {
        Some(screen) => screen,
        None => panic!("terminal backend should read screen state"),
    }
}
