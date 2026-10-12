//! The Core contract: the types, the `CoreApi` trait, the error codes and `CoreLimits` of
//! `frozen/final9/core-contract-v1.17.md` with Core Amendments 2, 3 and 4.
//!
//! Types and one trait. The only logic is `CoreLimits::validate` (Core 9B) and the code tables of `ErrorCode`.
//! The route codec is `botster-route-codec`, and this crate uses its types.
//!
//! JSON forms: a core-owned enum uses the contract's spelling (`Created`, `NotWritten`) and a field uses `snake_case`.
//! A unit variant is a string, and a variant with data is a one-key object. Durations are integer milliseconds.
//! Byte payloads are `{"$bytes_hex": "..."}` of the codec. The `Event` and `Op` forms are the forms that conformance transcripts write.

mod api;
mod error;
mod event;
mod features;
mod ids;
mod input;
mod limits;
mod op;
mod read;
mod route;
#[cfg(feature = "unstable")]
pub mod schema;
#[cfg(not(feature = "unstable"))]
#[allow(dead_code)]
mod schema;
mod service;
#[cfg(test)]
mod tests;
mod types;
mod util;

/// The names that a user of the Core contract needs.
pub mod prelude {
    pub use crate::api::{
        CoreApi, Now, OpenConfig, PumpReport, Wake, WakeHandle, WorkerCompatibility,
    };
    pub use crate::error::{CoreError, ErrorCategory, ErrorCode, Timing, UnsupportedReason};
    pub use crate::event::{
        ActivitySource, ClipboardContent, ClipboardReason, Event, FocusState, LostKind,
        NotificationSource, PromptMarkKind,
    };
    pub use crate::features::{Feature, Features};
    pub use crate::ids::{
        CaptureId, ClientId, InputCursor, InputRev, InstanceId, ModelRev, OpId, RouteId, ServiceId,
        SessionId,
    };
    pub use crate::input::{
        CancelRefusal, CancelResult, Guard, InputGuard, InputPayload, InputResult, KeyInput,
        MouseInput, NotWrittenReason, SourceClass, UnsupportedWhat, WriteOutcome,
    };
    pub use crate::limits::{CoreLimits, InvalidConfig};
    pub use crate::op::{
        NotDeleted, Op, OpOutput, OpResult, RemoveReport, ResizeResult, UploadsOutcome,
    };
    pub use crate::read::{
        Capture, ClientFacts, Cursor, Facts, FactsCursor, HostFacts, InputFacts, InputKind,
        InputRecord, InputRecordEntry, InputRecordState, InputSource, InputStatusUpdate, Modes,
        Page, Screen, SnapshotFormat, TapChunk, TerminalIdentity,
    };
    pub use crate::route::{
        AppliedRouteLimits, AttachOptions, AttachRefused, AttachResult, DetachReason, HistoryMode,
        RouteCloseReason, RouteLimitChoices, RouteTransport, StreamEndpoint,
    };
    pub use crate::service::{
        Bound, Frame, LaneEndReason, LaneSpec, OutboundFrame, RecvError, RlimitSpec, Rlimits,
        SendError, ServiceEnd, ServiceExit, ServiceExitCause, ServiceLimits, ServiceState,
        SpawnReport, CORE_SERVICE_TRANSPORT_UNIX_LANES_1,
    };
    pub use crate::types::{
        CellPx, ColorProfile, Exit, ExitCause, InputRevs, LostReason, NotificationPolicy, Rgb,
        SessionEnd, SessionRecord, SessionState, SessionStatus, Signal, Size, SizePolicy,
        SpawnRequest, StartFailReason, Status, TerminalState, UnixSeconds,
    };
    /// `ModeFlags` is the codec's type (Codec A.6, Core 5.1A).
    pub use botster_route_codec::prelude::ModeFlags;
}
