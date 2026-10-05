//! Direct checks of the real driver's descriptor state and byte accounting.
#![cfg(feature = "slow")]

#[path = "../../../botster-core-sys/tests/common/payload_guard.rs"]
mod payload_guard;

use super::*;
use botster_core_contract::prelude::{InstanceId, Size};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream as StdStream};
use std::time::Duration;

struct Harness {
    // Field order releases independent ownership before the production reaper.
    guard: Option<payload_guard::PayloadGuard>,
    driver: Driver,
    peer: StdStream,
    root: tempfile::TempDir,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("edges")
            .tempdir_in("/tmp")
            .unwrap();
        let control = root.path().join("c");
        let listener = UnixListener::bind(&control).unwrap();
        let driver = Driver::start(&WorkerLaunch {
            control,
            instance: InstanceId("1-1".into()),
            host_epoch: 1,
            token: [5; 32],
        })
        .unwrap();
        let (peer, _) = listener.accept().unwrap();
        peer.set_nonblocking(true).unwrap();
        Self {
            driver,
            peer,
            guard: None,
            root,
        }
    }

    fn waiting_payload(&mut self) {
        let ready = self.root.path().join("ready");
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&ready)
            .status()
            .unwrap()
            .success());
        let mut reader = std::fs::File::from(
            rustix::fs::open(
                &ready,
                rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .unwrap(),
        );
        let guard = payload_guard::PayloadGuard::new(self.root.path());
        let script = format!(
            "{}printf ready; /bin/echo queued > '{}'; read answer; exit 3",
            guard.prefix(),
            ready.display()
        );
        self.guard = Some(guard);
        let id = self
            .driver
            .spawn(&PayloadSpec {
                argv: vec!["/bin/sh".into(), "-c".into(), script],
                env: std::collections::BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
                cwd: "/".into(),
                size: Size {
                    rows: 24,
                    cols: 80,
                    cell_px: None,
                },
            })
            .unwrap();
        assert_eq!(id.pid, self.driver.payload.as_ref().unwrap().pid());
        let mut fds = [rustix::event::PollFd::from_borrowed_fd(
            reader.as_fd(),
            rustix::event::PollFlags::IN,
        )];
        // timer: deadline — bounds the program's queued-output marker.
        let limit = rustix::event::Timespec {
            tv_sec: 10,
            tv_nsec: 0,
        };
        assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
        assert!(fds[0].revents().contains(rustix::event::PollFlags::IN));
        let mut marker = [0; 64];
        let count = reader.read(&mut marker).unwrap();
        assert_eq!(&marker[..count], b"queued\n");
    }
}

#[test]
fn control_reads_require_readiness_and_report_eof_once() {
    let mut h = Harness::new();
    h.peer.write_all(b"retained").unwrap();
    h.driver.control_readable = false;
    h.driver.read_control();
    assert!(h.driver.inputs.is_empty());
    h.driver.control_readable = true;
    h.driver.read_control();
    assert_eq!(
        h.driver.inputs.pop_front(),
        Some(Input::LinkBytes(b"retained".to_vec()))
    );
    h.driver.read_control();
    assert!(!h.driver.control_readable);
    assert!(h.driver.inputs.is_empty());
    h.peer.shutdown(std::net::Shutdown::Both).unwrap();
    h.driver.control_readable = true;
    h.driver.read_control();
    assert!(!h.driver.link_open);
    assert_eq!(h.driver.inputs.pop_front(), Some(Input::LinkClosed));
    h.driver.read_control();
    assert!(h.driver.inputs.is_empty());
}

