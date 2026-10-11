//! Descriptor passing over a connected unix stream socket: `SCM_RIGHTS` with `sendmsg` and `recvmsg` (Core DP-2; plan section
//! 3, "Descriptor handoff"). rustix builds the control messages, so `cmsg_len`, whose type differs between Linux and macOS,
//! is never ours.
//!
//! **A descriptor rides on the first byte of its send.** [`send_with_fd`] sends the descriptor with the bytes, and the kernel
//! keeps it with that send's segment. One [`recv_with_fds`] holds at most one segment with descriptors, and the kernels
//! place it differently (measured; `a_read_holds_one_descriptor_segment_at_its_start_on_macos_and_at_its_end_on_linux`):
//! - macOS stops a read before a segment with descriptors once it has copied data, and goes on after it, so the segment
//!   is the start of the read;
//! - Linux goes on into a segment with descriptors and stops after it, so the segment is the end of the read.
//!
//! So a reader cannot place a descriptor by its byte position alone. The host sends each `AttachRoute` frame from its first
//! byte with its stream, and nothing else with a descriptor: a read with descriptors holds the start of exactly one
//! `AttachRoute` frame, and the descriptors belong to that frame.
//!
//! **Close-on-exec.** A received descriptor must never reach a process that the receiver starts. On Linux the kernel sets
//! `FD_CLOEXEC` as the descriptor arrives (`MSG_CMSG_CLOEXEC`). macOS has no such flag: [`recv_with_fds`] sets `FD_CLOEXEC`
//! with `fcntl` before it returns, so a received descriptor is open without the flag only inside [`recv_with_fds`]. A fork
//! on another thread inside that window would leak it into the child. The invariant that closes the window: **only the
//! thread that receives descriptors forks or execs.** In the worker that is its loop thread: `Payload::spawn` refuses to run
//! on any other thread (`payload::LoopThread`), and the worker's other thread, `Payload::watch_exit`'s, only waits.
//!
//! **No `SIGPIPE`.** A send to a socket whose reader is gone fails with `EPIPE`; it never raises `SIGPIPE`, whatever the
//! process's disposition (Core A49). On Linux [`send_with_fd`] passes `MSG_NOSIGNAL`. macOS has no such flag for `sendmsg`:
//! the socket must have `SO_NOSIGPIPE` (Mio sets it on each socket that it makes, and an accepted socket gets it from Mio's
//! listener), and [`send_with_fd`] refuses a socket without it before it sends.

use rustix::io::{fcntl_setfd, FdFlags};
use rustix::net::{
    recvmsg, sendmsg, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
};
use std::io::{self, IoSlice, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

/// The most descriptors that one read takes. The host sends one descriptor with each `AttachRoute` frame, and a read stops at
/// each of them, so one read gets at most one; the room for more only lets [`recv_with_fds`] see and close extra ones. More
/// than this in one message is an error.
pub const MAX_FDS: usize = 4;

/// Sends `bytes` with `fd` on their first byte, in one `sendmsg`. Returns the bytes that the socket took (at least 1): the
/// descriptor went with them. An error (`WouldBlock` included) took nothing, and the caller still owns `fd`.
///
/// The send takes at most the socket's send buffer size (`SO_SNDBUF`) of `bytes`: the caller writes the rest as plain bytes.
/// On macOS a socket that has no room for the message refuses the whole `sendmsg` with `EMSGSIZE`, where Linux gives
/// `EAGAIN`. `EMSGSIZE` can also mean "this message can never fit", but only for a message larger than the send buffer,
/// and this send is never larger. So `EMSGSIZE` means "full": it is reported as `WouldBlock`, and the send succeeds once
/// the reader has made room (`a_full_socket_takes_neither_the_bytes_nor_the_descriptor`). The size check stays as the
/// guard: an `EMSGSIZE` for a message larger than the send buffer is an error, never a retry
/// ([`emsgsize_waits`]). `EAGAIN` needs no decision: its error has the `WouldBlock` kind on every OS.
///
/// # Errors
/// `InvalidInput` for empty `bytes` (a descriptor needs a byte to ride on) and, on macOS, for a socket without
/// `SO_NOSIGPIPE` (see the module's text); else the error of `sendmsg` (`BrokenPipe` when the reader is gone).
pub fn send_with_fd(socket: impl AsFd, bytes: &[u8], fd: BorrowedFd<'_>) -> io::Result<usize> {
    if bytes.is_empty() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    #[cfg(target_os = "macos")]
    if !rustix::net::sockopt::socket_nosigpipe(&socket)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the socket raises SIGPIPE: it has no SO_NOSIGPIPE",
        ));
    }
    let send_buffer = rustix::net::sockopt::socket_send_buffer_size(&socket)?;
    let bytes = &bytes[..bytes.len().min(send_buffer.max(1))];
    let fds = [fd];
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
        return Err(io::Error::other("no room for one descriptor"));
    }
    match sendmsg(socket, &[IoSlice::new(bytes)], &mut control, send_flags()) {
        Ok(n) => Ok(n),
        #[cfg(target_os = "macos")]
        Err(rustix::io::Errno::MSGSIZE) if emsgsize_waits(bytes.len(), send_buffer) => {
            Err(io::ErrorKind::WouldBlock.into())
        }
        Err(errno) => Err(errno.into()),
    }
}

