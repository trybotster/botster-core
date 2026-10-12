//! Direct checks of the real driver's descriptor state and byte accounting.
#![cfg(feature = "slow")]

use super::slow_driver::payload_guard;
use super::slow_driver::process_guard::cleanup::CLEANUP;
use super::*;
use botster_core_contract::prelude::{InstanceId, Size};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream as StdStream};
use std::time::Duration;

struct Harness {
    // Field order is the guard's two phases, on every exit path (panics included): the cleanup request first, then the
    // bounded production cleanup (the driver's payload ends the group), then the guard, which reads its member's report.
    release: Option<payload_guard::Release>,
    driver: Bounded<Driver>,
    peer: StdStream,
    guard: Option<payload_guard::PayloadGuard>,
    root: tempfile::TempDir,
}

/// A value whose drop is production cleanup: it runs on a helper thread within the cleanup limit, so a production drop
/// that blocks fails the test instead of holding it. A test that runs the value elsewhere takes it out.
struct Bounded<T: Send + 'static>(Option<T>);

impl<T: Send + 'static> Bounded<T> {
    fn take(&mut self) -> T {
        self.0.take().expect("the value is still here")
    }
}

impl<T: Send + 'static> std::ops::Deref for Bounded<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.as_ref().expect("the value is still here")
    }
}

impl<T: Send + 'static> std::ops::DerefMut for Bounded<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0.as_mut().expect("the value is still here")
    }
}

impl<T: Send + 'static> Drop for Bounded<T> {
    fn drop(&mut self) {
        let Some(value) = self.0.take() else {
            return;
        };
        let (done, finished) = mpsc::channel();
        std::thread::spawn(move || {
            drop(value);
            let _ = done.send(());
        });
        // Production's drop can wait for the guard's member, whose cleanup takes up to CLEANUP, and then completes and
        // reports: the outer limit allows both.
        let limit = 2 * CLEANUP;
        // timer: deadline — bounds production's cleanup in a test.
        if finished.recv_timeout(limit).is_err() {
            let report = format!("production's cleanup did not finish within {limit:?}");
            if std::thread::panicking() {
                eprintln!("{report}");
            } else {
                panic!("{report}");
            }
        }
    }
}

impl Harness {
    fn new() -> Self {
        Self::with_read_bound(READ_CHUNK)
    }

    fn with_read_bound(read_chunk: NonZeroUsize) -> Self {
        let root = tempfile::Builder::new()
            .prefix("edges")
            .tempdir_in("/tmp")
            .unwrap();
        let control = root.path().join("c");
        let listener = UnixListener::bind(&control).unwrap();
        let driver = Driver::start_with_read_bound(
            &WorkerLaunch {
                control,
                instance: InstanceId("1-1".into()),
                host_epoch: 1,
                token: [5; 32],
                endpoint: root.path().join("e"),
                startup_ms: WorkerLaunch::millis(
                    botster_core_contract::prelude::CoreLimits::default().startup,
                ),
            },
            read_chunk,
        )
        .unwrap();
        let (peer, _) = listener.accept().unwrap();
        peer.set_nonblocking(true).unwrap();
        Self {
            release: None,
            driver: Bounded(Some(driver)),
            peer,
            guard: None,
            root,
        }
    }

    /// The harness owns `guard`: its cleanup request goes out before production's cleanup, and its report is read after.
    fn own(&mut self, guard: payload_guard::PayloadGuard) {
        self.release = Some(guard.release_handle());
        self.guard = Some(guard);
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
        self.own(guard);
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
        // timer: deadline — bounds the program's queued-output marker, and then its PTY output.
        let limit = rustix::event::Timespec {
            tv_sec: 10,
            tv_nsec: 0,
        };
        assert!(rustix::event::poll(&mut fds, Some(&limit)).unwrap() > 0);
        assert!(fds[0].revents().contains(rustix::event::PollFlags::IN));
        let mut marker = [0; 64];
        let count = reader.read(&mut marker).unwrap();
        assert_eq!(&marker[..count], b"queued\n");
        // The marker and the PTY output travel on two channels: on Linux the slave's bytes reach the master through the
        // asynchronous flip-buffer push, so the marker can arrive first (P5's gate on #222 at `7c93a2e7`). The test waits for
        // the PTY bytes themselves; the `printf` is one push of all its bytes, so a readable master holds the whole `ready`.
        let mut pty = [rustix::event::PollFd::from_borrowed_fd(
            self.driver.payload.as_ref().unwrap().master(),
            rustix::event::PollFlags::IN,
        )];
        assert!(
            rustix::event::poll(&mut pty, Some(&limit)).unwrap() > 0,
            "the program's PTY output did not arrive"
        );
        assert!(pty[0].revents().contains(rustix::event::PollFlags::IN));
    }
}

