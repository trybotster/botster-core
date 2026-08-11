//! Session-process wire protocol contract tests.

use std::collections::HashMap;

use botster_core::session_protocol::*;

fn metadata() -> SessionMetadata {
    SessionMetadata {
        session_uuid: "sess-test-123".to_string(),
        pid: 42,
        rows: 24,
        cols: 80,
        last_output_at: 1_234,
        title: Some("Build".to_string()),
        cwd: Some("/work/repo".to_string()),
        port: Some(4321),
        mode_flags: ModeFlags {
            kitty_enabled: true,
            cursor_visible: false,
            bracketed_paste: true,
            mouse_mode: 6,
            alt_screen: true,
            focus_reporting: true,
            application_cursor: false,
        },
        recovery_identity: Some(serde_json::json!({
            "session_type": "agent",
            "workspace_id": "ws-test",
        })),
    }
}

#[test]
fn handshake_round_trips_magic_version_and_metadata() {
    let encoded_hello = encode_hello(PROTOCOL_VERSION);
    let encoded_welcome = encode_welcome(PROTOCOL_VERSION, &metadata())
        .expect("expected protocol operation to succeed");

    assert_eq!(&encoded_hello[..4], HELLO_MAGIC);
    assert_eq!(
        decode_hello(&encoded_hello).expect("expected protocol operation to succeed"),
        PROTOCOL_VERSION
    );

    let (version, decoded) =
        decode_welcome(&encoded_welcome).expect("expected protocol operation to succeed");
    assert_eq!(version, PROTOCOL_VERSION);
    assert_eq!(decoded, metadata());
}

#[test]
fn frame_constants_match_session_process_wire_spec() {
    // Values are pinned from reference evidence:
    // /Users/jasonconigliari/Rails/trybotster/cli/src/session/protocol.rs
    assert_eq!(PROTOCOL_VERSION, 2);
    assert_eq!(HELLO_MAGIC, b"SPH1");
    assert_eq!(WELCOME_MAGIC, b"SPA1");
    assert_eq!(MAX_METADATA_LEN, 64 * 1024);
    assert_eq!(MAX_FRAME_LEN, 128 * 1024 * 1024);
    assert_eq!(DESYNC_THRESHOLD, 100);

    assert_eq!(FRAME_PTY_INPUT, 0x01);
    assert_eq!(FRAME_PTY_OUTPUT, 0x02);
    assert_eq!(FRAME_RESIZE, 0x03);
    assert_eq!(FRAME_ARM_TEE, 0x04);
    assert_eq!(FRAME_GET_SNAPSHOT, 0x05);
    assert_eq!(FRAME_SNAPSHOT, 0x06);
    assert_eq!(FRAME_PROCESS_EXITED, 0x07);
    assert_eq!(FRAME_PING, 0x08);
    assert_eq!(FRAME_PONG, 0x09);
    assert_eq!(FRAME_SHUTDOWN, 0x0a);
    assert_eq!(FRAME_SET_TIMEOUT, 0x0b);
    assert_eq!(FRAME_GET_MODE_FLAGS, 0x0c);
    assert_eq!(FRAME_MODE_FLAGS, 0x0d);
    assert_eq!(FRAME_GET_SCREEN, 0x0e);
    assert_eq!(FRAME_SCREEN, 0x0f);
    assert_eq!(FRAME_TITLE_CHANGED, 0x10);
    assert_eq!(FRAME_BELL, 0x11);
    // 0x12 was the legacy pushed terminal mode-change frame and is no longer a
    // public core wire protocol event.
    assert_eq!(FRAME_CWD_CHANGED, 0x13);
    assert_eq!(FRAME_PROMPT_MARK, 0x14);
    assert_eq!(FRAME_NOTIFICATION, 0x15);
    assert_eq!(FRAME_SET_COLOR_PROFILE, 0x16);
    assert_eq!(FRAME_SPAWN_SESSION, 0x17);
    assert_eq!(FRAME_METADATA_SHAPING, 0x18);
    assert_eq!(FRAME_MODE_GATED_PTY_INPUT, 0x19);
    assert_eq!(FRAME_MODE_GATED_PTY_INPUT_RESULT, 0x1a);
}

