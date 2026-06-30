//! Encrypted ordered stream contract tests.

use botster_core::{
    AesGcmKey, EncryptedStreamBackpressure, EncryptedStreamClose, EncryptedStreamCloseReason,
    EncryptedStreamControlFrame, EncryptedStreamDropReason, EncryptedStreamError,
    EncryptedStreamFrame, EncryptedStreamKeyId, EncryptedStreamLane, EncryptedStreamLaneCounters,
    EncryptedStreamMetadataFrame, EncryptedStreamPairingState, EncryptedStreamPayload,
    EncryptedStreamPayloadKind, EncryptedStreamPeerId, EncryptedStreamRejectionReason,
    EncryptedStreamSequence, EncryptedStreamSequenceValidator, EncryptedStreamStorageKeyId,
    EncryptedStreamTranscriptId, EncryptedStreamValidation, RequestId, SessionId, SubscriptionId,
    TransportEgress, TransportIngress,
};

fn key() -> AesGcmKey {
    AesGcmKey::new([9; 32])
}

fn key_id() -> EncryptedStreamKeyId {
    EncryptedStreamKeyId("stream-key-1".to_string())
}

fn transcript_id() -> EncryptedStreamTranscriptId {
    EncryptedStreamTranscriptId("transcript-1".to_string())
}

fn session_id() -> SessionId {
    SessionId("session-1".to_string())
}

fn subscription_id() -> SubscriptionId {
    SubscriptionId("subscription-1".to_string())
}

fn request_id() -> RequestId {
    RequestId("request-1".to_string())
}

#[test]
fn sequence_validator_accepts_only_next_sequence() {
    let mut validator = EncryptedStreamSequenceValidator::default();

    assert_eq!(
        validator.validate(EncryptedStreamSequence(0)),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::CriticalControl,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
    assert_eq!(
        validator.validate(EncryptedStreamSequence(1)),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::CriticalControl,
            sequence: EncryptedStreamSequence(1),
            next_expected: EncryptedStreamSequence(2),
        }
    );
}

#[test]
fn sequence_validator_rejects_duplicate_and_replay_without_advancing() {
    let mut validator = EncryptedStreamSequenceValidator::default();
    validator.validate(EncryptedStreamSequence(0));
    validator.validate(EncryptedStreamSequence(1));

    assert_eq!(
        validator.validate(EncryptedStreamSequence(1)),
        EncryptedStreamValidation::Rejected {
            lane: EncryptedStreamLane::CriticalControl,
            expected: EncryptedStreamSequence(2),
            received: EncryptedStreamSequence(1),
            reason: EncryptedStreamRejectionReason::ReplayOrDuplicate,
        }
    );
    assert_eq!(validator.next_expected(), EncryptedStreamSequence(2));
}

#[test]
fn sequence_validator_rejects_gap_or_out_of_order_without_buffering() {
    let mut validator = EncryptedStreamSequenceValidator::default();
    validator.validate(EncryptedStreamSequence(0));

    assert_eq!(
        validator.validate(EncryptedStreamSequence(2)),
        EncryptedStreamValidation::Rejected {
            lane: EncryptedStreamLane::CriticalControl,
            expected: EncryptedStreamSequence(1),
            received: EncryptedStreamSequence(2),
            reason: EncryptedStreamRejectionReason::GapOrOutOfOrder,
        }
    );
    assert_eq!(validator.next_expected(), EncryptedStreamSequence(1));

    assert_eq!(
        validator.validate(EncryptedStreamSequence(1)),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::CriticalControl,
            sequence: EncryptedStreamSequence(1),
            next_expected: EncryptedStreamSequence(2),
        }
    );
}

#[test]
fn sequence_validator_rejects_after_close() {
    let mut validator = EncryptedStreamSequenceValidator::default();
    validator.close();

    assert_eq!(
        validator.validate(EncryptedStreamSequence(0)),
        EncryptedStreamValidation::Rejected {
            lane: EncryptedStreamLane::CriticalControl,
            expected: EncryptedStreamSequence(0),
            received: EncryptedStreamSequence(0),
            reason: EncryptedStreamRejectionReason::StreamClosed,
        }
    );
}