#[test]
fn control_and_pty_reads_retain_bytes_at_each_positive_bound() {
    use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
    for bound in [65536, 1088, 1] {
        let mut h = Harness::with_read_bound(NonZeroUsize::new(bound).unwrap());
        let payloads = [vec![5; 1024], vec![7; 2800]];
        let mut wire = Vec::new();
        for payload in &payloads {
            encode_frame(FrameType::WORKER_MSG, payload, u32::MAX, &mut wire).unwrap();
        }
        h.peer.write_all(&wire).unwrap();
        let mut retained = Vec::new();
        while retained.len() < wire.len() {
            h.driver.read_control();
            let Some(Input::LinkBytes { bytes, descriptors }) = h.driver.inputs.pop_front() else {
                panic!("queued control bytes must remain readable at bound {bound}");
            };
            assert!(descriptors.is_empty());
            assert!(!bytes.is_empty() && bytes.len() <= bound);
            retained.extend(bytes);
            assert!(h.driver.inputs.is_empty());
        }
        assert_eq!(retained, wire);
        let mut decoder = FrameDecoder::new(1 << 20);
        let mut offset = 0;
        let mut frames = Vec::new();
        while offset < retained.len() {
            let took = decoder.push(&retained[offset..]);
            assert!(took > 0);
            offset += took;
            while let Some(frame) = decoder.next_frame().unwrap() {
                assert_eq!(frame.kind, FrameType::WORKER_MSG);
                frames.push(frame.payload);
            }
        }
        assert_eq!(frames, payloads);
        h.driver.read_control();
        assert!(!h.driver.control_readable);
        assert!(h.driver.inputs.is_empty());

        h.waiting_payload();
        let mut output = Vec::new();
        while output.len() < b"ready".len() {
            h.driver.read_pty_chunk().unwrap();
            let Some(Input::PtyOutput(bytes)) = h.driver.inputs.pop_front() else {
                panic!("queued program bytes must remain readable at bound {bound}");
            };
            assert!(!bytes.is_empty() && bytes.len() <= bound);
            output.extend(bytes);
            assert!(h.driver.inputs.is_empty());
        }
        assert_eq!(output, b"ready");
        h.driver.read_pty_chunk().unwrap();
        assert!(!h.driver.pty_readable);
        assert!(h.driver.inputs.is_empty());
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
        Some(Input::LinkBytes {
            bytes: b"retained".to_vec(),
            descriptors: Vec::new(),
        })
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
    h.own(guard);
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
        // timer: deadline — bounds the program's FIFO progress marker.
        let limit = rustix::event::Timespec {
            tv_sec: 10,
            tv_nsec: 0,
        };
        // The poll borrows the reader only within this block, so the read below can take it.
        let ready = {
            let mut fds = [rustix::event::PollFd::new(
                &*reader,
                rustix::event::PollFlags::IN,
            )];
            rustix::event::poll(&mut fds, Some(&limit)).unwrap()
        };
        assert!(ready > 0);
        let mut bytes = [0; 64];
        let count = reader.read(&mut bytes).unwrap();
        assert_eq!(&bytes[..count], expected);
    };
    marker(&mut fifos[0], b"ready\n");
    h.driver.read_pty_chunk().unwrap();
    assert!(!h.driver.pty_readable);
    assert!(h.driver.inputs.is_empty());
    // The test reads the link as a host does: the worker reports the program's output (Core 6.2), and a link that nobody
    // reads would keep the staged close of `Remove` waiting for its bytes. The thread ends at the link's end of file. The
    // clone comes before `send`, which borrows the link.
    let mut link = h.peer.try_clone().unwrap();
    link.set_nonblocking(false).unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while matches!(link.read(&mut buf), Ok(n) if n > 0) {}
    });
    let mut send = |kind, payload: &[u8]| {
        let mut wire = Vec::new();
        botster_core_link::frame::encode_frame(kind, payload, u32::MAX, &mut wire).unwrap();
        h.peer.write_all(&wire).unwrap();
    };
    let instance = InstanceId("1-1".into());
    let mut hello = Vec::new();
    botster_core_link::hello::Hello {
        protocol: 1,
        proof: botster_core_link::proof::host_proof(&[5; 32], &instance, 1),
        instance,
        host_epoch: 1,
    }
    .encode(&mut hello)
    .unwrap();
    send(botster_core_link::frame::FrameType::HELLO, &hello);
    // The program cannot fill the PTY until the real loop must rearm its cleared read flag.
    let driver = h.driver.take();
    let (sent, received) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        let _ = sent.send(driver.run());
    });
    fifos[1].write_all(b"g").unwrap();
    marker(&mut fifos[2], b"done\n");
    let mut remove = Vec::new();
    botster_core_link::msg::HostMsg::Remove.encode(&mut remove);
    // The guard's two phases around production's cleanup (`Remove`): the request first, the report after.
    drop(h.release.take());
    send(botster_core_link::frame::FrameType::HOST_MSG, &remove);
    // Retirement includes production's cleanup, which can wait for the guard's member (up to CLEANUP) and then
    // completes: the outer limit allows both, as in `Bounded`.
    received
        // timer: deadline — bounds retirement of the real driver loop.
        .recv_timeout(2 * CLEANUP)
        .unwrap()
        .unwrap();
    thread.join().unwrap();
    drop(h.guard.take());
}