#[test]
fn mode_gated_request_and_result_round_trip_json() {
    let request = ModeGatedPtyInputRequest {
        request_id: "req-1".to_string(),
        expected: ModeFreshnessToken {
            mode_generation: 9,
            mode_revision: 3,
        },
        data: b"hello\n".to_vec(),
    };
    let encoded = serde_json::to_vec(&request).expect("encode request");
    let decoded: ModeGatedPtyInputRequest =
        serde_json::from_slice(&encoded).expect("decode request");
    assert_eq!(decoded, request);

    let result = ModeGatedPtyInputResult {
        request_id: "req-1".to_string(),
        admitted: false,
        mode_flags: ModeFlags {
            kitty_enabled: true,
            ..ModeFlags::default()
        },
        mode_freshness: ModeFreshnessToken {
            mode_generation: 9,
            mode_revision: 4,
        },
        error_kind: None,
    };
    let encoded = serde_json::to_vec(&result).expect("encode result");
    let decoded: ModeGatedPtyInputResult = serde_json::from_slice(&encoded).expect("decode result");
    assert_eq!(decoded, result);
}

#[test]
fn frame_round_trips_binary_string_empty_and_json_payloads() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        &encode_frame(FRAME_PTY_OUTPUT, b"\x00\xffraw")
            .expect("expected protocol operation to succeed"),
    );
    bytes.extend_from_slice(
        &encode_string(FRAME_TITLE_CHANGED, "terminal")
            .expect("expected protocol operation to succeed"),
    );
    bytes.extend_from_slice(
        &encode_empty(FRAME_BELL).expect("expected protocol operation to succeed"),
    );
    bytes.extend_from_slice(
        &encode_json(
            FRAME_RESIZE,
            &ResizePayload {
                rows: 30,
                cols: 100,
            },
        )
        .expect("expected protocol operation to succeed"),
    );

    let mut decoder = FrameDecoder::new();
    let frames = decoder
        .feed(&bytes)
        .expect("expected protocol operation to succeed");

    assert_eq!(frames.len(), 4);
    assert_eq!(frames[0].payload, b"\x00\xffraw");
    assert_eq!(
        std::str::from_utf8(&frames[1].payload).expect("expected protocol operation to succeed"),
        "terminal"
    );
    assert!(frames[2].payload.is_empty());
    assert_eq!(
        frames[3]
            .json::<ResizePayload>()
            .expect("expected protocol operation to succeed"),
        ResizePayload {
            rows: 30,
            cols: 100
        }
    );
}

#[test]
fn decoder_buffers_split_header_and_payload_until_complete() {
    let encoded =
        encode_frame(FRAME_PTY_INPUT, b"hello").expect("expected protocol operation to succeed");
    let mut decoder = FrameDecoder::new();

    assert!(decoder
        .feed(&encoded[..3])
        .expect("expected protocol operation to succeed")
        .is_empty());
    assert!(decoder
        .feed(&encoded[3..6])
        .expect("expected protocol operation to succeed")
        .is_empty());

    let frames = decoder
        .feed(&encoded[6..])
        .expect("expected protocol operation to succeed");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].frame_type, FRAME_PTY_INPUT);
    assert_eq!(frames[0].payload, b"hello");
}

#[test]
fn decoder_drains_multiple_frames_from_one_feed() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        &encode_frame(FRAME_PTY_OUTPUT, b"one").expect("expected protocol operation to succeed"),
    );
    bytes.extend_from_slice(
        &encode_frame(FRAME_PTY_OUTPUT, b"two").expect("expected protocol operation to succeed"),
    );
    bytes.extend_from_slice(
        &encode_empty(FRAME_PONG).expect("expected protocol operation to succeed"),
    );

    let mut decoder = FrameDecoder::new();
    let frames = decoder
        .feed(&bytes)
        .expect("expected protocol operation to succeed");

    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].payload, b"one");
    assert_eq!(frames[1].payload, b"two");
    assert!(frames[2].payload.is_empty());
}

#[test]
fn decoder_rejects_zero_length_header() {
    let mut decoder = FrameDecoder::new();
    let err = decoder
        .feed(&0u32.to_le_bytes())
        .expect_err("expected protocol operation to fail");

    assert!(matches!(err, ProtocolError::FrameLengthZero));
}

#[test]
fn decoder_rejects_oversized_header() {
    let mut decoder = FrameDecoder::new();
    let oversized = ((MAX_FRAME_LEN as u32) + 1).to_le_bytes();
    let err = decoder
        .feed(&oversized)
        .expect_err("expected protocol operation to fail");

    assert!(matches!(
        err,
        ProtocolError::FrameLengthTooLarge { len, max }
            if len == MAX_FRAME_LEN + 1 && max == MAX_FRAME_LEN
    ));
}

