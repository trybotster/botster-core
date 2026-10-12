//! `CoreApi`, the trait that `FakeCore` and the real `Core` implement, with `WakeHandle`, `OpenConfig` and the small result types.

use crate::error::CoreError;
use crate::event::Event;
use crate::features::Features;
use crate::ids::{CaptureId, ClientId, OpId, ServiceId, SessionId};
use crate::input::CancelResult;
use crate::limits::CoreLimits;
use crate::op::Op;
use crate::read::{Page, SnapshotFormat, TapChunk, TerminalIdentity};
use crate::route::{AttachOptions, AttachRefused, AttachResult, RouteTransport};
use crate::service::SpawnReport;
use crate::service::{Frame, OutboundFrame, RecvError, SendError};
use crate::types::{SessionRecord, Status, TerminalState, UnixSeconds};
use botster_route_codec::prelude::QueryKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The configuration of `Core::open`, which is a constructor and not a `CoreApi` method.
///
/// Clause: Core LC-1, Core LC-2, Core 9B.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenConfig {
    pub data_dir: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_path: Option<PathBuf>,
    #[serde(default)]
    pub limits: CoreLimits,
}

/// The two clocks that the host passes to `pump`.
///
/// Clause: Core TM-1, Core 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Now {
    pub monotonic: Instant,
    pub unix: UnixSeconds,
}

/// What `pump` reports.
///
/// Clause: Core A2-7, Core TM-6.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PumpReport {
    /// Runnable work remains. Blocked work does not count (TM-6).
    pub more: bool,
    pub events_posted: u32,
}

/// The result of `WakeHandle::wait`. A spurious `Woken` is allowed.
///
/// Clause: Core TH-2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum Wake {
    Woken = 1,
    TimedOut = 2,
}

/// The wake object that the host waits on.
///
/// Clause: Core 2, Core TH-2, Core TM-3, Core TM-6, Core 13 (rule 5).
pub trait WakeHandle: Send + Sync {
    /// Waits for a wake or the timeout.
    fn wait(&self, timeout: Duration) -> Wake;
    /// A pollable descriptor for the host's loop.
    fn fd(&self) -> RawFd;
}

/// Whether a worker of protocol `p` is compatible with this Core.
///
/// Clause: Core AD-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
#[repr(u8)]
pub enum WorkerCompatibility {
    Compatible = 1,
    Incompatible = 2,
    Unknown = 3,
}

/// The trait that `FakeCore` and the real `Core` implement. Object-safe and `Send` (TH-1).
///
/// Clause: Core 11, Core 2, Core TH-1, Core OR-1.
pub trait CoreApi: Send {
    /// Starts an operation.
    ///
    /// Clause: Core 2, Core 9.3 (ER-0), Core OR-1, Core AM-1.
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError>;

    /// The only call that makes progress.
    ///
    /// Clause: Core 2, Core OR-1, Core TM-2, Core A2-7.
    fn pump(&mut self, now: Now) -> PumpReport;

    /// Reads at most `max` events from the one ordered queue. It makes no progress.
    ///
    /// Clause: Core 2, Core OR-1, Core OR-2.
    fn poll_events(&mut self, max: usize) -> Vec<Event>;

    /// Clause: Core 2, Core TH-2, Core TM-6.
    fn wake_handle(&self) -> Arc<dyn WakeHandle>;

    /// The earliest deadline that Core owns, on the monotonic clock.
    ///
    /// Clause: Core TM-3.
    fn next_deadline(&self) -> Option<Instant>;

    /// Cancels a `WriteInput` op.
    ///
    /// Clause: Core IN-6, Core A2-1.
    fn cancel(&mut self, op: OpId) -> CancelResult;

    /// Clause: Core LC-9, Core 9.3.
    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError>;

    /// Clause: Core LC-9.
    fn list(&self) -> Vec<SessionRecord>;

    /// Sessions and states only.
    ///
    /// Clause: Core LC-10.
    fn status(&self) -> Status;

    /// An opaque value with no compatibility promise.
    ///
    /// Clause: Core LC-10.
    fn diagnostics(&self) -> serde_json::Value;

    /// Clause: Core ST-4, Core 9.3.
    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError>;

    /// Clause: Core ST-6, Core 9.3.
    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError>;

    /// Idempotent.
    ///
    /// Clause: Core ST-6.
    fn release(&mut self, capture: CaptureId);

    /// Releases every capture of a client.
    ///
    /// Clause: Core ST-6.
    fn release_owner(&mut self, client: &ClientId);

    /// The formats that a session can emit.
    ///
    /// Clause: Core ST-6, Core OU-1.
    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError>;

    /// The query kinds that the running implementation's shadow answers.
    ///
    /// Clause: Core EV-8 (shadow set).
    fn shadow_answerable_kinds(&self) -> Vec<QueryKind>;

    /// Registers a route. The registration is sync, and the baseline follows in a `pump`.
    ///
    /// The transport belongs to Core only from a successful return. A sync refusal returns it in [`AttachRefused`], untouched.
    ///
    /// Clause: Core OU-1, Core OU-9, Core DP-2, Core AM-4, steward ruling R-19.
    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused>;

    /// Drains up to `max` bytes of the raw-output tap.
    ///
    /// Clause: Core TP-1.
    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError>;

    /// `None` turns the threshold off.
    ///
    /// Clause: Core A2-7, Core TM-4.
    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError>;

    /// Sends a frame on a lane. The frame is borrowed, so a refused send leaves the caller's frame intact (SV-3).
    ///
    /// Clause: Core A2-5, Core SV-3.
    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError>;

    /// `Ok(None)` when the inbound queue is empty.
    ///
    /// Clause: Core A2-5, Core SV-3, Core SV-9.
    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError>;

    /// The spawn report of a service, from the `Ok` spawn until `RemoveService`. It posts no event. An unknown id is `UnknownService`.
    ///
    /// Clause: Core A4-1, Core SV-4.
    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError>;

    /// The last bytes of the service's stdout and stderr. No compatibility promise.
    ///
    /// Clause: Core SV-9.
    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError>;

    /// Clause: Core A2-6, Core 9.3.
    fn features(&self) -> Features;

    /// The limits that `open` accepted.
    ///
    /// Clause: Core 9B, Core LC-1.
    fn limits(&self) -> CoreLimits;

    /// The running Core's worker protocol number T.
    ///
    /// Clause: Core AD-4.
    fn worker_protocol(&self) -> u8;

    /// `{T, T - 1}`.
    ///
    /// Clause: Core AD-4.
    fn adoptable_worker_protocols(&self) -> BTreeSet<u8>;

    /// The compatibility of a worker protocol number.
    ///
    /// Clause: Core AD-4.
    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility;

    /// A sync read that needs no session.
    ///
    /// Clause: Core A2-8 (TI-1).
    fn terminal_identity(&self) -> TerminalIdentity;
}