#[test]
fn pty_reads_clear_readiness_and_finish_a_bounded_drain() {
    let mut h = Harness::new();
    h.driver.drain = Some(Drain::Counted(4));
    h.driver.read_pty_chunk().unwrap();
    assert_eq!(h.driver.inputs.pop_front(), Some(Input::PtyDrained));
    h.driver.read_pty_chunk().unwrap();
    assert!(h.driver.inputs.is_empty());
    h.waiting_payload();
    h.driver.pty_readable = false;
    h.driver.read_pty_chunk().unwrap();
    assert!(h.driver.inputs.is_empty());
    h.driver.pty_readable = true;
    h.driver.read_pty_chunk().unwrap();
    assert_eq!(
        h.driver.inputs.pop_front(),
        Some(Input::PtyOutput(b"ready".to_vec()))
    );
    h.driver.read_pty_chunk().unwrap();
    assert!(!h.driver.pty_readable);
    assert!(h.driver.inputs.is_empty());
    // A drain whose read finds nothing is complete; `PtyDrained` follows in the next read step.
    h.driver.drain = Some(Drain::Counted(4));
    h.driver.read_pty_chunk().unwrap();
    assert_eq!(h.driver.drain, Some(Drain::Done));
    h.driver.read_pty_chunk().unwrap();
    assert_eq!(h.driver.inputs.pop_front(), Some(Input::PtyDrained));
    assert!(h.driver.drain.is_none());
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
    assert!(h.driver.drain.is_none());
}

