//! Reusable Botster runtime contracts and transport-neutral primitives.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients. It
//! defines stable data shapes and low-level contracts, while `botster-hub`
//! owns Botster policy and orchestration.

pub mod contract;
pub mod engine;
pub mod identity;
pub mod package;
pub mod runtime;

pub use contract::{
    actor, boundary, client, client_stream, entity, notification, session, session_protocol,
    terminal_screen, transport, ui,
};
pub use engine::{
    botster, managed_session_runtime, multiplexer, plugin_worker, session_activity, session_worker,
    subscription_multiplexer, terminal_screen as terminal_screen_engine,
};
pub use identity::{crypto, device, keyring};
pub use package::{capability, extension, manifest};
pub use runtime::{
    LocalProcessRuntime, PluginRuntime, ProcessIdentity, SessionRuntime, SessionRuntimeError,
    SessionRuntimeErrorKind, SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput,
    SessionSpawnRequest, SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory,
};

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, HubControlMessage, HubControlOrigin,
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, InitialSnapshotRequest,
    MailboxSendFailure, MailboxSendFailureReason, ModeFlagsReady, PluginCleanupResult,
    PluginCleanupScope, PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind,
    PluginHandlerRef, PluginInvocationContext, PluginInvocationFailure,
    PluginInvocationFailureKind, PluginInvocationRequest, PluginInvocationResult,
    PluginInvocationSuccess, PluginKey, PluginLoadSpec, PluginOwnedDescriptor, PluginReloadSpec,
    PluginResourceKind, PluginResourceRef, PluginUnloadSpec, PluginWorkerEvent,
    PluginWorkerMessage, PreparedSnapshotReady, PreparedSnapshotRequest, QueueSource, ScreenReady,
    SendFileErrorReason, SendFileFailed, SendFileRequest, SendFileWritten,
    SessionIoCoalescingPolicy, SessionIoEvent, SessionIoOrderedEvent, SessionIoRequest,
    SessionLifecycleState, SnapshotReady, TerminalAttachState, TransportConnectionMode,
    TransportDisconnectReason, TransportPeerState, TransportSignal, PUBLIC_QUEUE_SOURCES,
    SESSION_IO_MAX_COALESCED_BYTES, SESSION_IO_MAX_COALESCED_FRAMES,
    SESSION_IO_MAX_COALESCED_WINDOW,
};
pub use boundary::{BoundaryJson, Layer, LayerResponsibility};
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use client_stream::{
    ClientStreamGeneration, ClientStreamHarness, ClientStreamObservation, ClientStreamOutcome,
};
pub use crypto::{
    decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError, CryptoOperation,
    IdentityOperation,
};
pub use device::{
    device_fingerprint, verify_device_fingerprint, DeviceFingerprint, DevicePublicMetadata,
    PublicSigningKeyBytes,
};
pub use engine::{
    apply_session_activity_event, classify_session_activity, BotsterEngine, BotsterEngineError,
    BotsterEngineObservation, BotsterEngineOutput, BotsterSpawnOutcome, DefaultBotsterEngine,
    DefaultBotsterEngineError, ManagedSessionRuntime, ManagedSessionRuntimeError,
    MultiplexerEngine, MultiplexerEngineError, MultiplexerEngineObservation,
    MultiplexerEngineOutcome, MultiplexerSpawnOutcome, PluginHandlerRegistration,
    PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerRegistration, SessionWorkerEngine,
    SessionWorkerOutcome, SessionWorkerRuntime, SessionWorkerRuntimeEvent, SubscriptionMultiplexer,
    SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome, TerminalScreenEngine,
    TerminalScreenOutcome, TerminalScreenRuntime,
};
pub use entity::{
    EntityApplyStatus, EntityContract, EntityError, EntityFrame, EntityId, EntityKind, EntityStore,
    EntityStores,
};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use keyring::{
    CredentialRecord, CredentialStore, CredentialStoreError, NonExportableSigner, SignatureBytes,
    SigningError, SigningKeyHandle,
};
pub use notification::{
    NotificationAction, NotificationContent, NotificationDeliveryStatus, NotificationId,
    NotificationInbox, NotificationItem, NotificationKind, NotificationSeverity,
    NotificationSource, NotificationTarget, NotificationTimestamp,
};
pub use package::{PackageManifest, PackageSource};
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
    UiAction, UiActionId, UiActionPending, UiActionRequestId, UiActionResult, UiActionStatus,
    UiBind, UiBindIf, UiBindList, UiChild, UiColorToken, UiCondition, UiConditional, UiHeightClass,
    UiNode, UiNodeId, UiNodeKind, UiOrientation, UiPointer, UiResponsiveHeight, UiResponsiveValue,
    UiResponsiveWidth, UiSpaceToken, UiValidationError, UiViewport, UiWidthClass,
};
