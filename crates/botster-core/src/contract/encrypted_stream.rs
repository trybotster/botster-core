//! Transport-neutral encrypted ordered stream contract.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::crypto::{decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError};
use crate::transport::{TransportEgress, TransportIngress};

/// Current encrypted stream contract version.
pub const ENCRYPTED_STREAM_CONTRACT_VERSION: u8 = 1;

/// Stable peer identifier carried by encrypted stream handshakes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EncryptedStreamPeerId(pub String);

/// Identifier for the sealing key selected by the host-owned handshake.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EncryptedStreamKeyId(pub String);

/// Identifier for the handshake transcript that produced the active stream keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EncryptedStreamTranscriptId(pub String);

/// Identifier for host-owned persisted material needed to resume a paired peer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EncryptedStreamStorageKeyId(pub String);

/// Mechanism-level pairing and handshake state for an encrypted stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamPairingState {
    /// No peer pairing has started.
    Unpaired,
    /// Pairing material has been presented but not authenticated.
    PairingPresented,
    /// The remote peer identity has been authenticated.
    PeerAuthenticated,
    /// Stream keys and transcript identifiers have been established.
    KeysEstablished,
    /// The encrypted ordered stream may carry transport frames.
    Ready,
    /// The stream has closed and must not accept more frames.
    Closed,
}

/// Monotonic frame sequence number for one encrypted ordered stream direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EncryptedStreamSequence(pub u64);

/// Protocol lane inside one encrypted ordered stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamLane {
    /// Lossless control that must not be skipped.
    CriticalControl,
    /// Lossless live terminal input, output, attach, resize, and snapshot flow.
    TerminalLive,
    /// Coalescible terminal metadata such as titles, prompts, readiness, or mode hints.
    TerminalMetadata,
    /// Replay or bulk history payloads that may be shaped independently from live traffic.
    BulkReplay,
    /// Diagnostics and observability payloads that must not break live traffic when dropped.
    Diagnostics,
}

impl EncryptedStreamLane {
    /// All lanes defined by this contract.
    pub const ALL: [Self; 5] = [
        Self::CriticalControl,
        Self::TerminalLive,
        Self::TerminalMetadata,
        Self::BulkReplay,
        Self::Diagnostics,
    ];

    /// Return the ordering discipline for this protocol lane.
    #[must_use]
    pub const fn discipline(self) -> EncryptedStreamLaneDiscipline {
        match self {
            Self::CriticalControl | Self::TerminalLive | Self::BulkReplay => {
                EncryptedStreamLaneDiscipline::Lossless
            }
            Self::TerminalMetadata | Self::Diagnostics => {
                EncryptedStreamLaneDiscipline::LossyLatestWins
            }
        }
    }
}

/// Ordering and shedding discipline for an encrypted stream lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamLaneDiscipline {
    /// Every frame is lossless, ordered, and fail-closed.
    Lossless,
    /// Newer frames may supersede missing older frames without breaking other lanes.
    LossyLatestWins,
}

/// Public authenticated frame metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamFrameHeader {
    /// Contract version that produced the frame.
    pub version: u8,
    /// Protocol lane sequenced independently inside this encrypted stream.
    pub lane: EncryptedStreamLane,
    /// Monotonic sequence number for this stream direction.
    pub sequence: EncryptedStreamSequence,
    /// Identifier for the selected sealing key.
    pub key_id: EncryptedStreamKeyId,
    /// Identifier for the active handshake transcript.
    pub transcript_id: EncryptedStreamTranscriptId,
    /// Plaintext payload family sealed by the frame.
    pub payload_kind: EncryptedStreamPayloadKind,
}

/// Payload family carried by an encrypted stream frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamPayloadKind {
    /// Client-to-runtime transport ingress frame.
    TransportIngress,
    /// Runtime-to-client transport egress frame.
    TransportEgress,
    /// Stream control frame.
    Control,
    /// Coalescible terminal metadata frame.
    Metadata,
}

/// Typed control payload for encrypted stream lifecycle and pressure signals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EncryptedStreamControlFrame {
    /// Notify the peer about stream pressure.
    Backpressure(EncryptedStreamBackpressure),
    /// Close the encrypted stream.
    Close(EncryptedStreamClose),
}