#[test]
fn sequence_validator_tracks_protected_lanes_independently() {
    let mut validator = EncryptedStreamSequenceValidator::default();

    assert_eq!(
        validator.validate_in_lane(
            EncryptedStreamLane::CriticalControl,
            EncryptedStreamSequence(0)
        ),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::CriticalControl,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
    assert_eq!(
        validator.validate_in_lane(
            EncryptedStreamLane::TerminalLive,
            EncryptedStreamSequence(0)
        ),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::TerminalLive,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
    assert_eq!(
        validator.next_expected_for_lane(EncryptedStreamLane::CriticalControl),
        EncryptedStreamSequence(1)
    );
    assert_eq!(
        validator.next_expected_for_lane(EncryptedStreamLane::TerminalLive),
        EncryptedStreamSequence(1)
    );
}

#[test]
fn lossy_lane_coalescing_does_not_advance_protected_lanes() {
    let mut validator = EncryptedStreamSequenceValidator::default();

    assert_eq!(
        validator.validate_in_lane(
            EncryptedStreamLane::TerminalMetadata,
            EncryptedStreamSequence(5)
        ),
        EncryptedStreamValidation::Coalesced {
            lane: EncryptedStreamLane::TerminalMetadata,
            expected: EncryptedStreamSequence(0),
            received: EncryptedStreamSequence(5),
            next_expected: EncryptedStreamSequence(6),
        }
    );
    assert_eq!(
        validator.validate_in_lane(
            EncryptedStreamLane::TerminalMetadata,
            EncryptedStreamSequence(4)
        ),
        EncryptedStreamValidation::Dropped {
            lane: EncryptedStreamLane::TerminalMetadata,
            expected: EncryptedStreamSequence(6),
            received: EncryptedStreamSequence(4),
            reason: EncryptedStreamDropReason::Superseded,
        }
    );
    assert_eq!(
        validator.next_expected_for_lane(EncryptedStreamLane::CriticalControl),
        EncryptedStreamSequence(0)
    );
    assert_eq!(
        validator.validate_in_lane(
            EncryptedStreamLane::CriticalControl,
            EncryptedStreamSequence(0)
        ),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::CriticalControl,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
}

#[test]
fn bulk_replay_lane_is_lossless_and_rejects_gaps_and_replays() {
    let mut validator = EncryptedStreamSequenceValidator::new_for_lane(
        EncryptedStreamLane::BulkReplay,
        EncryptedStreamSequence(0),
    );

    assert_eq!(
        validator.validate_in_lane(EncryptedStreamLane::BulkReplay, EncryptedStreamSequence(2)),
        EncryptedStreamValidation::Rejected {
            lane: EncryptedStreamLane::BulkReplay,
            expected: EncryptedStreamSequence(0),
            received: EncryptedStreamSequence(2),
            reason: EncryptedStreamRejectionReason::GapOrOutOfOrder,
        }
    );
    assert_eq!(
        validator.validate_in_lane(EncryptedStreamLane::BulkReplay, EncryptedStreamSequence(0)),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::BulkReplay,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
    assert_eq!(
        validator.validate_in_lane(EncryptedStreamLane::BulkReplay, EncryptedStreamSequence(0)),
        EncryptedStreamValidation::Rejected {
            lane: EncryptedStreamLane::BulkReplay,
            expected: EncryptedStreamSequence(1),
            received: EncryptedStreamSequence(0),
            reason: EncryptedStreamRejectionReason::ReplayOrDuplicate,
        }
    );
}

#[test]
fn per_lane_counters_track_accepted_coalesced_rate_limited_and_dropped(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut validator = EncryptedStreamSequenceValidator::default();

    validator.validate_in_lane(
        EncryptedStreamLane::TerminalMetadata,
        EncryptedStreamSequence(0),
    );
    validator.validate_in_lane(
        EncryptedStreamLane::TerminalMetadata,
        EncryptedStreamSequence(5),
    );
    validator.validate_in_lane(
        EncryptedStreamLane::TerminalMetadata,
        EncryptedStreamSequence(4),
    );

    let rate_limited_frame = EncryptedStreamFrame::seal_payload_on_recommended_lane(
        EncryptedStreamSequence(6),
        key_id(),
        transcript_id(),
        &key(),
        EncryptedStreamPayload::Metadata(EncryptedStreamMetadataFrame::Bell),
    )?;

    assert_eq!(
        validator.rate_limited(&rate_limited_frame),
        EncryptedStreamValidation::RateLimited {
            lane: EncryptedStreamLane::TerminalMetadata,
            received: EncryptedStreamSequence(6),
        }
    );
    assert_eq!(
        validator.counters_for_lane(EncryptedStreamLane::TerminalMetadata),
        EncryptedStreamLaneCounters {
            accepted: 1,
            coalesced: 1,
            rate_limited: 1,
            dropped: 1,
            rejected: 0,
        }
    );
    assert_eq!(
        validator.counters_for_lane(EncryptedStreamLane::CriticalControl),
        EncryptedStreamLaneCounters::default()
    );

    Ok(())
}

#[test]
fn metadata_and_terminal_payloads_recommend_distinct_lanes(
) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = EncryptedStreamPayload::Metadata(EncryptedStreamMetadataFrame::PromptMark);
    let terminal = EncryptedStreamPayload::TransportEgress(TransportEgress::TerminalOutput {
        session_id: session_id(),
        subscription_id: subscription_id(),
        data: b"hello".to_vec(),
    });
    let replay = EncryptedStreamPayload::TransportEgress(TransportEgress::Snapshot {
        session_id: session_id(),
        subscription_id: subscription_id(),
        data: b"history".to_vec(),
    });

    assert_eq!(
        metadata.recommended_lane(),
        EncryptedStreamLane::TerminalMetadata
    );
    assert_eq!(
        terminal.recommended_lane(),
        EncryptedStreamLane::TerminalLive
    );
    assert_eq!(replay.recommended_lane(), EncryptedStreamLane::BulkReplay);

    let metadata_frame = EncryptedStreamFrame::seal_payload_on_recommended_lane(
        EncryptedStreamSequence(0),
        key_id(),
        transcript_id(),
        &key(),
        metadata,
    )?;

    assert_eq!(
        metadata_frame.header.lane,
        EncryptedStreamLane::TerminalMetadata
    );
    assert_eq!(
        metadata_frame.header.payload_kind,
        EncryptedStreamPayloadKind::Metadata
    );

    Ok(())
}

#[test]
fn encrypted_frame_round_trips_transport_ingress() -> Result<(), Box<dyn std::error::Error>> {
    let ingress = TransportIngress::RequestSnapshot {
        request_id: request_id(),
        session_id: session_id(),
    };

    let frame = EncryptedStreamFrame::seal_transport_ingress(
        EncryptedStreamSequence(0),
        key_id(),
        transcript_id(),
        &key(),
        ingress.clone(),
    )?;
    let mut validator = EncryptedStreamSequenceValidator::default();

    assert_eq!(
        frame.header.payload_kind,
        EncryptedStreamPayloadKind::TransportIngress
    );
    assert_eq!(
        validator.validate_frame(&frame),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::TerminalLive,
            sequence: EncryptedStreamSequence(0),
            next_expected: EncryptedStreamSequence(1),
        }
    );
    assert_eq!(
        frame.open_payload(&key())?,
        EncryptedStreamPayload::TransportIngress(ingress)
    );

    Ok(())
}

#[test]
fn encrypted_frame_round_trips_transport_egress() -> Result<(), Box<dyn std::error::Error>> {
    let egress = TransportEgress::TerminalOutput {
        session_id: session_id(),
        subscription_id: subscription_id(),
        data: b"hello".to_vec(),
    };

    let frame = EncryptedStreamFrame::seal_transport_egress(
        EncryptedStreamSequence(7),
        key_id(),
        transcript_id(),
        &key(),
        egress.clone(),
    )?;
    let mut validator = EncryptedStreamSequenceValidator::new_for_lane(
        EncryptedStreamLane::TerminalLive,
        EncryptedStreamSequence(7),
    );

    assert_eq!(
        frame.header.payload_kind,
        EncryptedStreamPayloadKind::TransportEgress
    );
    assert_eq!(
        validator.validate_frame(&frame),
        EncryptedStreamValidation::Accepted {
            lane: EncryptedStreamLane::TerminalLive,
            sequence: EncryptedStreamSequence(7),
            next_expected: EncryptedStreamSequence(8),
        }
    );
    assert_eq!(
        frame.open_payload(&key())?,
        EncryptedStreamPayload::TransportEgress(egress)
    );

    Ok(())
}

#[test]
fn encrypted_frame_round_trips_control_payloads() -> Result<(), Box<dyn std::error::Error>> {
    let backpressure = EncryptedStreamPayload::Control(EncryptedStreamControlFrame::Backpressure(
        EncryptedStreamBackpressure {
            queued_bytes: 1024,
            max_queued_bytes: 2048,
        },
    ));
    let close =
        EncryptedStreamPayload::Control(EncryptedStreamControlFrame::Close(EncryptedStreamClose {
            reason: EncryptedStreamCloseReason::IntegrityFailed,
            message: "replay detected".to_string(),
        }));

    for (sequence, payload) in [
        (EncryptedStreamSequence(0), backpressure),
        (EncryptedStreamSequence(1), close),
    ] {
        let frame = EncryptedStreamFrame::seal_payload(
            EncryptedStreamLane::CriticalControl,
            sequence,
            key_id(),
            transcript_id(),
            &key(),
            payload.clone(),
        )?;

        assert_eq!(frame.header.lane, EncryptedStreamLane::CriticalControl);
        assert_eq!(
            frame.header.payload_kind,
            EncryptedStreamPayloadKind::Control
        );
        assert_eq!(frame.open_payload(&key())?, payload);
    }

    Ok(())
}

#[test]
fn pairing_identity_and_storage_vocabulary_round_trips_through_json(
) -> Result<(), Box<dyn std::error::Error>> {
    let peer = EncryptedStreamPeerId("peer-1".to_string());
    let storage = EncryptedStreamStorageKeyId("storage-1".to_string());
    let state = EncryptedStreamPairingState::KeysEstablished;

    assert_eq!(
        serde_json::from_str::<EncryptedStreamPeerId>(&serde_json::to_string(&peer)?)?,
        peer
    );
    assert_eq!(
        serde_json::from_str::<EncryptedStreamStorageKeyId>(&serde_json::to_string(&storage)?)?,
        storage
    );
    assert_eq!(
        serde_json::from_str::<EncryptedStreamPairingState>(&serde_json::to_string(&state)?)?,
        state
    );

    Ok(())
}

#[test]
fn encrypted_frame_rejects_tampered_public_header() -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = EncryptedStreamFrame::seal_transport_ingress(
        EncryptedStreamSequence(0),
        key_id(),
        transcript_id(),
        &key(),
        TransportIngress::Ping {
            request_id: request_id(),
        },
    )?;

    frame.header.sequence = EncryptedStreamSequence(1);

    assert!(matches!(
        frame.open_payload(&key()),
        Err(EncryptedStreamError::HeaderMismatch)
    ));

    Ok(())
}

#[test]
fn encrypted_stream_contract_source_excludes_concrete_transport_terms() {
    let source = std::fs::read_to_string("src/contract/encrypted_stream.rs")
        .expect("read encrypted stream source");

    for term in [
        "RTCPeerConnection",
        "ICE",
        "DataChannel",
        "WebRTC",
        "WebRtc",
        "browser",
        "Rails",
        "cloud",
        "localhost",
    ] {
        assert!(
            !source.contains(term),
            "encrypted stream core contract must not contain banned term {term}"
        );
    }
}
