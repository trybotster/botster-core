//! Reusable Botster runtime contracts and the embeddable local session engine.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients: typed
//! contracts plus a policy-free library path. Production durable supervision
//! lives in the sibling `botster-core-daemon` crate (`CoreDaemon` +
//! `botster-session-worker`). Product policy stays in hosts such as
//! `botster-hub`.
//!
//! # Start here
//!
//! Import the curated surface and follow spawn → attach → drain → input →
//! shutdown:
//!
//! ```rust
//! use botster_core::prelude::*;
//! ```
//!
//! Full lifecycle docs and a compile-checked sketch live on [`prelude`]. Prefer
//! that module over scanning every crate-root re-export.
//!
//! | Host path | Entry |
//! | --- | --- |
//! | Library (default features) | `DefaultBotsterEngine` / `DefaultEngineCommand` via [`prelude`] |
//! | Library (custom runtime) | [`BotsterEngine`] + host `SessionRuntime` via [`prelude`] / [`engine`] |
//! | Production | `botster_core_daemon::CoreDaemon` (sibling crate) + session worker |
//!
//! Do **not** start by assembling [`MultiplexerEngine`] or speaking raw
//! `session_protocol` frames. Those remain available as advanced modules.
//!
//! # Module map (preferred discovery)
//!
//! | Module | Role |
//! | --- | --- |
//! | [`prelude`] | Embedder start-here re-exports for the session lifecycle |
//! | [`contract`] | Stable wire/session/transport/entity contracts (`contract::*`) |
//! | [`engine`] | `BotsterEngine` facade and lower-level engines (advanced below the facade) |
//! | [`runtime`] | `SessionRuntime` traits, spawn requests, optional local PTY adapters |
//! | [`package`] | Package manifest and host-profile admission helpers |
//! | [`identity`] | Crypto, device fingerprint, keyring primitives |
//!
//! # Features
//!
//! The default feature set includes `local-runtime`, which exposes the
//! policy-free local PTY/process adapter and `DefaultBotsterEngine`. Embedders
//! that only need contracts and custom host adapters can disable default
//! features and keep [`BotsterEngine`], runtime traits, and transport contracts
//! without the local process dependency.
//!
//! # Compatibility re-exports
//!
//! Flat crate-root type re-exports below are retained so existing
//! `use botster_core::{Type, ...}` imports keep compiling. New code should
//! prefer [`prelude`] or the module paths in the map above. See
//! [`prelude`] migration notes if a future release narrows flat re-exports.
//!
//! Advanced surfaces that stay exported but should not look like start-here
//! peers: `MultiplexerEngine`, `session_protocol`, capability runtime types,
//! and the short root aliases for lower-level engine submodules
//! (`multiplexer`, `session_worker`, `subscription_multiplexer`, …). Prefer
//! [`prelude`] and the facade for ordinary embeds.
//!
//! # Presentation contracts live outside Core
//!
//! UI payloads and package surface/navigation contracts are owned by Hub and
//! `botster-ui-contract`. They are intentionally unavailable from this crate:
//!
//! ```compile_fail
//! use botster_core::{UiAction, UiNode};
//! ```

/// Curated start-here re-exports for spawn/attach/drain/input/shutdown embeds.
pub mod prelude;

pub mod contract;
pub mod engine;
pub mod identity;
pub mod package;
pub mod runtime;

// ---------------------------------------------------------------------------
// Preferred module paths (also re-exported as short names for compatibility)
// ---------------------------------------------------------------------------

/// Contract submodules (`botster_core::contract::*`). Prefer these paths for
/// stable wire shapes; for session lifecycle start with [`prelude`].
///
/// `session_protocol` is advanced process wire framing — prefer engine facades
/// for ordinary session I/O.
pub use contract::{
    actor, boundary, client, client_stream, durable_session, encrypted_stream, entity,
    notification, routed_envelope, session, session_protocol, terminal_adapter, terminal_metadata,
    terminal_screen, terminal_subscription, transport,
};

/// Engine submodules. Prefer [`engine::BotsterEngine`] / `DefaultBotsterEngine`
/// over assembling lower-level engines (`multiplexer`, `session_worker`,
/// `subscription_multiplexer`, `managed_session_runtime`) directly.
pub use engine::{
    botster, client_worker, command as engine_command, managed_session_runtime, multiplexer,
    plugin_timer, plugin_worker, routed_envelope as routed_envelope_engine, session_activity,
    session_worker, subscription_multiplexer, terminal_screen as terminal_screen_engine,
};

pub use identity::{crypto, device, keyring};
pub use package::{
    capability, configuration, dependency, extension, host_profile, manifest, resolution,
    runnable_entrypoint,
};

// ---------------------------------------------------------------------------
// Flat type re-exports (compatibility). Prefer prelude / modules for new code.
// ---------------------------------------------------------------------------