/// Coalescible terminal metadata carried outside terminal live bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamMetadataFrame {
    /// Prompt position or prompt lifecycle metadata.
    PromptMark,
    /// Terminal bell metadata.
    Bell,
    /// Notification metadata.
    Notification,
    /// Terminal mode flag metadata.
    ModeFlags,
    /// Screen readiness metadata.
    ScreenReady,
}

/// Backpressure observation carried over an encrypted stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamBackpressure {
    /// Current queued byte count for the stream direction.
    pub queued_bytes: u64,
    /// Configured queued byte limit for the stream direction.
    pub max_queued_bytes: u64,
}

/// Stream close frame carried over the encrypted stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamClose {
    /// Machine-readable close reason.
    pub reason: EncryptedStreamCloseReason,
    /// Human-readable diagnostic detail.
    pub message: String,
}

/// Machine-readable encrypted stream close reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamCloseReason {
    /// Peer requested normal stream shutdown.
    Normal,
    /// Peer detected authentication failure.
    AuthenticationFailed,
    /// Peer detected replay, duplicate, or dropped-frame evidence.
    IntegrityFailed,
    /// Peer is closing because pressure limits were exceeded.
    BackpressureExceeded,
    /// Peer is closing for a host-owned reason outside this core contract.
    HostClosed,
}

/// Plaintext payload sealed inside an encrypted stream frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum EncryptedStreamPayload {
    /// Client-to-runtime transport ingress frame.
    TransportIngress(TransportIngress),
    /// Runtime-to-client transport egress frame.
    TransportEgress(TransportEgress),
    /// Stream control frame.
    Control(EncryptedStreamControlFrame),
    /// Coalescible terminal metadata frame.
    Metadata(EncryptedStreamMetadataFrame),
}

impl EncryptedStreamPayload {
    /// Return the payload family for this plaintext payload.
    #[must_use]
    pub const fn kind(&self) -> EncryptedStreamPayloadKind {
        match self {
            Self::TransportIngress(_) => EncryptedStreamPayloadKind::TransportIngress,
            Self::TransportEgress(_) => EncryptedStreamPayloadKind::TransportEgress,
            Self::Control(_) => EncryptedStreamPayloadKind::Control,
            Self::Metadata(_) => EncryptedStreamPayloadKind::Metadata,
        }
    }

    /// Return the default protocol lane for this payload.
    #[must_use]
    pub const fn recommended_lane(&self) -> EncryptedStreamLane {
        match self {
            Self::TransportIngress(_) => EncryptedStreamLane::TerminalLive,
            Self::TransportEgress(egress) => recommended_egress_lane(egress),
            Self::Control(_) => EncryptedStreamLane::CriticalControl,
            Self::Metadata(_) => EncryptedStreamLane::TerminalMetadata,
        }
    }
}

const fn recommended_egress_lane(egress: &TransportEgress) -> EncryptedStreamLane {
    match egress {
        TransportEgress::TerminalOutput { .. } => EncryptedStreamLane::TerminalLive,
        TransportEgress::Snapshot { .. } | TransportEgress::Scrollback { .. } => {
            EncryptedStreamLane::BulkReplay
        }
        TransportEgress::ProcessExit { .. }
        | TransportEgress::AttachState { .. }
        | TransportEgress::FocusChanged { .. }
        | TransportEgress::Pong { .. }
        | TransportEgress::Close { .. } => EncryptedStreamLane::CriticalControl,
        TransportEgress::Binary { .. } | TransportEgress::BoundaryPayload { .. } => {
            EncryptedStreamLane::Diagnostics
        }
    }
}

/// Authenticated encrypted frame carrying one ordered stream payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamFrame {
    /// Public frame header, duplicated inside the authenticated plaintext.
    pub header: EncryptedStreamFrameHeader,
    /// Authenticated encrypted frame bytes.
    pub envelope: AesGcmEnvelope,
}