#[test]
fn link_loss_clears_queued_bytes_and_close_is_idempotent() {
    for notify in [false, true] {
        let mut h = Harness::new();
        h.driver.outbound.extend(b"unsent");
        if notify {
            h.driver.link_lost();
        } else {
            h.driver.drop_link();
        }
        assert!(!h.driver.link_open);
        assert!(!h.driver.control_readable);
        assert!(h.driver.outbound.is_empty());
        assert_eq!(
            h.driver.inputs.pop_front(),
            notify.then_some(Input::LinkClosed)
        );
        let mut byte = [0];
        assert_eq!(h.peer.read(&mut byte).unwrap(), 0);
        h.driver.link_lost();
        h.driver.drop_link();
        assert!(h.driver.inputs.is_empty());
        h.driver.outbound.extend(b"ignored");
        h.driver.flush().unwrap();
        h.driver.read_control();
        assert!(h.driver.inputs.is_empty());
        assert_eq!(h.driver.written, 0);
    }
}

#[test]
fn partial_writes_retain_bytes_and_track_interest_and_totals() {
    let mut h = Harness::new();
    rustix::net::sockopt::set_socket_send_buffer_size(&h.driver.control, 1024).unwrap();
    let wire: Vec<_> = (0..262_144).map(|index| (index % 251) as u8).collect();
    h.driver.outbound.extend(&wire);
    h.driver.control_writable = false;
    h.driver.flush().unwrap();
    assert!(h.driver.writable_interest);
    assert_eq!(h.driver.written, 0);
    assert!(h.driver.inputs.is_empty());
    h.driver.control_writable = true;
    h.driver.flush().unwrap();
    assert!(!h.driver.control_writable);
    assert!(!h.driver.outbound.is_empty());
    assert!(h.driver.written > 0);
    assert_eq!(
        h.driver.inputs.pop_front(),
        Some(Input::LinkWritten {
            total: h.driver.written
        })
    );
    let mut received = Vec::new();
    // timer: deadline — bounds completion of finite bytes through a partial-write socket.
    let deadline = Instant::now() + Duration::from_secs(10);
    while received.len() < wire.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "partial-write completion");
        let mut fds = [
            rustix::event::PollFd::from_borrowed_fd(h.peer.as_fd(), rustix::event::PollFlags::IN),
            rustix::event::PollFd::from_borrowed_fd(
                h.driver.control.as_fd(),
                if h.driver.outbound.is_empty() {
                    rustix::event::PollFlags::empty()
                } else {
                    rustix::event::PollFlags::OUT
                },
            ),
        ];
        let limit = rustix::event::Timespec {
            tv_sec: remaining.as_secs().try_into().unwrap(),
            tv_nsec: remaining.subsec_nanos().into(),
        };
        assert!(
            rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0,
            "partial-write readiness"
        );
        let writable = fds[1].revents().contains(rustix::event::PollFlags::OUT);
        let received_before = received.len();
        let mut bytes = [0; 4096];
        match h.peer.read(&mut bytes) {
            Ok(n) => {
                assert!(n > 0);
                received.extend_from_slice(&bytes[..n]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
        h.driver.control_writable = writable;
        let before = h.driver.written;
        h.driver.flush().unwrap();
        if h.driver.written != before {
            assert_eq!(
                h.driver.inputs.pop_front(),
                Some(Input::LinkWritten {
                    total: h.driver.written
                })
            );
        }
        assert!(
            received.len() > received_before || h.driver.written != before,
            "ready descriptors must make partial-write progress"
        );
        assert!(h.driver.inputs.is_empty());
    }
    assert_eq!(received, wire);
    assert_eq!(h.driver.written, wire.len() as u64);
    assert!(h.driver.outbound.is_empty());
    assert!(!h.driver.writable_interest);
}

#[test]
fn pty_events_resume_reads_after_would_block() {
    let mut h = Harness::new();
    let mut fifos = Vec::new();
    for name in ["ready", "go", "done"] {
        let path = h.root.path().join(name);
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        fifos.push(std::fs::File::from(
            rustix::fs::open(
                &path,
                rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .unwrap(),
        ));
    }
    let guard = payload_guard::PayloadGuard::new(h.root.path());
    let script = format!(
        "{}/bin/echo ready > '{}'; /usr/bin/head -c 1 '{}' >/dev/null; \
         /usr/bin/head -c 4194304 /dev/zero; /bin/echo done > '{}'; read answer",
        guard.prefix(),
        h.root.path().join("ready").display(),
        h.root.path().join("go").display(),
        h.root.path().join("done").display(),
    );
    h.guard = Some(guard);
    h.driver
        .spawn(&PayloadSpec {
            argv: vec!["/bin/sh".into(), "-c".into(), script],
            env: std::collections::BTreeMap::from([("PATH".into(), "/usr/bin:/bin".into())]),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
        })
        .unwrap();
    let marker = |reader: &mut std::fs::File, expected: &[u8]| {
        let mut fds = [rustix::event::PollFd::new(
            &*reader,
            rustix::event::PollFlags::IN,
        )];
        // timer: deadline — bounds the program's FIFO progress marker.
        let limit = rustix::event::Timespec {
            tv_sec: 10,
            tv_nsec: 0,
        };
        assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
        drop(fds);
        let mut bytes = [0; 64];
        let count = reader.read(&mut bytes).unwrap();
        assert_eq!(&bytes[..count], expected);
    };
    marker(&mut fifos[0], b"ready\n");
    h.driver.read_pty_chunk();
    assert!(!h.driver.pty_readable);
    assert!(h.driver.inputs.is_empty());
    let mut send = |kind, payload: &[u8]| {
        let mut wire = Vec::new();
        botster_core_link::frame::encode_frame(kind, payload, u32::MAX, &mut wire).unwrap();
        h.peer.write_all(&wire).unwrap();
    };
    let instance = InstanceId("1-1".into());
    let mut hello = Vec::new();
    botster_core_link::hello::Hello {
        protocol: 1,
        proof: botster_core_link::proof::token_proof(&[5; 32], &instance, 1),
        instance,
        host_epoch: 1,
    }
    .encode(&mut hello)
    .unwrap();
    send(botster_core_link::frame::FrameType::HELLO, &hello);
    // The program cannot fill the PTY until the real loop must rearm its cleared read flag.
    let driver = h.driver;
    let (sent, received) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _ = sent.send(driver.run());
    });
    fifos[1].write_all(b"g").unwrap();
    marker(&mut fifos[2], b"done\n");
    let mut remove = Vec::new();
    botster_core_link::msg::HostMsg::Remove.encode(&mut remove);
    send(botster_core_link::frame::FrameType::HOST_MSG, &remove);
    drop(h.guard.take());
    received
        // timer: deadline — bounds retirement of the real driver loop.
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    thread.join().unwrap();
}