pub use runtime::{
    apply_plugin_store_merge_patch, plugin_store_payload_bytes, CapabilityOperation,
    CapabilityOperationCompleted, CapabilityOperationFailure, CapabilityOperationId,
    CapabilityOperationResult, CapabilityResourceEvent, CapabilityResourceId,
    CapabilityRuntimeError, CapabilityRuntimeErrorKind, CapabilityRuntimeEvent,
    CapabilityRuntimeHandle, CapabilityRuntimeRequest, CapabilityTimerEvent, CapabilityWatchEvent,
    CapabilityWebSocketEvent, FileWatchEventSource, FileWatchRegistration, FileWatchRuntime,
    FileWatchRuntimeConfig, FileWatchSourceError, FileWatchSourceEvent, FilesystemCapabilityGrant,
    FilesystemCapabilityLimits, FilesystemCapabilityPermissions, FilesystemCapabilityRequest,
    FilesystemCapabilityResult, FilesystemEntry, FilesystemEntryKind, FilesystemMetadata,
    FilesystemOperation, HttpCapabilityEndpointPolicy, HttpCapabilityRequest,
    HttpCapabilityResponse, HttpCapabilityRuntime, HttpCapabilityRuntimeConfig,
    HttpCapabilityTransport, HttpHeader, HttpTransportRequest, InMemoryWebSocketCapabilityRuntime,
    PluginCancellationToken, PluginCapabilityRuntime, PluginRuntime, PluginStoreBackend,
    PluginStoreCapabilityRequest, PluginStoreEntry, PluginStoreKey, PluginStoreLimits,
    PluginStoreOperation, PluginStoreRecord, PluginStoreResult, ProcessIdentity,
    ScopedRelativePath, SessionRuntime, SessionRuntimeError, SessionRuntimeErrorKind,
    SessionRuntimeHandle, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
    SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory, TimerCapabilityRequest,
    WatchCapabilityRequest, WatchChangeKind, WebSocketCapabilityRequest,
    WebSocketCapabilityRuntimeConfig, WebSocketMessage, DEFAULT_FILE_WATCH_DEBOUNCE_MS,
    DEFAULT_WEBSOCKET_EVENT_CAPACITY, DEFAULT_WEBSOCKET_INBOUND_CAPACITY,
    DEFAULT_WEBSOCKET_OUTBOUND_CAPACITY,
};
#[cfg(feature = "local-runtime")]
pub use runtime::{
    LocalProcessRuntime, LocalProcessRuntimeOptions, LocalProcessWorkerRuntime, PtyIoBarrier,
    WorkerHealth, WorkerProcessRuntime, WorkerProcessRuntimeOptions,
    DEFAULT_MODE_GATED_INPUT_TIMEOUT, DEFAULT_PTY_READER_CHUNK_CAPACITY,
    DEFAULT_WORKER_EGRESS_CAPACITY,
};