impl EncryptedStreamFrame {
    /// Seal a plaintext payload into its recommended encrypted stream lane.
    pub fn seal_payload_on_recommended_lane(
        sequence: EncryptedStreamSequence,
        key_id: EncryptedStreamKeyId,
        transcript_id: EncryptedStreamTranscriptId,
        key: &AesGcmKey,
        payload: EncryptedStreamPayload,
    ) -> Result<Self, EncryptedStreamError> {
        Self::seal_payload(
            payload.recommended_lane(),
            sequence,
            key_id,
            transcript_id,
            key,
            payload,
        )
    }

    /// Seal a plaintext payload into an encrypted stream frame.
    pub fn seal_payload(
        lane: EncryptedStreamLane,
        sequence: EncryptedStreamSequence,
        key_id: EncryptedStreamKeyId,
        transcript_id: EncryptedStreamTranscriptId,
        key: &AesGcmKey,
        payload: EncryptedStreamPayload,
    ) -> Result<Self, EncryptedStreamError> {
        let header = EncryptedStreamFrameHeader {
            version: ENCRYPTED_STREAM_CONTRACT_VERSION,
            lane,
            sequence,
            key_id,
            transcript_id,
            payload_kind: payload.kind(),
        };
        let plaintext = EncryptedStreamPlaintext {
            header: header.clone(),
            payload,
        };
        let bytes = serde_json::to_vec(&plaintext)?;
        let envelope = encrypt_aes_gcm(key, &bytes, ENCRYPTED_STREAM_CONTRACT_VERSION)?;

        Ok(Self { header, envelope })
    }

    /// Open and authenticate a sealed encrypted stream payload.
    pub fn open_payload(
        &self,
        key: &AesGcmKey,
    ) -> Result<EncryptedStreamPayload, EncryptedStreamError> {
        let bytes = decrypt_aes_gcm(key, &self.envelope)?;
        let plaintext: EncryptedStreamPlaintext = serde_json::from_slice(&bytes)?;

        if plaintext.header != self.header {
            return Err(EncryptedStreamError::HeaderMismatch);
        }

        Ok(plaintext.payload)
    }

    /// Seal a transport ingress frame.
    pub fn seal_transport_ingress(
        sequence: EncryptedStreamSequence,
        key_id: EncryptedStreamKeyId,
        transcript_id: EncryptedStreamTranscriptId,
        key: &AesGcmKey,
        ingress: TransportIngress,
    ) -> Result<Self, EncryptedStreamError> {
        Self::seal_payload_on_recommended_lane(
            sequence,
            key_id,
            transcript_id,
            key,
            EncryptedStreamPayload::TransportIngress(ingress),
        )
    }

    /// Seal a transport egress frame.
    pub fn seal_transport_egress(
        sequence: EncryptedStreamSequence,
        key_id: EncryptedStreamKeyId,
        transcript_id: EncryptedStreamTranscriptId,
        key: &AesGcmKey,
        egress: TransportEgress,
    ) -> Result<Self, EncryptedStreamError> {
        Self::seal_payload_on_recommended_lane(
            sequence,
            key_id,
            transcript_id,
            key,
            EncryptedStreamPayload::TransportEgress(egress),
        )
    }
}

/// Sequence validation result for an encrypted ordered stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EncryptedStreamValidation {
    /// The frame was the next expected sequence number.
    Accepted {
        /// Protocol lane that accepted the frame.
        lane: EncryptedStreamLane,
        /// Accepted sequence number.
        sequence: EncryptedStreamSequence,
        /// Next expected sequence number after accepting this frame.
        next_expected: EncryptedStreamSequence,
    },
    /// A lossy lane skipped missing older frames and accepted the newer frame.
    Coalesced {
        /// Protocol lane that accepted a newer frame.
        lane: EncryptedStreamLane,
        /// Expected sequence before accepting the newer frame.
        expected: EncryptedStreamSequence,
        /// Received newer sequence.
        received: EncryptedStreamSequence,
        /// Next expected sequence number after accepting this frame.
        next_expected: EncryptedStreamSequence,
    },
    /// A lossy lane intentionally dropped a frame without advancing.
    Dropped {
        /// Protocol lane that dropped the frame.
        lane: EncryptedStreamLane,
        /// Expected sequence at validation time.
        expected: EncryptedStreamSequence,
        /// Dropped sequence.
        received: EncryptedStreamSequence,
        /// Reason the frame was dropped.
        reason: EncryptedStreamDropReason,
    },
    /// A frame was rate-limited before validation.
    RateLimited {
        /// Protocol lane that rate-limited the frame.
        lane: EncryptedStreamLane,
        /// Sequence number that was rate-limited.
        received: EncryptedStreamSequence,
    },
    /// The frame was rejected and must not be buffered or reordered.
    Rejected {
        /// Protocol lane that rejected the frame.
        lane: EncryptedStreamLane,
        /// Expected sequence number at validation time.
        expected: EncryptedStreamSequence,
        /// Received sequence number.
        received: EncryptedStreamSequence,
        /// Reason the frame was rejected.
        reason: EncryptedStreamRejectionReason,
    },
}

