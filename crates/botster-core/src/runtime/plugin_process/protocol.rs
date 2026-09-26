//! Wire frames between the parent and one plugin worker process.
//!
//! Frames use the session-protocol codec, `[u32 LE len][u8 type][payload]`,
//! with a Hub-supplied maximum length. Envelopes are JSON. Plugin-API bodies
//! are Hub-owned `BoundaryJson` values that Core carries without reading.

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd};

use serde::{Deserialize, Serialize};

use crate::boundary::BoundaryJson;
use crate::contract::session_protocol::{encode_frame, Frame, ProtocolError};

/// Protocol identity carried in `Bootstrap` and echoed in `Ready`.
pub(crate) const PROTOCOL_MAGIC: &str = "BPW1";
/// Protocol version carried in `Bootstrap` and echoed in `Ready`.
pub(crate) const PROTOCOL_VERSION: u8 = 1;

// Parent to child.
pub(crate) const FRAME_BOOTSTRAP: u8 = 0x01;
pub(crate) const FRAME_LOAD: u8 = 0x02;
pub(crate) const FRAME_SHUTDOWN: u8 = 0x06;

// Child to parent.
pub(crate) const FRAME_READY: u8 = 0x81;
pub(crate) const FRAME_BOOTSTRAP_FAILED: u8 = 0x82;
pub(crate) const FRAME_LOADED: u8 = 0x83;
pub(crate) const FRAME_LOAD_FAILED: u8 = 0x84;

/// The IPC socket end that the child finds at a fixed descriptor.
pub(crate) const CHILD_IPC_FD: i32 = 3;
/// The fatal-cause pipe write end that the child finds at a fixed descriptor.
pub(crate) const CHILD_FATAL_FD: i32 = 4;

/// Cause bytes written to the fatal-cause pipe.
pub(crate) const CAUSE_MEMORY_CAP: u8 = 1;
pub(crate) const CAUSE_PANIC: u8 = 2;

/// The Hub's sandbox profile (for example Seatbelt, or Landlock with
/// seccomp). Core carries it to the Hub's own `apply_sandbox` hook unread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SandboxProfile(pub BoundaryJson);

/// A plugin package's module set, as in-memory text, for the plugin runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginSources(pub BoundaryJson);

/// A plugin's configuration, for the plugin runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginConfig(pub BoundaryJson);

/// What a loaded plugin registers (handlers, descriptors), reported by the
/// plugin runtime and interpreted by the Hub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginRegistration(pub BoundaryJson);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BootstrapFrame {
    pub magic: String,
    pub version: u8,
    pub sandbox: SandboxProfile,
    pub memory_cap_bytes: Option<u64>,
    /// The parent's frame bound, so the child can refuse locally a frame
    /// that the parent would treat as a protocol violation.
    pub max_frame_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReadyFrame {
    pub magic: String,
    pub version: u8,
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FailedFrame {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LoadedFrame {
    pub registration: PluginRegistration,
}

/// The `Load` frame: the package sources and config the Hub sends.
///
/// Credit grants join this frame in the slice that introduces host calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadFrame {
    /// Package module set as in-memory text; the sandboxed child has no
    /// filesystem access to its package.
    pub sources: PluginSources,
    /// Plugin configuration.
    pub config: PluginConfig,
}

/// Encode one frame, refusing a frame longer than `max_len` (type byte plus
/// payload) before any byte is written.
pub(crate) fn encode_bounded(
    frame_type: u8,
    payload: &[u8],
    max_len: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let len = payload.len() + 1;
    if len > max_len {
        return Err(ProtocolError::FrameEncodeTooLarge { len, max: max_len });
    }
    encode_frame(frame_type, payload)
}

pub(crate) fn encode_json_bounded<T: Serialize>(
    frame_type: u8,
    value: &T,
    max_len: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let payload = serde_json::to_vec(value)?;
    encode_bounded(frame_type, &payload, max_len)
}

pub(crate) fn decode_json<T: serde::de::DeserializeOwned>(
    frame: &Frame,
) -> Result<T, ProtocolError> {
    frame.json()
}

/// Write all of `bytes` to a stream socket without raising SIGPIPE when the
/// peer has closed. A closed peer is `BrokenPipe`.
pub(crate) fn send_all(fd: BorrowedFd<'_>, mut bytes: &[u8]) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    const FLAGS: libc::c_int = libc::MSG_NOSIGNAL;
    // macOS has no MSG_NOSIGNAL; the socket carries SO_NOSIGPIPE instead.
    #[cfg(not(target_os = "linux"))]
    const FLAGS: libc::c_int = 0;
    while !bytes.is_empty() {
        // SAFETY: fd is a live descriptor for the duration of the borrow, and
        // the pointer and length describe the live `bytes` slice.
        let written = unsafe {
            libc::send(
                fd.as_raw_fd(),
                bytes.as_ptr().cast::<libc::c_void>(),
                bytes.len(),
                FLAGS,
            )
        };
        if written < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        let written = usize::try_from(written).unwrap_or(0);
        if written == 0 {
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        }
        bytes = &bytes[written..];
    }
    Ok(())
}
