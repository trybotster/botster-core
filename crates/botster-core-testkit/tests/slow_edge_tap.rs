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
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

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

    tap.upgrade()
        .expect("the edges hold the tap")
        .lock()
        .unwrap()
        .break_link(link);
    // Core's end of the stream is closed, not shut down: the worker reads EOF.
    assert_eq!(worker.read(&mut buf).unwrap(), 0);

    // The lowest free descriptor number is the next one given out, so the first new socket may take the old number.
    let (mut new_near, mut new_far) = UnixStream::pair().unwrap();
    new_far.write_all(b"z").unwrap();
    assert_eq!(edges.link_recv(link, &mut buf).unwrap(), 0);
    assert_eq!(
        edges.link_send(link, b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    // The new descriptor was not read and not written: its byte waits, and nothing arrived at its peer.
    assert_eq!(new_near.read(&mut buf).unwrap(), 1);
    assert_eq!(&buf[..1], b"z");
    new_far.set_nonblocking(true).unwrap();
    assert_eq!(
        new_far.read(&mut buf).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    // The driver's own close comes later and changes nothing.
    edges.link_close(link);
    assert_eq!(edges.link_recv(link, &mut buf).unwrap(), 0);
}
