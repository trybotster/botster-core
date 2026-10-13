//! The `unix_diag` request and reply (`linux/sock_diag.h`, `linux/unix_diag.h`, `linux/netlink.h`) of `route_frames_taken`'s
//! Linux form (`real::rqueue`), in the host's byte order as netlink wants. Pure bytes with no OS call, so it compiles and is
//! tested on every OS, and its bolero harness (`tests/props.rs` `sock_diag_decoder`) needs no feature.

use std::io;

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const NLM_F_REQUEST: u16 = 0x1;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const AF_UNIX: u8 = 1;
/// `UDIAG_SHOW_PEER | UDIAG_SHOW_RQLEN` (0x4 and 0x10): the peer's inode and the receive queue. One literal: the two
/// flags are distinct bits, so `|` and `^` give the same value, an equivalent mutant.
const UDIAG_SHOW: u32 = 0x14;
const UNIX_DIAG_PEER: u16 = 2;
const UNIX_DIAG_RQLEN: u16 = 4;
/// `INET_DIAG_NOCOOKIE`: the lookup names the socket by its inode alone.
const NO_COOKIE: u32 = u32::MAX;
/// `struct nlmsghdr`.
const HEADER: usize = 16;
/// `struct unix_diag_req`.
const REQUEST: usize = 24;
/// `struct unix_diag_msg`.
const MESSAGE: usize = 16;

/// What the lookup found of one socket.
#[derive(Debug, PartialEq, Eq)]
pub struct Found {
    pub peer: u32,
    pub unread: u32,
}

/// `unix_diag` names a socket by a 32-bit inode.
pub fn inode(inode: u64) -> io::Result<u32> {
    u32::try_from(inode)
        .map_err(|_| io::Error::other(format!("socket inode {inode} does not fit unix_diag")))
}

/// The exact lookup of the socket with inode `ino`: its peer and its receive queue.
pub fn request(ino: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + REQUEST);
    out.extend_from_slice(&((HEADER + REQUEST) as u32).to_ne_bytes());
    out.extend_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    out.extend_from_slice(&NLM_F_REQUEST.to_ne_bytes());
    out.extend_from_slice(&[0; 8]); // nlmsg_seq, nlmsg_pid
    out.extend_from_slice(&[AF_UNIX, 0, 0, 0]); // sdiag_family, sdiag_protocol, pad
    out.extend_from_slice(&u32::MAX.to_ne_bytes()); // udiag_states: every state
    out.extend_from_slice(&ino.to_ne_bytes());
    out.extend_from_slice(&UDIAG_SHOW.to_ne_bytes());
    out.extend_from_slice(&NO_COOKIE.to_ne_bytes());
    out.extend_from_slice(&NO_COOKIE.to_ne_bytes());
    out
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_ne_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_ne_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn bad(what: &str) -> io::Error {
    io::Error::other(format!("unix_diag reply: {what}"))
}

/// The answer for the socket with inode `ino` in one reply: its peer and its unread bytes. An error message (the socket
/// was not found: `ENOENT`), a reply for another socket, or a message that does not parse is an error.
pub fn parse(reply: &[u8], ino: u32) -> io::Result<Found> {
    let len = u32_at(reply, 0).ok_or_else(|| bad("no message header"))? as usize;
    let message = reply
        .get(..len)
        .filter(|_| len >= HEADER)
        .ok_or_else(|| bad("a message longer than the reply"))?;
    match u16_at(message, 4).ok_or_else(|| bad("no message type"))? {
        NLMSG_ERROR => {
            let errno = i32_at(message, HEADER).ok_or_else(|| bad("no error code"))?;
            // The kernel sends a negative errno. Zero (an acknowledgement) and i32::MIN (no positive errno) do not parse.
            let errno = errno
                .checked_neg()
                .filter(|e| *e > 0)
                .ok_or_else(|| bad("an error code that is not a negative errno"))?;
            Err(io::Error::other(format!(
                "the worker's end was not found: {}",
                io::Error::from_raw_os_error(errno)
            )))
        }
        NLMSG_DONE => Err(bad("done with no answer")),
        SOCK_DIAG_BY_FAMILY => {
            let body = &message[HEADER..];
            if body.first() != Some(&AF_UNIX) || u32_at(body, 4) != Some(ino) {
                return Err(bad("an answer for another socket"));
            }
            let mut peer = None;
            let mut unread = None;
            let mut at = MESSAGE;
            while at + 4 <= body.len() {
                let attr = usize::from(u16_at(body, at).ok_or_else(|| bad("attribute"))?);
                let data = body
                    .get(at + 4..at + attr)
                    .filter(|_| attr >= 4)
                    .ok_or_else(|| bad("an attribute longer than its message"))?;
                match u16_at(body, at + 2) {
                    Some(UNIX_DIAG_PEER) => peer = u32_at(data, 0),
                    Some(UNIX_DIAG_RQLEN) => unread = u32_at(data, 0),
                    _ => {}
                }
                at += attr.next_multiple_of(4);
            }
            Ok(Found {
                peer: peer.ok_or_else(|| bad("no peer"))?,
                unread: unread.ok_or_else(|| bad("no receive queue"))?,
            })
        }
        other => Err(bad(&format!("message type {other}"))),
    }
}