/// A31 evidence (lead, QUESTION A31-B; not a final test): the payload writes `n` bytes and a title that the driver has not
/// read, its group is killed, and the test then reads the master itself until the end of the output. It prints the count
/// that `pending_output` gave before the kill, each read's result, and how the output ended (EIO or EOF), and checks that
/// every byte that was written comes before that end.
#[test]
fn a31_evidence_the_tail_after_a_kill_comes_before_the_end_of_the_output() {
    const TITLE: &[u8] = b"\x1b]0;T\x1b\\";
    for n in [256usize, 1000, 4096, 16 * 1024, 60 * 1024] {
        let mut h = Harness::new();
        let ready = h.root.path().join("ready");
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
        let guard = payload_guard::PayloadGuard::new(h.root.path());
        let script = format!(
            "{}/usr/bin/head -c {n} /dev/zero | /usr/bin/tr '\\0' a; printf '\\033]0;T\\033\\\\'; /bin/echo queued > '{}'; read answer",
            guard.prefix(),
            ready.display()
        );
        h.own(guard);
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
        let mut fds = [rustix::event::PollFd::from_borrowed_fd(
            reader.as_fd(),
            rustix::event::PollFlags::IN,
        )];
        // timer: deadline — bounds the program's marker after its output; a PTY that holds less than n unread blocks it.
        let limit = rustix::event::Timespec {
            tv_sec: 3,
            tv_nsec: 0,
        };
        if rustix::event::poll(&mut fds, Some(&limit)).unwrap() == 0 {
            let payload = h.driver.payload.as_ref().unwrap();
            eprintln!(
                "A31-EVIDENCE os={} n={n} BLOCKED: no marker within 3 s, count={}",
                std::env::consts::OS,
                payload.pending_output().unwrap()
            );
            continue;
        }
        let mut marker = [0; 64];
        let count = reader.read(&mut marker).unwrap();
        assert_eq!(&marker[..count], b"queued\n");
        let payload = h.driver.payload.as_ref().unwrap();
        let before = payload.pending_output().unwrap();
        payload.signal_group(9);
        let mut output = Vec::new();
        let mut reads = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        // On macOS a dying leader can wait for its terminal to drain, so the test reads while it waits for the exit, and
        // counts those reads as before the exit.
        let mut waited = Duration::ZERO;
        loop {
            // timer: deadline — one slice of the bounded wait for the production exit watch after the group kill.
            let slice = Duration::from_millis(50);
            if h.driver.exits.1.recv_timeout(slice).is_ok() {
                break;
            }
            waited += slice;
            assert!(
                waited < Duration::from_secs(10),
                "n={n}: no exit after the kill"
            );
            match payload.read(&mut buf) {
                Ok(k) if k > 0 => {
                    output.extend_from_slice(&buf[..k]);
                    reads.push(format!("pre-exit:{k}"));
                }
                other => reads.push(format!("pre-exit:{other:?}")),
            }
        }
        let after_exit = payload.pending_output().unwrap();
        let end = loop {
            match payload.read(&mut buf) {
                Ok(0) => break "EOF".to_string(),
                Ok(k) => {
                    output.extend_from_slice(&buf[..k]);
                    reads.push(format!("{k}"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    reads.push("EAGAIN".into());
                    let mut pty = [rustix::event::PollFd::from_borrowed_fd(
                        payload.master(),
                        rustix::event::PollFlags::IN,
                    )];
                    // timer: deadline — bounds the wait for the master's next state after an EAGAIN.
                    let wait = rustix::event::Timespec {
                        tv_sec: 2,
                        tv_nsec: 0,
                    };
                    let ready = rustix::event::poll(&mut pty, Some(&wait)).unwrap();
                    reads.push(format!("poll={ready}:{:?}", pty[0].revents()));
                    assert!(ready > 0, "n={n}: the master stayed quiet after EAGAIN");
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => break format!("{:?} errno {:?}", e.kind(), e.raw_os_error()),
            }
        };
        eprintln!(
            "A31-EVIDENCE os={} n={n} count_before_kill={before} count_after_exit={after_exit} read={} reads=[{}] end={end}",
            std::env::consts::OS,
            output.len(),
            reads.join(",")
        );
        let mut expected = vec![b'a'; n];
        expected.extend_from_slice(TITLE);
        assert_eq!(
            output.len(),
            expected.len(),
            "n={n}: bytes lost before {end}"
        );
        assert_eq!(output, expected);
    }
}
