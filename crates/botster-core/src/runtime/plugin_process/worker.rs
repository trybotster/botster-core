//! Child side of a plugin process: the library that a worker binary runs.
//!
//! The Hub links this into its `botster-plugin-worker` binary together with
//! its Lua runtime and calls [`run_worker`] first thing in `main`. Before any
//! plugin byte is read, the library closes every inherited descriptor except
//! 0-4, installs the fatal panic hook, installs the memory cap, and applies
//! the Hub's sandbox profile.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::fd::{AsFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use crate::contract::session_protocol::{Frame, FrameDecoder};
use crate::runtime::PluginRuntime;

use super::protocol::{
    decode_json, encode_json_bounded, send_all, BootstrapFrame, FailedFrame, LoadFrame,
    LoadedFrame, PluginRegistration, ReadyFrame, SandboxProfile, CAUSE_PANIC, CHILD_FATAL_FD,
    CHILD_IPC_FD, FRAME_BOOTSTRAP, FRAME_BOOTSTRAP_FAILED, FRAME_LOAD, FRAME_LOADED,
    FRAME_LOAD_FAILED, FRAME_READY, FRAME_SHUTDOWN, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};

/// Worker-binary hooks: the policy and the runtime that the Hub supplies.
#[derive(Clone, Copy)]
pub struct WorkerHooks {
    /// Apply the Hub's opaque sandbox profile. Runs before plugin code loads.
    pub apply_sandbox: fn(&SandboxProfile) -> Result<(), String>,
    /// Load the plugin from its in-memory sources.
    pub load: fn(LoadFrame) -> Result<LoadedPlugin, String>,
}

/// A loaded plugin: its runtime and the registration reported to the parent.
pub struct LoadedPlugin {
    /// Runtime that executes the plugin's handlers.
    pub runtime: Arc<dyn PluginRuntime>,
    /// Hub-defined registration, returned to the parent in `Loaded`.
    pub registration: PluginRegistration,
}

/// Exit code for a failure that the worker reported to the parent.
const EXIT_REPORTED_FAILURE: i32 = 1;
/// Exit code for a broken channel or protocol.
const EXIT_PROTOCOL: i32 = 2;

/// Run the worker. Never returns.
pub fn run_worker(hooks: WorkerHooks) -> ! {
    close_inherited_descriptors();
    install_fatal_panic_hook();
    if !descriptor_is_open(CHILD_IPC_FD) {
        eprintln!("botster plugin worker: no IPC channel at fd {CHILD_IPC_FD}");
        std::process::exit(EXIT_PROTOCOL);
    }
    // SAFETY: fd 3 is open, was placed by the parent launcher, and nothing
    // else in this process owns it.
    let ipc = unsafe { UnixStream::from_raw_fd(CHILD_IPC_FD) };
    let mut channel = Channel::new(ipc);

    let bootstrap: BootstrapFrame = channel.expect(FRAME_BOOTSTRAP, "Bootstrap");
    if bootstrap.magic != PROTOCOL_MAGIC || bootstrap.version != PROTOCOL_VERSION {
        eprintln!(
            "botster plugin worker: protocol {} v{} is not {PROTOCOL_MAGIC} v{PROTOCOL_VERSION}",
            bootstrap.magic, bootstrap.version
        );
        std::process::exit(EXIT_PROTOCOL);
    }
    channel.max_frame_bytes = bootstrap.max_frame_bytes;
    if let Err(reason) = install_memory_cap(bootstrap.memory_cap_bytes)
        .and_then(|()| (hooks.apply_sandbox)(&bootstrap.sandbox))
    {
        channel.fail(FRAME_BOOTSTRAP_FAILED, reason);
    }
    channel.send(
        FRAME_READY,
        &ReadyFrame {
            magic: PROTOCOL_MAGIC.to_string(),
            version: PROTOCOL_VERSION,
            pid: std::process::id(),
        },
    );

    let load: LoadFrame = channel.expect(FRAME_LOAD, "Load");
    let loaded = match (hooks.load)(load) {
        Ok(loaded) => loaded,
        Err(reason) => channel.fail(FRAME_LOAD_FAILED, reason),
    };
    let registration = LoadedFrame {
        registration: loaded.registration,
    };
    match encode_json_bounded(FRAME_LOADED, &registration, channel.max_frame_bytes) {
        Ok(frame) => channel.write(&frame),
        Err(error) => channel.fail(
            FRAME_LOAD_FAILED,
            format!("registration does not fit the frame bound: {error}"),
        ),
    }
    let _runtime = loaded.runtime;

    // After Loaded, this slice accepts only Shutdown; invocations come later.
    match channel.next() {
        None => std::process::exit(0),
        Some(frame) if frame.frame_type == FRAME_SHUTDOWN => std::process::exit(0),
        Some(frame) => {
            eprintln!(
                "botster plugin worker: unexpected frame type {:#04x}",
                frame.frame_type
            );
            std::process::exit(EXIT_PROTOCOL);
        }
    }
}