#[test]
fn pty_reads_clear_readiness_and_finish_a_bounded_drain() {
    let mut h = Harness::new();
    h.driver.drain_left = Some(4);
    h.driver.read_pty_chunk();
    assert_eq!(h.driver.inputs.pop_front(), Some(Input::PtyDrained));
    h.driver.read_pty_chunk();
    assert!(h.driver.inputs.is_empty());
    h.waiting_payload();
    h.driver.pty_readable = false;
    h.driver.read_pty_chunk();
    assert!(h.driver.inputs.is_empty());
    h.driver.pty_readable = true;
    h.driver.read_pty_chunk();
    assert_eq!(
        h.driver.inputs.pop_front(),
        Some(Input::PtyOutput(b"ready".to_vec()))
    );
    h.driver.read_pty_chunk();
    assert!(!h.driver.pty_readable);
    assert!(h.driver.inputs.is_empty());
    h.driver.drain_left = Some(4);
    h.driver.read_pty_chunk();
    assert_eq!(h.driver.drain_left, Some(0));
    h.driver.read_pty_chunk();
    assert_eq!(h.driver.inputs.pop_front(), Some(Input::PtyDrained));
    assert!(h.driver.drain_left.is_none());
    h.driver.payload.as_ref().unwrap().signal_group(9);
    h.driver
        .exits
        .1
        // timer: deadline — the production exit watch must observe the group kill.
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    h.driver.pty_readable = true;
    h.driver.perform(Action::ReapPayload).unwrap();
    assert!(!h.driver.pty_registered);
    assert!(!h.driver.pty_readable);
    assert!(h.driver.payload.is_none());
    assert!(h.driver.drain_left.is_none());
}
