//! Guardian messages on the private control link (plan 3; Core AD-6, SV-5, SV-9).
//!
//! Commands and reports are JSON in `HOST_MSG` and `WORKER_MSG` frames. Log bytes are bulk data, so they travel raw in
//! [`LOG_FRAME`] frames (plan 3).

use botster_core_contract::prelude::*;
use botster_core_link::frame::{FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::msg::PayloadId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

/// The launch input. The host supplies every Core environment variable (SV-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSpec {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    pub limits: ServiceLimits,
}

/// A command from an authenticated host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Command {
    Launch(Box<ServiceSpec>),
    /// The host committed the first complete lane epoch (A2-5).
    EpochCommitted,
    Stop,
    Remove,
}

/// The guardian's observed state, retained across host loss (SV-5, SV-8). After each authentication the guardian sends its
/// retained log ring and then this status, so a host that has read it holds the whole tail (SV-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub service: ServiceId,
    pub payload: Option<PayloadId>,
    pub report: Option<SpawnReport>,
    pub exit: Option<ServiceExit>,
}

/// A result from the guardian. The host maps it to its operation and event queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Report {
    Started {
        payload: PayloadId,
        report: SpawnReport,
    },
    StartFailed {
        reason: StartFailReason,
    },
    BoundUnavailable {
        bound: Bound,
    },
    Exited {
        exit: ServiceExit,
    },
    Status(Status),
    Removed,
}

impl Command {
    /// Unknown fields are readable. An unknown command cannot run.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// The frame type of [`LogChunk`] on a guardian link. Packages name their own frame types (`botster-core-link`).
pub const LOG_FRAME: FrameType = FrameType(0x20);

/// The bytes of the offset that precedes the log bytes.
const OFFSET_LEN: usize = 8;

/// The log chunk size of the guardian: the most log bytes that fit one [`LOG_FRAME`] at the link's default bound.
/// No clause fixes a chunk size (SV-9); any positive size up to this one carries the same bytes.
pub const LOG_CHUNK_BYTES: NonZeroUsize =
    NonZeroUsize::new(DEFAULT_MAX_PAYLOAD as usize - OFFSET_LEN).unwrap();

/// Captured stdout and stderr bytes, `[u64 LE offset][bytes]` (SV-9).
///
/// `offset` counts every byte the service wrote before `bytes`. A receiver that sees an offset other than the end of what
/// it holds has missed bytes, so it keeps only what follows. The guardian always resends its whole ring after such a gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogChunk {
    pub offset: u64,
    pub bytes: Vec<u8>,
}

impl LogChunk {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(OFFSET_LEN + self.bytes.len());
        out.extend_from_slice(&self.offset.to_le_bytes());
        out.extend_from_slice(&self.bytes);
        out
    }

    /// `None` when the payload is shorter than the offset.
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let (offset, bytes) = payload.split_first_chunk::<OFFSET_LEN>()?;
        Some(LogChunk {
            offset: u64::from_le_bytes(*offset),
            bytes: bytes.to_vec(),
        })
    }
}

/// Splits a log tail that starts at `offset` into chunks of at most `max` bytes, with contiguous offsets (SV-9).
pub fn log_chunks(
    offset: u64,
    bytes: &[u8],
    max: NonZeroUsize,
) -> impl Iterator<Item = LogChunk> + '_ {
    (offset..)
        .step_by(max.get())
        .zip(bytes.chunks(max.get()))
        .map(|(offset, bytes)| LogChunk {
            offset,
            bytes: bytes.to_vec(),
        })
}
