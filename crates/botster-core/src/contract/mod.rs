//! Stable contracts shared by Botster hosts, clients, providers, and plugins.

pub mod actor;
pub mod boundary;
pub mod client;
pub mod client_stream;
pub mod durable_session;
pub mod entity;
pub mod notification;
pub mod routed_envelope;
pub mod session;
pub mod session_protocol;
pub mod terminal_screen;
pub mod transport;
pub mod ui;

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, DeliveryLag, HubControlMessage, HubControlOrigin,
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, InitialSnapshotRequest,
    MailboxSendFailure, MailboxSendFailureReason, ModeFlagsReady, PluginCleanupResult,
    PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginInvocationContext, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult,
    PluginInvocationSuccess, PluginKey, PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec,
    PluginResourceKind, PluginResourceRef, PluginTimerCancellationResult, PluginTimerEvent,
    PluginTimerId, PluginTimerMode, PluginTimerSchedule, PluginUnloadSpec, PluginWorkerEvent,
    PluginWorkerMessage, PreparedSnapshotReady, PreparedSnapshotRequest, QueueSource, ScreenReady,
    SendFileErrorReason, SendFileFailed, SendFileRequest, SendFileWritten,
    SessionIoCoalescingPolicy, SessionIoEvent, SessionIoOrderedEvent, SessionIoRequest,
    SessionLifecycleState, SnapshotReady, TerminalAttachState, TransportConnectionMode,
    TransportDisconnectReason, TransportPeerState, TransportSignal, PUBLIC_QUEUE_SOURCES,
    SESSION_IO_MAX_COALESCED_BYTES, SESSION_IO_MAX_COALESCED_FRAMES,
    SESSION_IO_MAX_COALESCED_WINDOW,
};
pub use boundary::{BoundaryJson, Layer, LayerResponsibility};
pub use client::{ClientId, ClientScope, ClientState};
pub use client_stream::{
    ClientStreamGeneration, ClientStreamHarness, ClientStreamObservation, ClientStreamOutcome,
};
pub use durable_session::{
    DaemonCliOperation, DaemonControlOperation, DaemonControlOutcome, DurableRestartSemantics,
    DurableSessionProtocolVersion, GuardedSessionWriteDeferralReason, GuardedSessionWritePolicy,
    GuardedSessionWritePrimitive, GuardedSessionWriteRejectionReason, GuardedSessionWriteRequest,
    GuardedSessionWriteState, RestartBoundary, RestartSurvival, SessionReadinessEvidence,
    SessionWorkerAdoptRequest, SessionWorkerAdoptionVerdict, SessionWorkerAttachRequest,
    SessionWorkerCapability, SessionWorkerDetached, SessionWorkerFailure, SessionWorkerHealth,
    SessionWorkerHealthReason, SessionWorkerHeartbeat, SessionWorkerId, SessionWorkerIdentity,
    SessionWorkerOutputFrame, SessionWorkerProcessIdentity, SessionWorkerQueueLimits,
    SessionWorkerShutdownMode, SessionWorkerShutdownRequest, SessionWorkerSpawnRequest,
    SessionWorkerSpawned, SessionWorkerStaleReason, SlowConsumerBehavior, SnapshotHandoffStrategy,
    DURABLE_SESSION_PROTOCOL_VERSION,
};
pub use entity::{
    EntityApplyStatus, EntityContract, EntityError, EntityFrame, EntityId, EntityKind, EntityStore,
    EntityStores,
};
pub use notification::{
    NotificationAction, NotificationContent, NotificationDeliveryStatus, NotificationId,
    NotificationInbox, NotificationItem, NotificationKind, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp,
};
pub use routed_envelope::{
    EndpointId, EnvelopeCursor, EnvelopeDeliveryState, EnvelopeDeliveryStatus, EnvelopeId,
    EnvelopeTarget, RoutedEnvelope, RoutedEnvelopeDrainOutcome, RoutedEnvelopeObservation,
    RoutedEnvelopePayload, RoutedEnvelopePublishOutcome, RoutedEnvelopeQueueConfig,
};
pub use session::{
    CoreSession, CoreSessionMetadata, RequestId, SessionActivity, SessionActivityEvent,
    SessionActivityStatus, SessionId, SubscriptionId, MAX_CORE_SESSION_METADATA_LEN,
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
pub use terminal_screen::{
    TerminalOutputChunk, TerminalScreenHook, TerminalScreenSize, TerminalScreenState,
    TerminalSnapshotPayload,
};
pub use transport::{TransportEgress, TransportIngress};
pub use ui::{
    validate_ui_node_with_capabilities, UiAction, UiActionId, UiActionKind, UiActionRequest,
    UiActionRequestId, UiActionResult, UiActionResultState, UiBind, UiBindIf, UiBindList,
    UiCapabilityFallback, UiCapabilitySet, UiChild, UiColorToken, UiCondition, UiConditional,
    UiDialogPresentation, UiFieldErrors, UiFieldKind, UiFieldOption, UiFieldSchema,
    UiFieldValidationHints, UiFormValues, UiHeightClass, UiKeyboardCapability, UiNode, UiNodeId,
    UiNodeKind, UiOrientation, UiPointer, UiResponsiveHeight, UiResponsiveValue, UiResponsiveWidth,
    UiSpaceToken, UiSurfaceId, UiTreeUpdateRef, UiValidationError, UiViewport, UiWidthClass,
};
