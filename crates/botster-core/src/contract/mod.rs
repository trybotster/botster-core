//! Stable contracts shared by Botster hosts, clients, providers, and plugins.
//!
//! Prefer importing through `botster_core::contract::<module>` (or
//! [`crate::prelude`] for session lifecycle essentials). Crate-root flat
//! re-exports of these types remain for compatibility.

pub mod actor;
pub mod boundary;
pub mod client;
pub mod client_stream;
pub mod durable_session;
pub mod encrypted_stream;
pub mod entity;
pub mod notification;
pub mod routed_envelope;
pub mod session;
pub mod session_protocol;
pub mod terminal_adapter;
pub mod terminal_metadata;
pub mod terminal_screen;
pub mod terminal_subscription;
pub mod terminal_wake;
pub mod transport;

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, DeliveryLag, HubControlMessage, HubControlOrigin,
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, InitialSnapshotRequest,
    MailboxSendFailure, MailboxSendFailureReason, ModeFlagsReady, PluginAdmissionResult,
    PluginBackpressureCause, PluginCleanupResult, PluginCleanupScope, PluginCompletion,
    PluginCompletionDrain, PluginCompletionItem, PluginDescriptorKind, PluginDescriptorRef,
    PluginHandlerKind, PluginHandlerRef, PluginInvocationClass, PluginInvocationContext,
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginInvocationSuccess, PluginKey, PluginLoadSpec,
    PluginOwnedDescriptor, PluginReloadSpec, PluginResourceKind, PluginResourceRef,
    PluginTimerCancellationResult, PluginTimerEvent, PluginTimerId, PluginTimerMode,
    PluginTimerSchedule, PluginUnloadSpec, PluginWorkerEvent, PluginWorkerMessage,
    PreparedSnapshotReady, PreparedSnapshotRequest, QueueSource, ScreenReady, SendFileErrorReason,
    SendFileFailed, SendFileRequest, SendFileWritten, SessionIoCoalescingPolicy, SessionIoEvent,
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
pub use encrypted_stream::{
    EncryptedStreamBackpressure, EncryptedStreamClose, EncryptedStreamCloseReason,
    EncryptedStreamControlFrame, EncryptedStreamDropReason, EncryptedStreamError,
    EncryptedStreamFrame, EncryptedStreamFrameHeader, EncryptedStreamKeyId, EncryptedStreamLane,
    EncryptedStreamLaneCounters, EncryptedStreamLaneDiscipline, EncryptedStreamMetadataFrame,
    EncryptedStreamPairingState, EncryptedStreamPayload, EncryptedStreamPayloadKind,
    EncryptedStreamPeerId, EncryptedStreamRejectionReason, EncryptedStreamSequence,
    EncryptedStreamSequenceValidator, EncryptedStreamStorageKeyId, EncryptedStreamTranscriptId,
    EncryptedStreamValidation, ENCRYPTED_STREAM_CONTRACT_VERSION,
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
    decode_final_state, decode_hello, decode_welcome, decode_worker_input_operation, encode_empty,
    encode_final_state, encode_frame, encode_hello, encode_json, encode_startup_failure,
    encode_string, encode_welcome, encode_worker_input_operation, encode_worker_operation,
    read_hello, read_startup_reply, read_welcome, split_worker_operation_key, write_hello,
    write_startup_failure, write_welcome, Frame, FrameDecoder, ModeFlags, ModeFlagsPayload,
    NotificationPayload, ProcessExitedPayload, PromptMarkPayload, ProtocolError, ResizePayload,
    Rgb, ScreenPayload, SessionMetadata, StartupFailureOutcome, StartupFailureReport, StartupReply,
    TeePayload, TerminalColorProfile, TimeoutPayload, WorkerFinalState, WorkerInputKind,
    WorkerInputOperation, WorkerProbeRequest, DESYNC_THRESHOLD, FRAME_ARM_TEE, FRAME_BELL,
    FRAME_CWD_CHANGED, FRAME_FINAL_STATE, FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN,
    FRAME_GET_SNAPSHOT, FRAME_INPUT_CANCEL, FRAME_INPUT_OPERATION, FRAME_INPUT_RESULT,
    FRAME_METADATA_SHAPING, FRAME_MODES_CHANGED, FRAME_MODE_FLAGS, FRAME_NOTIFICATION, FRAME_PING,
    FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT,
    FRAME_RESIZE, FRAME_RESIZE_APPLIED, FRAME_SCREEN, FRAME_SET_COLOR_PROFILE, FRAME_SET_TIMEOUT,
    FRAME_SHUTDOWN, FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED, HELLO_MAGIC,
    MAX_FRAME_LEN, MAX_METADATA_LEN, PROTOCOL_VERSION, STARTUP_FAILURE_MAGIC, WELCOME_MAGIC,
    WORKER_OPERATION_KEY_BYTES,
};
pub use session_protocol::{WorkerSnapshotPhase, WorkerSnapshotRequest, WorkerSnapshotResult};
pub use terminal_metadata::{
    TerminalMetadataKind, TerminalMetadataLaneShaper, TerminalMetadataObservation,
    TerminalMetadataProducer, TerminalMetadataShapingObservation, TerminalMetadataShapingOutcome,
};
pub use terminal_screen::{
    TerminalBackendError, TerminalOutputChunk, TerminalScreenHook, TerminalScreenSize,
    TerminalScreenState, TerminalSnapshotPayload,
};
pub use terminal_subscription::{
    AttachTerminalRouteError, BindTerminalAdapterError, DetachTerminalSubscriptionResult,
    StagedTerminalInput, TerminalCapabilitySet, TerminalCapabilitySetError, TerminalInputCommand,
    TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
};
pub use terminal_wake::{
    SessionWakeHandle, TerminalWakeBatch, TerminalWakeKind, TerminalWakeRoute, TerminalWakeSink,
    TerminalWakeSource, WakingTerminalAdapter, WAKE_QUEUE_CAPACITY,
};
pub use transport::{TransportEgress, TransportIngress};
