//! Stable contracts shared by Botster hosts, clients, providers, and plugins.

pub mod actor;
pub mod boundary;
pub mod client;
pub mod client_stream;
pub mod entity;
pub mod session;
pub mod session_protocol;
pub mod transport;
pub mod ui;

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, InitialSnapshotRequest,
    MailboxSendFailure, MailboxSendFailureReason, ModeFlagsReady, PasteFileErrorReason,
    PasteFileFailed, PasteFileRequest, PasteFileWritten, PluginCleanupResult, PluginCleanupScope,
    PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind, PluginHandlerRef,
    PluginInvocationContext, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginKey,
    PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef,
    PluginUnloadSpec, PluginWorkerEvent, PluginWorkerMessage, PreparedSnapshotReady,
    PreparedSnapshotRequest, QueueSource, ScreenReady, SessionIoCoalescingPolicy, SessionIoEvent,
    SessionIoOrderedEvent, SessionIoRequest, SessionLifecycleState, SnapshotReady,
    TerminalAttachState, TransportConnectionMode, TransportDisconnectReason, TransportPeerState,
    TransportSignal, PUBLIC_QUEUE_SOURCES, SESSION_IO_MAX_COALESCED_BYTES,
    SESSION_IO_MAX_COALESCED_FRAMES, SESSION_IO_MAX_COALESCED_WINDOW,
};
pub use boundary::{BoundaryJson, Layer, LayerResponsibility};
pub use client::{ClientId, ClientScope, ClientState};
pub use client_stream::{
    ClientStreamGeneration, ClientStreamHarness, ClientStreamObservation, ClientStreamOutcome,
};
pub use entity::{
    EntityApplyStatus, EntityContract, EntityError, EntityFrame, EntityId, EntityKind, EntityStore,
    EntityStores,
};
pub use session::{
    CoreSession, RequestId, SessionActivity, SessionActivityEvent, SessionActivityStatus,
    SessionId, SessionKind, SubscriptionId,
};
pub use session_protocol::{
    decode_hello, decode_welcome, encode_empty, encode_frame, encode_hello, encode_json,
    encode_string, encode_welcome, read_hello, read_welcome, write_hello, write_welcome, Frame,
    FrameDecoder, ModeFlags, NotificationPayload, ProcessExitedPayload, PromptMarkPayload,
    ProtocolError, ResizePayload, Rgb, SessionMetadata, TeePayload, TerminalColorProfile,
    TimeoutPayload, DESYNC_THRESHOLD, FRAME_ARM_TEE, FRAME_BELL, FRAME_CWD_CHANGED,
    FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN, FRAME_GET_SNAPSHOT, FRAME_MODE_FLAGS,
    FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK,
    FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE, FRAME_SCREEN, FRAME_SET_COLOR_PROFILE,
    FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT, FRAME_TITLE_CHANGED, HELLO_MAGIC,
    MAX_FRAME_LEN, MAX_METADATA_LEN, PROTOCOL_VERSION, WELCOME_MAGIC,
};
pub use transport::{TransportEgress, TransportIngress};
pub use ui::{
    UiAction, UiActionId, UiActionPending, UiActionRequestId, UiActionResult, UiActionStatus,
    UiBind, UiBindIf, UiBindList, UiChild, UiColorToken, UiCondition, UiConditional, UiHeightClass,
    UiNode, UiNodeId, UiNodeKind, UiOrientation, UiPointer, UiResponsiveHeight, UiResponsiveValue,
    UiResponsiveWidth, UiSpaceToken, UiValidationError, UiViewport, UiWidthClass,
};