/// Reason a lossy lane dropped an encrypted stream frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamDropReason {
    /// The frame was older than the latest accepted sequence for the lane.
    Superseded,
    /// The stream was already closed.
    StreamClosed,
}

/// Reason an encrypted stream frame failed sequence validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncryptedStreamRejectionReason {
    /// The frame sequence was lower than the next expected sequence.
    ReplayOrDuplicate,
    /// The frame sequence was higher than the next expected sequence.
    GapOrOutOfOrder,
    /// The validator was closed before the frame arrived.
    StreamClosed,
}

/// Observable validation counters for one encrypted stream lane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamLaneCounters {
    /// Frames accepted with the exact next expected sequence.
    pub accepted: u64,
    /// Lossy-lane frames accepted after skipping older missing frames.
    pub coalesced: u64,
    /// Frames rate-limited before validation.
    pub rate_limited: u64,
    /// Lossy-lane frames dropped without advancing the lane.
    pub dropped: u64,
    /// Lossless-lane frames rejected as integrity failures.
    pub rejected: u64,
}

/// Fail-closed ordered stream sequence validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedStreamSequenceValidator {
    next_expected_by_lane: BTreeMap<EncryptedStreamLane, EncryptedStreamSequence>,
    counters_by_lane: BTreeMap<EncryptedStreamLane, EncryptedStreamLaneCounters>,
    closed: bool,
}

impl EncryptedStreamSequenceValidator {
    /// Build a validator with the supplied next expected sequence.
    #[must_use]
    pub fn new(next_expected: EncryptedStreamSequence) -> Self {
        Self::new_for_lane(EncryptedStreamLane::CriticalControl, next_expected)
    }

    /// Build a validator with one lane initialized explicitly and every other lane at zero.
    #[must_use]
    pub fn new_for_lane(lane: EncryptedStreamLane, next_expected: EncryptedStreamSequence) -> Self {
        let mut next_expected_by_lane = BTreeMap::new();
        let mut counters_by_lane = BTreeMap::new();
        for initialized_lane in EncryptedStreamLane::ALL {
            next_expected_by_lane.insert(initialized_lane, EncryptedStreamSequence(0));
            counters_by_lane.insert(initialized_lane, EncryptedStreamLaneCounters::default());
        }
        next_expected_by_lane.insert(lane, next_expected);

        Self {
            next_expected_by_lane,
            counters_by_lane,
            closed: false,
        }
    }

    /// Return the next expected sequence.
    #[must_use]
    pub fn next_expected(&self) -> EncryptedStreamSequence {
        self.next_expected_for_lane(EncryptedStreamLane::CriticalControl)
    }

    /// Return the next expected sequence for one protocol lane.
    #[must_use]
    pub fn next_expected_for_lane(&self, lane: EncryptedStreamLane) -> EncryptedStreamSequence {
        self.next_expected_by_lane
            .get(&lane)
            .copied()
            .unwrap_or(EncryptedStreamSequence(0))
    }

    /// Return observable counters for one protocol lane.
    #[must_use]
    pub fn counters_for_lane(&self, lane: EncryptedStreamLane) -> EncryptedStreamLaneCounters {
        self.counters_by_lane
            .get(&lane)
            .copied()
            .unwrap_or_default()
    }