/// The flags of the send: on Linux the kernel returns `EPIPE` without `SIGPIPE` (see the module's text).
fn send_flags() -> SendFlags {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        SendFlags::NOSIGNAL
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        SendFlags::empty()
    }
}

/// macOS: whether an `EMSGSIZE` for a refused `sendmsg` of `len` bytes with a descriptor means "no room now" (see
/// [`send_with_fd`]): the message fits the send buffer. `EMSGSIZE` for a larger message means that it can never fit: an
/// error, so that the caller does not retry forever. Only macOS compiles the decision, so the Linux gate lists none of its
/// mutants, and the Mac run tests them.
#[cfg(target_os = "macos")]
fn emsgsize_waits(len: usize, send_buffer: usize) -> bool {
    len <= send_buffer
}

/// One read of [`recv_with_fds`]: the bytes read into the buffer, and the descriptors that came with them, each with
/// `FD_CLOEXEC` set. The read holds at most one segment with descriptors: at its start on macOS, at its end on Linux (see
/// the module's text). So the descriptors belong to the one `AttachRoute` frame that starts in this read (plan 23ae), not
/// to a byte position.
#[derive(Debug)]
pub struct Received {
    pub len: usize,
    pub fds: Vec<OwnedFd>,
}

/// The flags of the read: on Linux the kernel sets close-on-exec on each received descriptor.
fn recv_flags() -> RecvFlags {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        RecvFlags::CMSG_CLOEXEC
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        RecvFlags::empty()
    }
}

/// The control space of one read: more than any message that a sender's kernel accepts at the default limits. Linux takes at
/// most `SCM_MAX_FD` (253) descriptors in one message, and macOS at most about 254 (`EINVAL` above; measured on macOS 27),
/// and 4096 bytes hold 1020. So the kernel never has to cut a message's descriptors (`MSG_CTRUNC`).
///
/// It must not: on macOS a cut message keeps its full `cmsg_len`, and rustix 1.1.5 reads the descriptors by that length,
/// past the end of the buffer (`AncillaryDrain::advance`, `src/net/send_recv/msg.rs:544`, `**length -= msg_len`: an
/// overflow panic in a debug build, out-of-bounds descriptors in a release build). `RecvAncillaryBuffer`'s drop drains
/// too, so the panic comes in a destructor and the process aborts. So a cut message is never parsed (see [`recv_into`]).
/// With this space the cut cannot happen; that branch is a defence only.
pub const CONTROL_BYTES: usize = 4096;

const _: () = assert!(rustix::cmsg_space!(ScmRights(MAX_FDS)) <= CONTROL_BYTES);

/// Reads bytes and the descriptors that come with them. `len` 0 with no descriptor is the end of the stream. Every received
/// descriptor has `FD_CLOEXEC` set before this returns (see the module's text).
///
/// # Errors
/// The error of `recvmsg` (`WouldBlock` included), an `fcntl` failure, more descriptors than [`MAX_FDS`], or a message that
/// the kernel cut (`MSG_CTRUNC`). On an error the caller treats the socket as broken, never as a route that silently has no
/// stream.
pub fn recv_with_fds(socket: impl AsFd, buf: &mut [u8]) -> io::Result<Received> {
    let mut space = [MaybeUninit::uninit(); CONTROL_BYTES];
    recv_into(socket, buf, &mut space)
}

/// [`recv_with_fds`] with its control space given (a test gives a small one, so that the kernel cuts a message).
///
/// A cut message (`MSG_CTRUNC`) is not parsed: the buffer is forgotten, so neither a drain nor its drop reads the cut
/// `cmsg_len` (see [`CONTROL_BYTES`]). The descriptors that did arrive stay open, unseen, until the process exits. That is
/// acceptable: the caller treats the socket as broken, and a worker whose control link is broken ends, so the leak lasts
/// only until that end. The other errors close every descriptor that arrived.
fn recv_into(
    socket: impl AsFd,
    buf: &mut [u8],
    space: &mut [MaybeUninit<u8>],
) -> io::Result<Received> {
    let mut control = RecvAncillaryBuffer::new(space);
    let message = recvmsg(
        socket,
        &mut [IoSliceMut::new(buf)],
        &mut control,
        recv_flags(),
    )?;
    if message.flags.contains(ReturnFlags::CTRUNC) {
        std::mem::forget(control);
        return Err(io::Error::other(
            "the kernel cut the descriptors of a message (MSG_CTRUNC)",
        ));
    }
    let mut fds = Vec::new();
    for ancillary in control.drain() {
        if let RecvAncillaryMessage::ScmRights(received) = ancillary {
            fds.extend(received);
        }
    }
    for fd in &fds {
        fcntl_setfd(fd, FdFlags::CLOEXEC)?;
    }
    if fds.len() > MAX_FDS {
        return Err(io::Error::other(format!(
            "more descriptors came than one read takes ({} received)",
            fds.len()
        )));
    }
    Ok(Received {
        len: message.bytes,
        fds,
    })
}

#[cfg(test)]
mod tests;
