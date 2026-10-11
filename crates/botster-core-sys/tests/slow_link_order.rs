//! The order of a control link whose peer process ended (steward ruling R-78 item 4, design test 14). Slow tier: each test
//! starts a real peer process.
//!
//! The host driver reads a link on after a send error, so the frames that a worker wrote before it ended are still handled.
//! That relies on the kernel: a unix stream whose peer ended gives the bytes that wait in it first, and only then the end of
//! file or `ECONNRESET`, also when the host's own bytes were never read by the peer. These tests prove it on the platform
//! that runs them, with a real peer process that writes a frame and ends without reading.
//!
//! Clause: steward ruling R-78 item 4 (drain before classify).
#![cfg(feature = "slow")]

use botster_test_process::OwnedChild;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::Duration;

/// What the peer writes before it ends.
const FRAME: &[u8] = b"exited-frame\n";

/// A link whose peer process wrote `FRAME` and ended without reading the bytes that the host sent first (`unread`, or as
/// many as the socket takes). The host holds no copy of the peer's end.
fn link_of_an_ended_peer(unread: usize) -> UnixStream {
    let (host, worker) = UnixStream::pair().unwrap();
    // The host's bytes that the peer never reads: as many as `unread`, or as many as the socket takes.
    host.set_nonblocking(true).unwrap();
    let mut left = unread;
    while left > 0 {
        match (&host).write(&vec![b'h'; left]) {
            Ok(n) => left -= n,
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(e) => panic!("{e}"),
        }
    }
    assert!(unread == 0 || left < unread, "some bytes wait unread");
    host.set_nonblocking(false).unwrap();
    // timer: deadline — bounds a read that the kernel would never end
    host.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // timer: deadline — bounds a send that the kernel would never end
    host.set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let as_stdin = OwnedFd::from(worker.try_clone().unwrap());
    let as_stdout = OwnedFd::from(worker);
    // The peer ends by itself at once; the owned child is reaped here, and its drop ends it on every other path.
    let mut peer = OwnedChild::spawn(
        Command::new("/bin/echo")
            .arg(std::str::from_utf8(&FRAME[..FRAME.len() - 1]).unwrap())
            .stdin(Stdio::from(as_stdin))
            .stdout(Stdio::from(as_stdout))
            .stderr(Stdio::null()),
    )
    .unwrap();
    assert!(peer.status().success(), "the peer wrote its frame");
    host
}

/// Reads to the end: the bytes, then how the stream ended (`None` for an end of file, the error kind otherwise).
fn read_to_end(host: &UnixStream) -> (Vec<u8>, Option<ErrorKind>) {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        match (&*host).read(&mut buf) {
            Ok(0) => return (got, None),
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return (got, Some(e.kind())),
        }
    }
}

fn assert_frame_then_end(host: &UnixStream) {
    let (got, end) = read_to_end(host);
    assert_eq!(got, FRAME, "the peer's frame comes first, whole");
    assert!(
        matches!(end, None | Some(ErrorKind::ConnectionReset)),
        "then the end of file or ECONNRESET: {end:?}"
    );
}

/// The send fails first (the order of the driver's pump: it flushes before it reads), then the read gives the frame.
#[test]
fn a_send_error_then_the_read_gives_the_ended_peers_frame_first() {
    for unread in [0, 1, 64 * 1024] {
        let host = link_of_an_ended_peer(unread);
        let sent = (&host).write(b"kill");
        assert!(
            sent.is_err(),
            "unread {unread}: a send to an ended peer fails: {sent:?}"
        );
        assert_frame_then_end(&host);
    }
}

/// The read comes first: the frame, then the end; a send after it fails.
#[test]
fn the_read_gives_the_ended_peers_frame_before_the_end_and_a_send_then_fails() {
    for unread in [0, 1, 64 * 1024] {
        let host = link_of_an_ended_peer(unread);
        assert_frame_then_end(&host);
        assert!((&host).write(b"kill").is_err(), "unread {unread}");
    }
}