/// The child's end of the IPC channel.
struct Channel {
    ipc: UnixStream,
    decoder: FrameDecoder,
    pending: VecDeque<Frame>,
    max_frame_bytes: usize,
}

impl Channel {
    fn new(ipc: UnixStream) -> Self {
        Self {
            ipc,
            decoder: FrameDecoder::new(),
            pending: VecDeque::new(),
            // Only Ready and failure frames go out before Bootstrap sets it.
            max_frame_bytes: 64 * 1024,
        }
    }

    /// The next frame, or `None` at EOF. A broken stream ends the worker.
    fn next(&mut self) -> Option<Frame> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Some(frame);
            }
            let read = match self.ipc.read(&mut buf) {
                Ok(0) => return None,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            };
            match self.decoder.feed(&buf[..read]) {
                Ok(frames) => self.pending.extend(frames),
                Err(error) => {
                    eprintln!("botster plugin worker: bad frame from parent: {error}");
                    std::process::exit(EXIT_PROTOCOL);
                }
            }
        }
    }

    fn expect<T: serde::de::DeserializeOwned>(&mut self, frame_type: u8, name: &str) -> T {
        let Some(frame) = self.next() else {
            std::process::exit(0);
        };
        if frame.frame_type != frame_type {
            eprintln!(
                "botster plugin worker: expected {name}, got frame type {:#04x}",
                frame.frame_type
            );
            std::process::exit(EXIT_PROTOCOL);
        }
        decode_json(&frame).unwrap_or_else(|error| {
            eprintln!("botster plugin worker: bad {name} frame: {error}");
            std::process::exit(EXIT_PROTOCOL);
        })
    }

    fn send<T: serde::Serialize>(&mut self, frame_type: u8, value: &T) {
        match encode_json_bounded(frame_type, value, self.max_frame_bytes) {
            Ok(frame) => self.write(&frame),
            Err(error) => {
                eprintln!("botster plugin worker: cannot encode frame: {error}");
                std::process::exit(EXIT_PROTOCOL);
            }
        }
    }

    fn write(&mut self, frame: &[u8]) {
        if send_all(self.ipc.as_fd(), frame).is_err() {
            std::process::exit(EXIT_PROTOCOL);
        }
    }

    /// Report a failure to the parent and exit.
    fn fail(&mut self, frame_type: u8, reason: String) -> ! {
        let mut reason = reason;
        // Keep the report inside the frame bound; the reason is diagnostic.
        let budget = self.max_frame_bytes.saturating_sub(64);
        if reason.len() > budget {
            let mut end = budget;
            while !reason.is_char_boundary(end) {
                end -= 1;
            }
            reason.truncate(end);
        }
        self.send(frame_type, &FailedFrame { reason });
        std::process::exit(EXIT_REPORTED_FAILURE);
    }
}

fn install_memory_cap(cap: Option<u64>) -> Result<(), String> {
    match cap {
        None => Ok(()),
        // The capped allocator arrives in a later slice; until then a
        // requested cap is refused rather than silently ignored.
        Some(_) => {
            Err("a memory cap requires the capped allocator, which this worker lacks".into())
        }
    }
}

/// Report a panic through the fatal-cause pipe, then abort. The byte write
/// is one non-blocking system call and never touches the IPC channel.
fn install_fatal_panic_hook() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        write_fatal_cause(CAUSE_PANIC);
        std::process::abort();
    }));
}

pub(crate) fn write_fatal_cause(cause: u8) {
    // SAFETY: a one-byte write from a live local to the fatal-cause pipe;
    // write is async-signal-safe and allocation-free.
    let _ = unsafe { libc::write(CHILD_FATAL_FD, (&cause as *const u8).cast(), 1) };
}

fn descriptor_is_open(fd: libc::c_int) -> bool {
    // SAFETY: F_GETFD only queries the descriptor table.
    unsafe { libc::fcntl(fd, libc::F_GETFD) >= 0 }
}

/// Close every descriptor above the fatal-cause pipe. This covers
/// descriptors that the parent's libraries opened without close-on-exec,
/// and the launcher's intermediate copies.
fn close_inherited_descriptors() {
    #[cfg(target_os = "linux")]
    const FD_DIR: &str = "/proc/self/fd";
    #[cfg(not(target_os = "linux"))]
    const FD_DIR: &str = "/dev/fd";
    let Ok(entries) = std::fs::read_dir(FD_DIR) else {
        eprintln!("botster plugin worker: cannot list {FD_DIR}");
        std::process::exit(EXIT_PROTOCOL);
    };
    let open: Vec<libc::c_int> = entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > CHILD_FATAL_FD)
        .collect();
    // The directory's own descriptor is closed by now; closing it again only
    // returns EBADF.
    for fd in open {
        // SAFETY: closing a descriptor number this process may own; no Rust
        // object owns any of them this early in the worker.
        unsafe { libc::close(fd) };
    }
}
