//! The Hub→client contract types: the envelope, errors, handshake, `describe`, watch and event frames, every Hub-owned kind, pairing and the MCP tool data.
//!
//! Source: `frozen/current` HC parity45 with A1 to A4 (ledger prefix `hc`). The crate holds types, the closed registries and the schema generation. It holds no Hub logic.
//! The route frames are `botster-route-codec` and the view vocabulary is `botster-ui-contract`; this crate uses their types.
//!
//! JSON forms (A.1, A.1b): an enum value with no data is a string, an enum value with data is a one-key object. A u64 id is a decimal string.
//! An optional field is absent when it has no value, never `null`. A non-exhaustive set reads an unknown value as `Other` (EN-6).

mod actions;
mod describe;
mod envelope;
mod error;
mod events;
mod handshake;
mod host;
mod mcp;
mod pairing;
mod refs;
mod registry;
mod rows;
#[cfg(feature = "unstable")]
pub mod schema;
#[cfg(not(feature = "unstable"))]
#[allow(dead_code)]
mod schema;
#[cfg(test)]
mod tests;
mod watch;
mod wire;

/// The names that a user of the Hub contract needs.
pub mod prelude {
    pub use crate::actions::*;
    pub use crate::describe::{
        ActionDescriptor, AppliesTo, DescribeResult, DescribedReason, EffectClass, KindDescriptor,
        ListMode, VerbDescriptor,
    };
    pub use crate::envelope::{
        Cancel, ClientFrame, Close, Frame, ListArgs, ListResult, Outcome, Req, ReqViolation, Res,
        ServerFrame, UpdateArgs, Verb, WatchArgs, WatchOpened,
    };
    pub use crate::error::{
        errors_json, Category, Error, ErrorCode, ErrorEffect, ErrorRow, FieldError, Retryable,
        COMMON_REASONS, ERROR_TABLE,
    };
    pub use crate::events::{
        Event, EventFilter, EventFrame, EventGap, EventPriority, EventSource, GapTag,
        NoticePayload, Recovery, RouteEndedPayload, RouteEndedSubject, SessionNotification,
        Severity, SourceKind, StateRead, ROUTES_STREAM_ID,
    };
    pub use crate::handshake::{
        route_datachannel_label, route_id_of_label, BusyData, ClientInfo, CloseReason, Hello,
        HubParameter, HubShutdownData, Limits, ProtocolErrorData, VersionRange,
        VersionUnsupportedData, Welcome, BINDINGS, BINDING_FEATURE_PREFIX,
        CONTROL_DATACHANNEL_LABEL, FEATURE_UI_1_1, FEATURE_UI_EMBED, PROTOCOL, READY_FAILURE_CODES,
        ROUTE_DATACHANNEL_LABEL_PREFIX, ROUTE_SEND_BUFFER_BYTES,
    };
    pub use crate::host::{SessionContextFile, TrustedHost};
    pub use crate::mcp::{
        list_uri, row_uri, AnyRef, CreateTool, DeleteTool, GetTool, InvokeTool, ListTool,
        ToolRequestFields, UpdateTool,
    };
    pub use crate::pairing::{
        accepted_transcript, auth_transcript, pairing_transcript, AcceptedMode, AuthAccepted,
        AuthChallenge, AuthFrame, AuthResponse, BundleError, IceServer, IceServers, PairRequest,
        PairingBundle, RouteConnect, Signal, ACCEPTED_TRANSCRIPT_TAG, AUTH_TRANSCRIPT_TAG,
        BUNDLE_VERSION, PAIRING_TRANSCRIPT_TAG,
    };
    pub use crate::refs::{ResourceRef, SessionRef};
    pub use crate::registry::{
        kind as kind_spec, operation_kind, range_filters, ActionSpec, KindSpec, OperationKindSpec,
        SchemaFn, VerbSpec, KINDS, OPERATION_KINDS, RANGE_FILTERS,
    };
    pub use crate::rows::*;
    pub use crate::watch::{Change, SnapshotReason, WatchClosedReason, WatchFrame};
    pub use crate::wire::{OpenObject, RequestId, Timestamp};
    /// `ModeFlags` is the codec's type (Codec A.6).
    pub use botster_route_codec::prelude::{LifecycleClass, ModeFlags, RouteLimits, SessionState};
}
