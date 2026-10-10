//! The steward's condition on `break_control` (plan 23l), on the production edges: after the break, every later call of Core
//! on the link reaches `RealEdges`' closed-link state (`Ok(0)` on `link_recv`, `BrokenPipe` on `link_send`), never an
//! operation on a raw descriptor number that the OS may have given to a new descriptor. It uses real sockets, so it lives in
//! the slow tier.
//!
//! Clause: Core LC-5, Core A2-1 (a broken control link).
#![cfg(feature = "slow")]

use botster_core::open_parts;
use botster_core_contract::prelude::*;
use botster_core_host::driver::HostEdges;
use botster_core_testkit::edge_tap::{EdgeTap, Rows};
use std::collections::BTreeSet;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

/// The open descriptor numbers of this process (`/dev/fd`, on Linux and macOS), without directories. The listing holds a
/// descriptor of its own, at the lowest free number, which may be the number that the test just freed; it is the only
/// directory that the test has open, so leaving out directories leaves out the listing itself.
fn open_descriptors() -> BTreeSet<i32> {
    std::fs::read_dir("/dev/fd")
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .parse()
                .unwrap()
        })
        .filter(|fd: &i32| !std::fs::metadata(format!("/dev/fd/{fd}")).is_ok_and(|m| m.is_dir()))
        .collect()
}

/// One read that cannot block: a byte already queued on a Unix socket, or its end of file, is there at once (the peer's
/// write or close is complete), so `WouldBlock` fails the check at once instead of hanging the test.
fn read_now(stream: &mut UnixStream, buf: &mut [u8]) -> io::Result<usize> {
    stream.set_nonblocking(true)?;
    stream.read(buf)
}

#[test]
fn a_broken_link_never_reaches_a_descriptor_that_reuses_its_number() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d");
    let (_cfg, inner) = open_parts(OpenConfig {
        data_dir: dir.clone(),
        worker_path: Some("/the/worker/is/not/started".into()),
        limits: CoreLimits::default(),
    })
    .unwrap();
    let (mut edges, tap) = EdgeTap::new(inner, Rows::default());
    let mut worker = UnixStream::connect(dir.join("c")).unwrap();
    // A connect on a Unix socket is queued at once; the accept sees it.
    let link = edges.accept_link().expect("a worker is waiting");
    let mut buf = [0u8; 16];
    worker.write_all(b"hi").unwrap();
    assert_eq!(edges.link_recv(link, &mut buf).unwrap(), 2);
    assert_eq!(&buf[..2], b"hi");

    let before = open_descriptors();
    tap.upgrade()
        .expect("the edges hold the tap")
        .lock()
        .unwrap()
        .break_link(link);
    // Core's end of the stream is closed, not shut down: the worker reads EOF.
    assert_eq!(read_now(&mut worker, &mut buf).unwrap(), 0);
    let closed: Vec<i32> = before.difference(&open_descriptors()).copied().collect();
    assert_eq!(
        closed.len(),
        1,
        "the break closes exactly Core's end: {closed:?}"
    );

    // The lowest free descriptor number is the next one given out. The proof needs a new socket at the number of Core's
    // closed end, so it fills any lower free number with spare pairs until one end takes it, and checks that it did.
    let old = closed[0];
    let mut spare = Vec::new();
    let (mut new_near, mut new_far) = loop {
        let (a, b) = UnixStream::pair().unwrap();
        if a.as_raw_fd() == old {
            break (a, b);
        }
        if b.as_raw_fd() == old {
            break (b, a);
        }
        assert!(
            a.as_raw_fd() < old && b.as_raw_fd() < old,
            "no new socket took the number {old} of Core's closed end"
        );
        spare.push((a, b));
    };
    new_far.write_all(b"z").unwrap();
    assert_eq!(edges.link_recv(link, &mut buf).unwrap(), 0);
    assert_eq!(
        edges.link_send(link, b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    // The new descriptor was not read and not written: its byte waits, and nothing arrived at its peer.
    assert_eq!(read_now(&mut new_near, &mut buf).unwrap(), 1);
    assert_eq!(&buf[..1], b"z");
    assert_eq!(
        read_now(&mut new_far, &mut buf).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    // The driver's own close comes later and changes nothing.
    edges.link_close(link);
    assert_eq!(edges.link_recv(link, &mut buf).unwrap(), 0);
}
