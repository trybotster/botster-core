//! Reusable Botster runtime contracts and transport-neutral primitives.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients. It
//! defines stable data shapes and low-level contracts, while `botster-hub`
//! owns Botster policy and orchestration.

pub mod actor;
pub mod boundary;
pub mod capability;
pub mod client;
pub mod crypto;
pub mod entity;
pub mod extension;
pub mod package;
pub mod session;
pub mod transport;
pub mod ui;

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    PasteFileErrorReason, PluginHandlerKind, PluginHandlerRef, PluginKey, PluginLoadSpec,
    PluginWorkerEvent, PluginWorkerMessage, PreparedSnapshot, QueueSource, SessionIoEvent,
    SessionIoRequest, SessionLifecycleState, TerminalAttachState, TerminalColorProfile,
    TerminalModeSummary, TransportConnectionMode, TransportDisconnectReason, TransportPeerState,
    TransportSignal, PUBLIC_QUEUE_SOURCES,
};
pub use boundary::{BoundaryJson, Layer, LayerResponsibility};
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use crypto::{CryptoOperation, IdentityOperation};
pub use entity::{EntityFrame, EntityId, EntityKind};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use package::{PackageManifest, PackageSource};
pub use session::{RequestId, SessionId, SubscriptionId};
pub use transport::{TransportEgress, TransportIngress};