#[test]
fn decoder_reports_desync_after_repeated_bad_headers() {
    let mut decoder = FrameDecoder::new();

    for _ in 1..DESYNC_THRESHOLD {
        decoder
            .record_discarded_header()
            .expect("expected protocol operation to succeed");
        assert!(!decoder.is_desynced());
    }

    let err = decoder
        .record_discarded_header()
        .expect_err("expected protocol operation to fail");
    assert!(matches!(
        err,
        ProtocolError::Desynchronized {
            bad_headers,
            threshold
        } if bad_headers == DESYNC_THRESHOLD && threshold == DESYNC_THRESHOLD
    ));
    assert!(decoder.is_desynced());
}

#[test]
fn handshake_rejects_metadata_over_64k() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(WELCOME_MAGIC);
    bytes.push(PROTOCOL_VERSION);
    bytes.extend_from_slice(&((MAX_METADATA_LEN as u32) + 1).to_le_bytes());

    let err = decode_welcome(&bytes).expect_err("expected protocol operation to fail");

    assert!(matches!(
        err,
        ProtocolError::MetadataTooLarge { len, max }
            if len == MAX_METADATA_LEN + 1 && max == MAX_METADATA_LEN
    ));
}

#[test]
fn session_metadata_round_trips_optional_recovery_identity_and_mode_flags() {
    let json = serde_json::to_vec(&metadata()).expect("expected protocol operation to succeed");
    let decoded: SessionMetadata =
        serde_json::from_slice(&json).expect("expected protocol operation to succeed");

    assert_eq!(decoded, metadata());
    assert_eq!(
        decoded
            .recovery_identity
            .expect("expected protocol operation to succeed")["session_type"],
        serde_json::json!("agent")
    );
}

#[test]
fn mode_flags_round_trip_all_fields() {
    let flags = metadata().mode_flags;
    let json = serde_json::to_vec(&flags).expect("expected protocol operation to succeed");
    let decoded: ModeFlags =
        serde_json::from_slice(&json).expect("expected protocol operation to succeed");

    assert_eq!(decoded, flags);
}

#[test]
fn terminal_color_profile_serializes_core_rgb_map() {
    let mut colors = HashMap::new();
    colors.insert(0, Rgb { r: 1, g: 2, b: 3 });
    colors.insert(
        257,
        Rgb {
            r: 254,
            g: 253,
            b: 252,
        },
    );
    let profile = TerminalColorProfile { colors };

    let json = serde_json::to_vec(&profile).expect("expected protocol operation to succeed");
    let decoded: TerminalColorProfile =
        serde_json::from_slice(&json).expect("expected protocol operation to succeed");

    assert_eq!(decoded, profile);
}

#[test]
fn process_exit_payload_supports_code_and_signal_absence() {
    let exited = ProcessExitedPayload {
        exit_code: Some(0),
        signal: None,
    };
    let unknown = ProcessExitedPayload {
        exit_code: None,
        signal: None,
    };

    assert_eq!(
        serde_json::from_slice::<ProcessExitedPayload>(
            &serde_json::to_vec(&exited).expect("expected protocol operation to succeed")
        )
        .expect("expected protocol operation to succeed"),
        exited
    );
    assert_eq!(
        serde_json::from_slice::<ProcessExitedPayload>(
            &serde_json::to_vec(&unknown).expect("expected protocol operation to succeed")
        )
        .expect("expected protocol operation to succeed"),
        unknown
    );
}

#[test]
fn handshake_exposes_peer_protocol_version_without_policy_negotiation() {
    let peer_version = PROTOCOL_VERSION - 1;
    let encoded_hello = encode_hello(peer_version);
    let encoded_welcome =
        encode_welcome(peer_version, &metadata()).expect("expected protocol operation to succeed");

    assert_eq!(
        decode_hello(&encoded_hello).expect("expected protocol operation to succeed"),
        peer_version
    );
    assert_eq!(
        decode_welcome(&encoded_welcome)
            .expect("expected protocol operation to succeed")
            .0,
        peer_version
    );
}

#[test]
fn snapshot_frame_round_trips_opaque_bytes_without_parsing() {
    let snapshot = vec![0, 1, 2, 3, 255, 128, 64, 32];
    let encoded =
        encode_frame(FRAME_SNAPSHOT, &snapshot).expect("expected protocol operation to succeed");
    let mut decoder = FrameDecoder::new();
    let frames = decoder
        .feed(&encoded)
        .expect("expected protocol operation to succeed");

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].frame_type, FRAME_SNAPSHOT);
    assert_eq!(frames[0].payload, snapshot);
}