pub use actor::{
    BackpressureRoute, BackpressureSummary, BoundedQueueConfig, ClientConnectionHealth,
    ClientControlFrame, ClientWorkerMessage, DeliveryLag, HubControlMessage, HubControlOrigin,
    InitialSnapshotBarrier, InitialSnapshotPhase, InitialSnapshotReady, InitialSnapshotRequest,
    MailboxSendFailure, MailboxSendFailureReason, ModeFlagsReady, PluginAdmissionResult,
    PluginCleanupResult, PluginCleanupScope, PluginCompletion, PluginCompletionDrain,
    PluginDescriptorKind, PluginDescriptorRef, PluginHandlerKind, PluginHandlerRef,
    PluginInvocationClass, PluginInvocationContext, PluginInvocationFailure,
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
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use client_stream::{
    ClientStreamGeneration, ClientStreamHarness, ClientStreamObservation, ClientStreamOutcome,
};
pub use contract::{WorkerSnapshotPhase, WorkerSnapshotRequest, WorkerSnapshotResult};
pub use crypto::{
    decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError, CryptoOperation,
    IdentityOperation,
};
pub use device::{
    device_fingerprint, verify_device_fingerprint, DeviceFingerprint, DevicePublicMetadata,
    PublicSigningKeyBytes,
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
pub use engine::{
    apply_session_activity_event, classify_session_activity, BotsterEngine, BotsterEngineError,
    BotsterEngineObservation, BotsterEngineOutput, BotsterSpawnOutcome, ClientWorker,
    ClientWorkerTeardown, EngineClientId, EngineCommand, EngineCommandError, EngineCommandEvent,
    EngineCommandKind, EngineCommandOutcome, EngineCommandResult, EngineNotificationId,
    EngineNotificationItem, EngineNotificationTarget, EngineReplaySnapshotRequest, EngineRequestId,
    EngineSessionId, EngineSessionInspection, EngineSessionIoRequest, EngineSpawnSessionMetadata,
    EngineSpawnSessionRequest, EngineSpawnSessionResult, EngineSubscriptionId,
    ManagedSessionRuntime, ManagedSessionRuntimeError, MultiplexerEngine, MultiplexerEngineError,
    MultiplexerEngineObservation, MultiplexerEngineOutcome, MultiplexerSpawnOutcome,
    PluginHandlerRegistration, PluginInvocationOutcome, PluginTimerDrainOutcome,
    PluginTimerScheduleOutcome, PluginTimerScheduler, PluginWorkerDebugSnapshot,
    PluginWorkerEngine, PluginWorkerEngineConfig, PluginWorkerPluginDebugSnapshot,
    PluginWorkerRegistration, RoutedEnvelopeRouter, SessionWorkerEngine, SessionWorkerOutcome,
    SessionWorkerRuntime, SessionWorkerRuntimeEvent, SubscriptionMultiplexer,
    SubscriptionMultiplexerObservation, SubscriptionMultiplexerOutcome, TerminalScreenEngine,
    TerminalScreenOutcome, TerminalScreenRuntime, ENGINE_COMMAND_KINDS,
};
#[cfg(feature = "local-runtime")]
pub use engine::{
    DefaultBotsterEngine, DefaultBotsterEngineError, DefaultEngineCommand,
    WorkerBackedBotsterEngine, WorkerBackedBotsterEngineError,
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
pub use package::{
    admit_host_profile, resolve_package_dependencies, validate_package_runnable_entrypoints,
    AdmittedHostProfile, HostProfileAdmissionError, HostProfileCompatibilityField,
    HostProfileMetadata, HostProfilePolicySection, PackageAuthState, PackageBlockedReason,
    PackageConfigState, PackageConfigurationField, PackageConfigurationFieldType,
    PackageConfigurationGroup, PackageConfigurationOption, PackageConfigurationSchema,
    PackageConfigurationSecretValue, PackageConfigurationValidationHints,
    PackageConfigurationValue, PackageDependency, PackageDependencyKind,
    PackageDependencyResolution, PackageFeatureGate, PackageFeatureResolution, PackageManifest,
    PackageRequirement, PackageRequirementStatus, PackageResolutionInput, PackageResolutionMatrix,
    PackageResolutionPackage, PackageResolutionState, PackageSource, RunnableEntrypoint,
    RunnableEntrypointEnvironmentRequirement, RunnableEntrypointHubConnection,
    RunnableEntrypointHubConnectionTransport, RunnableEntrypointHubConnectionValidationError,
    RunnableEntrypointInjection, RunnableEntrypointInjectionKind,
    RunnableEntrypointInjectionTarget, RunnableEntrypointKind, RunnableEntrypointLaunchMode,
    RunnableEntrypointLaunchResult, RunnableEntrypointProcessState, RunnableEntrypointReadiness,
    RunnableEntrypointResultField, RunnableEntrypointValidationError,
    RunnableEntrypointWorkingDirectory,
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
    FrameDecoder, ModeFlags, ModeFlagsPayload, ModeFreshnessToken, ModeGatedPtyInputRequest,
    ModeGatedPtyInputResult, NotificationPayload, ProcessExitedPayload, PromptMarkPayload,
    ProtocolError, ResizePayload, Rgb, SessionMetadata, TeePayload, TerminalColorProfile,
    TimeoutPayload, DESYNC_THRESHOLD, FRAME_ARM_TEE, FRAME_BELL, FRAME_CWD_CHANGED,
    FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN, FRAME_GET_SNAPSHOT, FRAME_METADATA_SHAPING,
    FRAME_MODE_FLAGS, FRAME_MODE_GATED_PTY_INPUT, FRAME_MODE_GATED_PTY_INPUT_RESULT,
    FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG, FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK,
    FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE, FRAME_SCREEN, FRAME_SET_COLOR_PROFILE,
    FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT, FRAME_SPAWN_SESSION, FRAME_TITLE_CHANGED,
    HELLO_MAGIC, MAX_FRAME_LEN, MAX_METADATA_LEN, PROTOCOL_VERSION, WELCOME_MAGIC,
};
pub use terminal_metadata::{
    TerminalMetadataKind, TerminalMetadataLaneShaper, TerminalMetadataObservation,
    TerminalMetadataProducer, TerminalMetadataShapingObservation, TerminalMetadataShapingOutcome,
};
pub use terminal_screen::{
    TerminalBackendError, TerminalOutputChunk, TerminalScreenHook, TerminalScreenSize,
    TerminalScreenState, TerminalSnapshotPayload,
};
pub use terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalSubscriptionGeneration,
    TerminalSubscriptionRecord,
};
pub use transport::{TransportEgress, TransportIngress};
