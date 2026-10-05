//! Guardian messages on the private control link (plan 3; Core AD-6, SV-5, SV-9).

use botster_core_contract::prelude::*;
use botster_core_link::msg::PayloadId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    Status,
    LogTail {
        req: u64,
        max: u64,
    },
}

/// The guardian's observed state, retained across host loss (SV-5, SV-8).
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
    /// Log chunks are consecutive. Only the last chunk has `last = true`.
    Log {
        req: u64,
        bytes: Vec<u8>,
        last: bool,
    },
    Removed,
}

impl Command {
    /// Unknown fields are readable. An unknown command cannot run.
    pub fn decode(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}