    /// Return whether the validator is closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Mark the stream closed. Future frames are rejected.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Validate one sequence number without buffering or reordering.
    pub fn validate(&mut self, received: EncryptedStreamSequence) -> EncryptedStreamValidation {
        self.validate_in_lane(EncryptedStreamLane::CriticalControl, received)
    }

    /// Validate one sequence number within one protocol lane.
    pub fn validate_in_lane(
        &mut self,
        lane: EncryptedStreamLane,
        received: EncryptedStreamSequence,
    ) -> EncryptedStreamValidation {
        let expected = self.next_expected_for_lane(lane);

        if self.closed {
            return match lane.discipline() {
                EncryptedStreamLaneDiscipline::Lossless => {
                    self.increment_counter(lane, |counters| counters.rejected += 1);
                    EncryptedStreamValidation::Rejected {
                        lane,
                        expected,
                        received,
                        reason: EncryptedStreamRejectionReason::StreamClosed,
                    }
                }
                EncryptedStreamLaneDiscipline::LossyLatestWins => {
                    self.increment_counter(lane, |counters| counters.dropped += 1);
                    EncryptedStreamValidation::Dropped {
                        lane,
                        expected,
                        received,
                        reason: EncryptedStreamDropReason::StreamClosed,
                    }
                }
            };
        }

        if received == expected {
            let next_expected = EncryptedStreamSequence(expected.0 + 1);
            self.next_expected_by_lane.insert(lane, next_expected);
            self.increment_counter(lane, |counters| counters.accepted += 1);
            return EncryptedStreamValidation::Accepted {
                lane,
                sequence: received,
                next_expected,
            };
        }

        if lane.discipline() == EncryptedStreamLaneDiscipline::LossyLatestWins {
            if received > expected {
                let next_expected = EncryptedStreamSequence(received.0 + 1);
                self.next_expected_by_lane.insert(lane, next_expected);
                self.increment_counter(lane, |counters| counters.coalesced += 1);
                return EncryptedStreamValidation::Coalesced {
                    lane,
                    expected,
                    received,
                    next_expected,
                };
            }

            self.increment_counter(lane, |counters| counters.dropped += 1);
            return EncryptedStreamValidation::Dropped {
                lane,
                expected,
                received,
                reason: EncryptedStreamDropReason::Superseded,
            };
        }

        let reason = if received < expected {
            EncryptedStreamRejectionReason::ReplayOrDuplicate
        } else {
            EncryptedStreamRejectionReason::GapOrOutOfOrder
        };

        self.increment_counter(lane, |counters| counters.rejected += 1);
        EncryptedStreamValidation::Rejected {
            lane,
            expected,
            received,
            reason,
        }
    }

    /// Validate one frame by its public header sequence.
    pub fn validate_frame(&mut self, frame: &EncryptedStreamFrame) -> EncryptedStreamValidation {
        self.validate_in_lane(frame.header.lane, frame.header.sequence)
    }

    /// Mark a frame as rate-limited before validation.
    pub fn rate_limited(&mut self, frame: &EncryptedStreamFrame) -> EncryptedStreamValidation {
        self.increment_counter(frame.header.lane, |counters| counters.rate_limited += 1);
        EncryptedStreamValidation::RateLimited {
            lane: frame.header.lane,
            received: frame.header.sequence,
        }
    }

    fn increment_counter(
        &mut self,
        lane: EncryptedStreamLane,
        increment: impl FnOnce(&mut EncryptedStreamLaneCounters),
    ) {
        let counters = self.counters_by_lane.entry(lane).or_default();
        increment(counters);
    }
}

impl Default for EncryptedStreamSequenceValidator {
    fn default() -> Self {
        Self::new(EncryptedStreamSequence(0))
    }
}

/// Errors returned while sealing or opening encrypted stream frames.
#[derive(Debug, Error)]
pub enum EncryptedStreamError {
    /// JSON serialization or deserialization failed.
    #[error("encrypted stream payload serialization failed")]
    Serialization(#[from] serde_json::Error),
    /// The underlying crypto primitive failed.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// The public header differed from the authenticated plaintext header.
    #[error("encrypted stream frame header did not match authenticated plaintext")]
    HeaderMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EncryptedStreamPlaintext {
    header: EncryptedStreamFrameHeader,
    payload: EncryptedStreamPayload,
}
