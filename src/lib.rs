//! Reusable Botster runtime contracts and transport-neutral primitives.
//!
//! `botster-core` is the shared substrate for Botster hosts and clients. It
//! defines stable data shapes and low-level contracts, while `botster-hub`
//! owns Botster policy and orchestration.

pub mod boundary;
pub mod capability;
pub mod client;
pub mod crypto;
pub mod entity;
pub mod extension;
pub mod package;
pub mod session;
pub mod session_protocol;
pub mod transport;
pub mod ui;

pub use boundary::{Layer, LayerResponsibility};
pub use capability::{Capability, CapabilitySet, CapabilitySurface};
pub use client::{ClientId, ClientScope, ClientState};
pub use crypto::{CryptoOperation, IdentityOperation};
pub use entity::{EntityFrame, EntityId, EntityKind};
pub use extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
pub use package::{PackageManifest, PackageSource};
pub use session::{RequestId, SessionId, SubscriptionId};
pub use session_protocol::{
    decode_hello, decode_welcome, encode_empty, encode_frame, encode_hello, encode_json,
    encode_string, encode_welcome, mode_changed_from_flags, read_hello, read_welcome, write_hello,
    write_welcome, Frame, FrameDecoder, ModeChanged, ModeFlags, NotificationPayload,
    ProcessExitedPayload, PromptMarkPayload, ProtocolError, ResizePayload, Rgb, SessionMetadata,
    TeePayload, TerminalColorProfile, TimeoutPayload, DESYNC_THRESHOLD, FRAME_ARM_TEE, FRAME_BELL,
    FRAME_CWD_CHANGED, FRAME_GET_MODE_FLAGS, FRAME_GET_SCREEN, FRAME_GET_SNAPSHOT,
    FRAME_MODE_CHANGED, FRAME_MODE_FLAGS, FRAME_NOTIFICATION, FRAME_PING, FRAME_PONG,
    FRAME_PROCESS_EXITED, FRAME_PROMPT_MARK, FRAME_PTY_INPUT, FRAME_PTY_OUTPUT, FRAME_RESIZE,
    FRAME_SCREEN, FRAME_SET_COLOR_PROFILE, FRAME_SET_TIMEOUT, FRAME_SHUTDOWN, FRAME_SNAPSHOT,
    FRAME_TITLE_CHANGED, HELLO_MAGIC, MAX_FRAME_LEN, MAX_METADATA_LEN, PROTOCOL_VERSION,
    WELCOME_MAGIC,
};
pub use transport::{TransportEgress, TransportIngress};
